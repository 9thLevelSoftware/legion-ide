use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use xtask::perf_harness::{
    ManualRendererPerfToml, PERF_REPORT_FILE, PerfReport, SkeletonDescriptor, SkeletonKind,
    SkeletonMeasurement, SkeletonStatus, apply_fail_on_budget_value, classify_skeleton_status,
    manual_renderer_perf_measurement, plan_m0_skeletons, plan_perf_harness, plan_perf_skeletons,
    read_manual_renderer_perf_report, read_report, resolve_workspace_git_sha, write_report,
};

struct TempDir {
    root: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        // Per-thread + monotonic counter keeps the temp dir unique even when
        // tests run in parallel within the same nanosecond.
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("legion-perf-harness-{name}-{nanos}-{seq}"));
        fs::create_dir_all(&root).expect("create temp dir");
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn tiny_skeleton() -> SkeletonDescriptor {
    SkeletonDescriptor {
        name: "test.tiny".to_string(),
        kind: SkeletonKind::InputToPaintMicrobenchmark,
        fixture_bytes: 1024,
        file_count: None,
        sample_count: 4,
        budget_millis: 5_000,
        note: "test fixture".to_string(),
    }
}

fn heavy_skeleton() -> SkeletonDescriptor {
    // Sized so each sample is dominated by a real walk (≥ ~256µs) and the
    // sorted percentile samples agree on the µs boundary across runs.
    // Used by the determinism test, where host scheduler jitter would
    // otherwise flip adjacent percentile samples.
    SkeletonDescriptor {
        name: "test.heavy".to_string(),
        kind: SkeletonKind::InputToPaintMicrobenchmark,
        fixture_bytes: 256 * 1024,
        file_count: None,
        sample_count: 32,
        budget_millis: 5_000,
        note: "determinism-test fixture".to_string(),
    }
}

#[test]
fn perf_harness_plan_is_deterministic_for_same_descriptor() {
    // The µs measurements are not byte-deterministic across host
    // scheduler states (cold cache vs warm cache, kernel preemption).
    // What is deterministic is the skeleton shape: same name, kind,
    // sample count, fixture bytes, budget, status, and message
    // template. We assert on those and not on the raw µs values.
    let skeleton = heavy_skeleton();
    let left = plan_perf_harness(&skeleton);
    let right = plan_perf_harness(&skeleton);
    assert_eq!(left.name, right.name);
    assert_eq!(left.kind, right.kind);
    assert_eq!(left.fixture_bytes, right.fixture_bytes);
    assert_eq!(left.sample_count, right.sample_count);
    assert_eq!(left.budget_millis, right.budget_millis);
    assert_eq!(left.status, right.status);
    assert!(
        left.message.starts_with("total ")
            && left.message.contains("within budget")
            && right.message.starts_with("total ")
            && right.message.contains("within budget"),
        "messages should share the success template, got left={:?} right={:?}",
        left.message,
        right.message
    );
    // Total µs should still be on the same order of magnitude — the
    // skeleton is doing real work, so even with cache jitter the total
    // does not differ by 10x.
    let left_total = left.total_micros.max(1);
    let right_total = right.total_micros.max(1);
    let ratio = left_total.max(right_total) as f64 / left_total.min(right_total) as f64;
    assert!(
        ratio < 5.0,
        "total_us should be within 5x across runs, got left={} right={} ratio={:.2}",
        left.total_micros,
        right.total_micros,
        ratio,
    );
}

#[test]
fn perf_harness_plan_records_p50_p95_within_budget() {
    let skeleton = tiny_skeleton();
    let measurement = plan_perf_harness(&skeleton);
    assert_eq!(measurement.name, skeleton.name);
    assert_eq!(measurement.kind, skeleton.kind);
    assert_eq!(measurement.fixture_bytes, skeleton.fixture_bytes);
    assert_eq!(measurement.sample_count, skeleton.sample_count);
    assert_eq!(measurement.budget_millis, skeleton.budget_millis);
    assert_eq!(measurement.status, SkeletonStatus::Passed);
    assert!(
        measurement.p50_micros <= measurement.p95_micros,
        "p50={}us should be <= p95={}us",
        measurement.p50_micros,
        measurement.p95_micros,
    );
    assert!(
        measurement.p95_micros <= measurement.total_micros,
        "p95={}us should be <= total={}us",
        measurement.p95_micros,
        measurement.total_micros,
    );
}

#[test]
fn perf_harness_zero_budget_marks_measurement_skipped() {
    let mut skeleton = tiny_skeleton();
    skeleton.budget_millis = 0;
    let measurement = plan_perf_harness(&skeleton);
    assert_eq!(measurement.status, SkeletonStatus::Skipped);
    assert!(
        measurement.message.contains("report-only"),
        "skeleton message should mention report-only mode, got {:?}",
        measurement.message
    );
}

#[test]
fn perf_harness_classifies_status_failed_when_total_exceeds_budget() {
    // Deterministically exercise the failure-classification path without
    // relying on host timing: a total above the budget must classify as
    // Failed, a total within budget as Passed, and a `None` (report-only)
    // budget as Skipped.
    assert_eq!(
        classify_skeleton_status(Duration::from_millis(5), Some(Duration::from_millis(1))),
        SkeletonStatus::Failed,
        "total above budget must classify as Failed"
    );
    // Boundary: total exactly at the budget is still within budget.
    assert_eq!(
        classify_skeleton_status(Duration::from_millis(1), Some(Duration::from_millis(1))),
        SkeletonStatus::Passed,
        "total equal to budget must classify as Passed"
    );
    assert_eq!(
        classify_skeleton_status(Duration::from_micros(1), Some(Duration::from_millis(5))),
        SkeletonStatus::Passed,
        "total below budget must classify as Passed"
    );
    assert_eq!(
        classify_skeleton_status(Duration::from_millis(5), None),
        SkeletonStatus::Skipped,
        "report-only (no budget) must classify as Skipped"
    );
}

#[test]
fn perf_harness_zero_budget_classifies_measurement_skipped() {
    // A zero-millisecond budget is report-only; the measured plan must be
    // Skipped (never Failed) regardless of how long the stand-in takes.
    let mut skeleton = tiny_skeleton();
    skeleton.budget_millis = 0;
    let measurement = plan_perf_harness(&skeleton);
    assert_eq!(measurement.status, SkeletonStatus::Skipped);
    assert_ne!(measurement.status, SkeletonStatus::Failed);
}

#[test]
fn perf_harness_fail_on_budget_value_overrides_descriptor_budget() {
    // Exercise the override logic via the injected-value helper so the test
    // never mutates process-global environment state (which would race the
    // other integration tests cargo runs concurrently in this binary).
    let mut skeleton = tiny_skeleton();
    apply_fail_on_budget_value(&mut skeleton, "0");
    assert_eq!(
        skeleton.budget_millis, 0,
        "FAIL_ON_BUDGET=0 should disable the gate (budget=0)"
    );

    let mut skeleton = tiny_skeleton();
    apply_fail_on_budget_value(&mut skeleton, "  7  ");
    assert_eq!(
        skeleton.budget_millis, 7,
        "a numeric override should set the budget (after trimming whitespace)"
    );

    let mut skeleton = tiny_skeleton();
    let original = skeleton.budget_millis;
    apply_fail_on_budget_value(&mut skeleton, "not-a-number");
    assert_eq!(
        skeleton.budget_millis, original,
        "a non-numeric override must leave the descriptor budget unchanged"
    );
}

#[test]
fn perf_harness_write_and_read_report_round_trip() {
    let temp = TempDir::new("round-trip");
    let out_dir = temp.path("target/perf-harness");
    let skeleton = tiny_skeleton();
    let report = plan_m0_skeletons("legion-desktop", "deadbeef", &skeleton);
    let path = write_report(&out_dir, &report).expect("write perf report");
    assert!(path.is_file(), "perf report should be on disk");

    let text = fs::read_to_string(&path).expect("read perf report");
    assert!(
        text.contains("schema_version = 1"),
        "report should serialize schema_version=1, got body:\n{text}"
    );
    assert!(
        text.contains("[summary]"),
        "report should contain a [summary] table"
    );
    assert!(
        text.contains("git_sha = \"deadbeef\""),
        "report should embed the supplied git sha"
    );

    let round_trip: PerfReport = read_report(&path).expect("read round trip");
    assert_eq!(round_trip, report);
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(PERF_REPORT_FILE),
        "report file name should be {}",
        PERF_REPORT_FILE
    );
}

#[test]
fn perf_harness_report_summary_matches_skeleton_statuses() {
    let temp = TempDir::new("summary");
    let out_dir = temp.path("target/perf-harness");
    let skeleton = tiny_skeleton();
    let report = plan_m0_skeletons("legion-desktop", "feedface", &skeleton);
    assert_eq!(report.summary.total, 1);
    assert_eq!(
        report.summary.passed + report.summary.failed + report.summary.skipped,
        1
    );
    // The M0 input-to-paint microbenchmark with a 5-second budget is
    // expected to pass on the hosted CI runner; if it ever fails the test
    // will surface the actual measurement.
    assert_eq!(
        report.skeletons[0].status,
        SkeletonStatus::Passed,
        "tiny skeleton should pass with a 5s budget, got status={:?} message={}",
        report.skeletons[0].status,
        report.skeletons[0].message,
    );
    assert_eq!(report.summary.passed, 1);
    assert_eq!(report.summary.failed, 0);
    assert_eq!(report.summary.skipped, 0);
    write_report(&out_dir, &report).expect("write report");
}

#[test]
fn perf_harness_m0_skeleton_descriptor_uses_safe_budget() {
    let skeleton = SkeletonDescriptor::m0_input_to_paint();
    assert_eq!(skeleton.kind, SkeletonKind::InputToPaintMicrobenchmark);
    assert!(skeleton.budget_millis > 0);
    assert!(skeleton.budget_millis <= 5_000);
    assert!(skeleton.sample_count >= 8);
    assert!(skeleton.fixture_bytes >= 1024);
    assert!(
        skeleton.note.contains("WS18.T1"),
        "skeleton note should reference the follow-on work, got {:?}",
        skeleton.note
    );
}

#[test]
fn perf_harness_line_galley_skeleton_gates_visible_rows_under_two_ms() {
    let skeleton = SkeletonDescriptor::m1_line_galley_shaping_cache();
    assert_eq!(skeleton.kind, SkeletonKind::LineGalleyShapingCache);
    assert_eq!(skeleton.fixture_bytes, 10_000);
    assert_eq!(skeleton.budget_millis, 2);
    assert!(skeleton.note.contains("visible viewport rows"));

    let measurement = plan_perf_harness(&skeleton);
    assert_eq!(measurement.status, SkeletonStatus::Passed);
    assert!(
        measurement.total_micros < 2_000,
        "line-galley frame should remain under 2ms, got {}us ({})",
        measurement.total_micros,
        measurement.message
    );
}

#[test]
fn perf_harness_default_report_includes_line_galley_gate() {
    let skeletons = vec![
        SkeletonDescriptor::m0_input_to_paint(),
        SkeletonDescriptor::m1_line_galley_shaping_cache(),
    ];
    let report = plan_perf_skeletons("legion-desktop", "feedface", &skeletons);
    assert_eq!(report.summary.total, 2);
    assert_eq!(report.skeletons.len(), 2);
    assert!(
        report
            .skeletons
            .iter()
            .any(|measurement| measurement.kind == SkeletonKind::LineGalleyShapingCache),
        "default perf report should include the WS01.T2 line-galley cache gate"
    );
}

fn manual_renderer_report(status: &str) -> ManualRendererPerfToml {
    ManualRendererPerfToml {
        schema_version: 1,
        scenario: "manual_editor_input_to_paint".to_string(),
        status: status.to_string(),
        sample_count: 16,
        keypress_p50_micros: 1_200,
        keypress_p95_micros: 20_000,
        scroll_p95_micros: 8_000,
        keypress_p50_budget_ms: 16,
        keypress_p95_budget_ms: 32,
        scroll_p95_budget_ms: 24,
        message: "desktop egui projection render path stayed within Manual latency budgets"
            .to_string(),
    }
}

#[test]
fn perf_harness_manual_renderer_budget_constants_match_ws_manual_01() {
    assert_eq!(
        xtask::perf_harness::MANUAL_RENDERER_PERF_REPORT_FILE,
        "manual_renderer_perf.toml"
    );
    let budgets = xtask::perf_harness::manual_renderer_budgets();
    assert_eq!(budgets.keypress_p50_millis, 16);
    assert_eq!(budgets.keypress_p95_millis, 32);
    assert_eq!(budgets.scroll_p95_millis, 32);
    assert_eq!(budgets.sample_count, 16);
}

#[test]
fn manual_renderer_perf_report_maps_to_perf_measurement() {
    let report = manual_renderer_report("passed");
    let measurement = manual_renderer_perf_measurement(&report);

    assert_eq!(measurement.name, "manual.renderer_input_to_paint");
    assert_eq!(
        measurement.kind,
        SkeletonKind::RendererBackedManualInputToPaint
    );
    assert_eq!(measurement.sample_count, 16);
    assert_eq!(measurement.p50_micros, 1_200);
    assert_eq!(measurement.p95_micros, 20_000);
    assert_eq!(measurement.total_micros, 28_000);
    assert_eq!(measurement.budget_millis, 32);
    assert_eq!(measurement.status, SkeletonStatus::Passed);
    assert!(measurement.message.contains("Manual latency budgets"));
}

#[test]
fn manual_renderer_perf_report_failed_status_fails_measurement() {
    let mut report = manual_renderer_report("failed");
    report.message = "Manual renderer measurement exceeded budget".to_string();

    let measurement = manual_renderer_perf_measurement(&report);

    assert_eq!(measurement.status, SkeletonStatus::Failed);
    assert!(measurement.message.contains("exceeded budget"));
}

#[test]
fn manual_renderer_perf_report_skipped_status_skips_measurement() {
    let mut report = manual_renderer_report("skipped");
    report.message = "renderer backend unavailable".to_string();

    let measurement = manual_renderer_perf_measurement(&report);

    assert_eq!(measurement.status, SkeletonStatus::Skipped);
    assert!(measurement.message.contains("renderer backend unavailable"));
}

#[test]
fn manual_renderer_direct_plan_is_subprocess_supplied_skip() {
    let skeleton = SkeletonDescriptor {
        name: "manual.renderer_input_to_paint".to_string(),
        kind: SkeletonKind::RendererBackedManualInputToPaint,
        fixture_bytes: 0,
        file_count: None,
        sample_count: 16,
        budget_millis: 32,
        note: "manual renderer subprocess fixture".to_string(),
    };

    let measurement = plan_perf_harness(&skeleton);

    assert_eq!(
        measurement.kind,
        SkeletonKind::RendererBackedManualInputToPaint
    );
    assert_eq!(measurement.status, SkeletonStatus::Skipped);
    assert_eq!(
        measurement.message,
        "renderer-backed Manual measurement is supplied by legion-desktop subprocess"
    );
}

#[test]
fn manual_renderer_perf_report_read_round_trip() {
    let temp = TempDir::new("manual-renderer-report");
    let path = temp.path("manual_renderer_perf.toml");
    fs::write(
        &path,
        r#"schema_version = 1
scenario = "manual_editor_input_to_paint"
status = "passed"
workspace_root = "C:\\workspace"
initial_file = "Cargo.toml"
report_path = "target/perf-harness/manual_renderer_perf.toml"
sample_count = 16
keypress_p50_micros = 1200
keypress_p95_micros = 20000
scroll_p95_micros = 8000
keypress_p50_budget_ms = 16
keypress_p95_budget_ms = 32
scroll_p95_budget_ms = 24
message = "desktop egui projection render path stayed within Manual latency budgets"
"#,
    )
    .expect("write manual renderer report fixture");

    let report = read_manual_renderer_perf_report(&path).expect("parse manual renderer report");

    assert_eq!(report, manual_renderer_report("passed"));
}

#[test]
fn perf_harness_resolve_workspace_git_sha_returns_unknown_for_missing_repo() {
    let temp = TempDir::new("no-git");
    let sha = resolve_workspace_git_sha(&temp.root);
    assert_eq!(sha, "unknown");
}

#[test]
fn perf_harness_skeleton_measurement_serializes_stably() {
    let measurement = SkeletonMeasurement {
        name: "test".to_string(),
        kind: SkeletonKind::InputToPaintMicrobenchmark,
        fixture_bytes: 1024,
        sample_count: 4,
        total_micros: 1_234,
        p50_micros: 12,
        p95_micros: 34,
        budget_millis: 250,
        status: SkeletonStatus::Passed,
        message: "ok".to_string(),
    };
    let text = toml::to_string_pretty(&measurement).expect("serialize");
    assert!(
        text.contains("kind = \"input_to_paint_microbenchmark\""),
        "input-to-paint kind should serialize as stable snake_case, got:\n{text}"
    );
    let round_trip: SkeletonMeasurement = toml::from_str(&text).expect("deserialize");
    assert_eq!(round_trip, measurement);
}

#[test]
fn perf_harness_skeleton_kind_serializes_stable_snake_case() {
    let cases = [
        (
            SkeletonKind::InputToPaintMicrobenchmark,
            "input_to_paint_microbenchmark",
            "inputtopaintmicrobenchmark",
        ),
        (
            SkeletonKind::LineGalleyShapingCache,
            "line_galley_shaping_cache",
            "linegalleyshapingcache",
        ),
        (
            SkeletonKind::RendererBackedManualInputToPaint,
            "renderer_backed_manual_input_to_paint",
            "renderer_backed_manual_input_to_paint",
        ),
    ];
    for (kind, expected, legacy) in cases {
        let measurement = SkeletonMeasurement {
            name: "kind-test".to_string(),
            kind,
            fixture_bytes: 0,
            sample_count: 1,
            total_micros: 0,
            p50_micros: 0,
            p95_micros: 0,
            budget_millis: 0,
            status: SkeletonStatus::Skipped,
            message: "kind serialization test".to_string(),
        };
        let serialized = toml::to_string(&measurement).expect("serialize skeleton measurement");
        assert!(
            serialized.contains(expected),
            "kind {kind:?} should serialize as {expected}, got {serialized}"
        );
        let legacy = serialized.replace(expected, legacy);
        let parsed: SkeletonMeasurement =
            toml::from_str(&legacy).expect("legacy skeleton kind should deserialize");
        assert_eq!(parsed.kind, kind);
    }
}
