//! Tests for the real product workloads (P8.F4.T1) and their budgets.
//!
//! These are written so that each one fails if the thing it describes stops
//! working: every budget assertion is paired with the opposite case, so a
//! classifier that returned a constant would fail half of them.

use xtask::perf_harness::{SkeletonKind, SkeletonStatus};
use xtask::perf_workloads::{
    ProductBudget, ProductWorkloadRow, classify_product_row, git_report_only_measurements,
    parse_product_perf_report, product_measurements, product_workload_policies,
    regate_with_ceiling,
};

fn policy(name: &str) -> xtask::perf_workloads::ProductWorkloadPolicy {
    product_workload_policies()
        .into_iter()
        .find(|policy| policy.name == name)
        .unwrap_or_else(|| panic!("policy `{name}` should exist"))
}

fn row(name: &str) -> ProductWorkloadRow {
    ProductWorkloadRow {
        name: name.to_string(),
        measured: true,
        sample_count: 64,
        fixture_bytes: 1_526_802,
        p50_micros: 0,
        p95_micros: 0,
        total_micros: 0,
        bytes_value: 0,
        detail: "test row".to_string(),
    }
}

// ── The workload set ─────────────────────────────────────────────────────────

/// Every reference workload P8.F4.T1 names has a policy, and none of them is
/// optional. This is the list `verify-perf-harness` enforces coverage against,
/// so it is also the list that would have to shrink for a workload to go
/// missing quietly.
#[test]
fn product_policies_cover_every_named_reference_workload() {
    let names: Vec<&str> = product_workload_policies()
        .into_iter()
        .map(|policy| policy.name)
        .collect();
    for expected in [
        "p8.startup",
        "p8.input_to_paint",
        "p8.scroll_jank",
        "p8.memory_ceiling",
        "p8.legion_repo",
        "p8.fixture_100k_files",
        "git.ui_dispatch_refresh",
        "git.remote_push_does_not_block_dispatch",
    ] {
        assert!(
            names.contains(&expected),
            "product workload `{expected}` must be enforced; got {names:?}"
        );
    }
}

#[test]
fn git_dispatch_policies_gate_four_milliseconds_p95() {
    for name in [
        "git.ui_dispatch_refresh",
        "git.remote_push_does_not_block_dispatch",
    ] {
        match policy(name).budget {
            ProductBudget::Latency {
                p50_millis,
                p95_millis,
            } => {
                assert_eq!(p50_millis, 4);
                assert_eq!(p95_millis, 4);
            }
            other => panic!("{name} must be a latency budget, got {other:?}"),
        }
    }
}

#[test]
fn git_follow_up_rows_are_explicitly_report_only() {
    let rows = git_report_only_measurements();
    assert_eq!(rows.len(), 3);
    for row in rows {
        assert!(!row.measured, "{} must not claim a measurement", row.name);
        assert_eq!(row.status, SkeletonStatus::Skipped);
        assert!(row.message.contains("report-only"));
    }
}

/// ADR-0048's numbers, not a rounded restatement of them.
#[test]
fn input_to_paint_uses_adr_0048_keypress_budgets() {
    match policy("p8.input_to_paint").budget {
        ProductBudget::Latency {
            p50_millis,
            p95_millis,
        } => {
            assert_eq!(p50_millis, 16, "ADR-0048 keypress p50 budget is 16ms");
            assert_eq!(p95_millis, 32, "ADR-0048 keypress p95 budget is 32ms");
        }
        other => panic!("input-to-paint must be a latency budget, got {other:?}"),
    }
    match policy("p8.scroll_jank").budget {
        ProductBudget::Latency { p95_millis, .. } => {
            assert_eq!(p95_millis, 32, "ADR-0048 scroll p95 budget is 32ms");
        }
        other => panic!("scroll must be a latency budget, got {other:?}"),
    }
}

// ── Latency classification ───────────────────────────────────────────────────

#[test]
fn latency_within_budget_passes_and_over_budget_fails() {
    let policy = policy("p8.input_to_paint");

    let mut fast = row("p8.input_to_paint");
    fast.p50_micros = 2_011;
    fast.p95_micros = 2_304;
    let measurement = classify_product_row(&policy, &fast, "windows");
    assert_eq!(measurement.status, SkeletonStatus::Passed);
    assert!(measurement.measured);
    assert!(!measurement.synthetic_stand_in);
    assert_eq!(measurement.kind, SkeletonKind::ProductWorkload);

    // p95 alone over budget must fail: a keystroke that is usually fine and
    // occasionally 40ms is exactly the jank the p95 budget exists to catch.
    let mut janky = row("p8.input_to_paint");
    janky.p50_micros = 2_011;
    janky.p95_micros = 40_000;
    let measurement = classify_product_row(&policy, &janky, "windows");
    assert_eq!(measurement.status, SkeletonStatus::Failed);
    assert!(
        measurement.message.contains("OVER BUDGET"),
        "an over-budget row must say so in the report, got: {}",
        measurement.message
    );

    // p50 alone over budget must also fail.
    let mut slow = row("p8.input_to_paint");
    slow.p50_micros = 17_000;
    slow.p95_micros = 17_500;
    assert_eq!(
        classify_product_row(&policy, &slow, "windows").status,
        SkeletonStatus::Failed
    );
}

/// The single-sample workloads have no meaningful p95, so their p95 ceiling is
/// 0 and must not be treated as "budget of zero milliseconds".
#[test]
fn zero_percentile_ceiling_is_untracked_not_impossible() {
    let policy = policy("p8.startup");
    let mut startup = row("p8.startup");
    startup.p50_micros = 3_612_368;
    startup.p95_micros = 3_612_368;
    assert_eq!(
        classify_product_row(&policy, &startup, "windows").status,
        SkeletonStatus::Passed,
        "a 3.6s startup is inside the ceiling; the untracked p95 must not fail it"
    );

    let mut slow = row("p8.startup");
    slow.p50_micros = 90_000_000;
    slow.p95_micros = 90_000_000;
    assert_eq!(
        classify_product_row(&policy, &slow, "windows").status,
        SkeletonStatus::Failed,
        "a 90s startup is outside the ceiling"
    );
}

/// Product budgets must stay armed on hosted CI, and this now checks it.
///
/// The previous version asserted a function that returned a literal `true` —
/// a claim restating itself. The claim is real and testable: the skeleton-era
/// `LEGION_PERF_FAIL_ON_BUDGET_MS=0` override made every budget on every OS
/// unfailable, which is P8.F4.T2's stop condition, and product classification
/// must ignore that variable entirely.
///
/// So the variable is set to the value that disarms skeleton budgets, and an
/// over-budget product row must still fail.
#[test]
fn product_budgets_ignore_the_skeleton_report_only_override() {
    // Safety: set and removed around a synchronous classification call that
    // spawns nothing. `classify_product_row` reads no environment at all, which
    // is the property under test.
    unsafe {
        std::env::set_var("LEGION_PERF_FAIL_ON_BUDGET_MS", "0");
    }
    let policy = policy("p8.startup");
    let mut over = row("p8.startup");
    over.p50_micros = 90_000_000;
    over.p95_micros = 90_000_000;
    let measurement = classify_product_row(&policy, &over, "linux");
    unsafe {
        std::env::remove_var("LEGION_PERF_FAIL_ON_BUDGET_MS");
    }

    assert_eq!(
        measurement.status,
        SkeletonStatus::Failed,
        "a 90s startup must fail even with the skeleton override set; product ceilings are          sized to survive a slow runner precisely so they can stay armed everywhere"
    );
}

// ── Memory classification ────────────────────────────────────────────────────

#[test]
fn memory_ceiling_gates_on_bytes_not_time() {
    let policy = policy("p8.memory_ceiling");
    let mut within = row("p8.memory_ceiling");
    within.bytes_value = 23_767_922;
    let measurement = classify_product_row(&policy, &within, "windows");
    assert_eq!(measurement.status, SkeletonStatus::Passed);
    assert_eq!(measurement.bytes_value, 23_767_922);
    assert_eq!(
        measurement.budget_millis, 0,
        "a byte-valued workload must not advertise a millisecond budget"
    );

    let mut over = row("p8.memory_ceiling");
    over.bytes_value = 64 * 1024 * 1024;
    let measurement = classify_product_row(&policy, &over, "windows");
    assert_eq!(measurement.status, SkeletonStatus::Failed);
    assert!(
        measurement.message.contains("OVER CEILING"),
        "an over-ceiling row must say so, got: {}",
        measurement.message
    );
}

// ── Unmeasured rows ──────────────────────────────────────────────────────────

/// A workload that could not run must not be reported as a pass, and must
/// carry the reason.
#[test]
fn unmeasured_row_stays_unmeasured() {
    let policy = policy("p8.legion_repo");
    let mut broken = row("p8.legion_repo");
    broken.measured = false;
    broken.detail = "repo search found 0 hits for the planted needle".to_string();
    let measurement = classify_product_row(&policy, &broken, "linux");
    assert!(!measurement.measured);
    assert_eq!(measurement.status, SkeletonStatus::Skipped);
    assert!(
        measurement.message.contains("0 hits"),
        "the reason must survive into the report, got: {}",
        measurement.message
    );
    assert!(
        measurement.message.contains("linux"),
        "the OS must be in the row so a three-OS artifact set is readable, got: {}",
        measurement.message
    );
}

/// A workload missing from the subprocess report becomes an unmeasured row
/// rather than disappearing. This is the P8.F4.T2 failure mode: a shorter
/// report that still looks green.
#[test]
fn absent_workload_becomes_an_unmeasured_row() {
    let report = parse_product_perf_report(
        "schema_version = 1\nos = \"linux\"\narch = \"x86_64\"\nworkspace_root = \"/w\"\n",
    )
    .expect("empty report should parse");
    let measurements = product_measurements(&report);
    assert_eq!(
        measurements.len(),
        product_workload_policies().len(),
        "one row per policy, even when the subprocess reported nothing"
    );
    assert!(
        measurements.iter().all(|m| !m.measured),
        "no row may claim to be measured when the report carried no workloads"
    );
    assert!(
        measurements[0].message.contains("absent"),
        "the row must say the workload was absent, got: {}",
        measurements[0].message
    );
}

#[test]
fn report_with_wrong_schema_version_is_rejected() {
    let err = parse_product_perf_report("schema_version = 2\nos = \"linux\"\n")
        .expect_err("schema_version 2 must be rejected");
    assert!(err.contains("schema_version"), "got: {err}");
}

/// The subprocess's own report format round-trips, so a field rename on the
/// product side fails here rather than silently reporting zeros.
#[test]
fn subprocess_report_shape_parses() {
    let text = "schema_version = 1\n\
                os = \"windows\"\n\
                arch = \"x86_64\"\n\
                workspace_root = \"D:/legion-ide\"\n\
                \n\
                [[workload]]\n\
                name = \"p8.input_to_paint\"\n\
                measured = true\n\
                sample_count = 64\n\
                fixture_bytes = 1526802\n\
                p50_micros = 2011\n\
                p95_micros = 2304\n\
                total_micros = 130000\n\
                bytes_value = 0\n\
                detail = \"real keystroke plus real projection\"\n";
    let report = parse_product_perf_report(text).expect("parse");
    assert_eq!(report.os, "windows");
    assert_eq!(report.workload.len(), 1);
    assert_eq!(report.workload[0].p95_micros, 2_304);

    let measurements = product_measurements(&report);
    let input = measurements
        .iter()
        .find(|m| m.name == "p8.input_to_paint")
        .expect("input-to-paint row");
    assert!(input.measured);
    assert_eq!(input.p95_micros, 2_304);
    assert_eq!(input.status, SkeletonStatus::Passed);
}

// ── Failing-gate drill ───────────────────────────────────────────────────────

/// A ceiling of 0 makes a row report-only, and must not resurrect a workload
/// that never ran.
#[test]
fn zero_ceiling_relaxes_budgets_but_not_missing_measurements() {
    let input_policy = policy("p8.input_to_paint");
    let mut janky = row("p8.input_to_paint");
    janky.p95_micros = 40_000;
    let mut measurement = classify_product_row(&input_policy, &janky, "ubuntu");
    assert_eq!(measurement.status, SkeletonStatus::Failed);
    regate_with_ceiling(&mut measurement, 0);
    assert_eq!(
        measurement.status,
        SkeletonStatus::Skipped,
        "ceiling 0 makes a budget failure report-only"
    );
    assert_eq!(
        measurement.p95_micros, 40_000,
        "the measured number must survive the re-gate"
    );

    let mut absent = row("p8.legion_repo");
    absent.measured = false;
    let mut measurement = classify_product_row(&policy("p8.legion_repo"), &absent, "ubuntu");
    regate_with_ceiling(&mut measurement, 0);
    assert!(
        !measurement.measured,
        "relaxing a budget must not turn a workload that did not run into one that did"
    );
}

/// The failing-gate drill: a ceiling below the measured value must fail the
/// row, and must decline to decide the byte-valued one rather than silently
/// passing it.
#[test]
fn tight_ceiling_fails_latency_and_declines_bytes() {
    let mut fast = row("p8.input_to_paint");
    fast.p50_micros = 2_011;
    fast.p95_micros = 2_304;
    let mut measurement = classify_product_row(&policy("p8.input_to_paint"), &fast, "windows");
    assert_eq!(measurement.status, SkeletonStatus::Passed);
    regate_with_ceiling(&mut measurement, 1);
    assert_eq!(
        measurement.status,
        SkeletonStatus::Failed,
        "a 1ms ceiling must fail a 2.3ms p95"
    );

    let mut memory = row("p8.memory_ceiling");
    memory.bytes_value = 23_767_922;
    let mut measurement = classify_product_row(&policy("p8.memory_ceiling"), &memory, "windows");
    regate_with_ceiling(&mut measurement, 1);
    assert_eq!(
        measurement.status,
        SkeletonStatus::Passed,
        "a millisecond ceiling cannot decide a byte-valued workload"
    );
    assert!(
        measurement.message.contains("does not apply"),
        "the re-gate must say it did not apply, got: {}",
        measurement.message
    );
}
