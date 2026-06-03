use std::{
    ffi::OsString,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use legion_desktop::{
    bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge},
    metrics::{FrameTimingRecorder, FrameTimingSummary},
    smoke::{RendererSmokeConfig, RendererSmokeReport, RendererSmokeStatus},
    workflow::DesktopLaunchConfig,
};
use legion_protocol::{BufferId, TextCoordinate};
use legion_ui::{ActiveBufferProjection, Shell};

fn approx_eq(left: f64, right: f64) {
    assert!(
        (left - right).abs() < 0.001,
        "expected {left} to be approximately {right}"
    );
}

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

#[test]
fn platform_smoke_metrics_summarize_empty_and_single_sample() {
    let empty = FrameTimingRecorder::new().summary();
    assert_eq!(empty, FrameTimingSummary::default());

    let mut recorder = FrameTimingRecorder::new();
    let start = Instant::now();
    recorder.record_input(start);
    recorder.record_paint(start + Duration::from_millis(12));
    recorder.record_frame_duration(Duration::from_millis(16));

    let summary = recorder.summary();
    assert_eq!(summary.sample_count, 1);
    approx_eq(summary.p50_input_to_paint_ms, 12.0);
    approx_eq(summary.p95_input_to_paint_ms, 12.0);
    assert_eq!(summary.frame_count, 1);
    approx_eq(summary.average_frame_ms, 16.0);
    approx_eq(summary.frame_variance_ms2, 0.0);
}

#[test]
fn platform_smoke_metrics_compute_percentiles_and_variance() {
    let mut recorder = FrameTimingRecorder::new();
    let start = Instant::now();
    for (index, millis) in [10_u64, 20, 30, 40, 50].into_iter().enumerate() {
        let input_at = start + Duration::from_millis(index as u64 * 100);
        recorder.record_input(input_at);
        recorder.record_paint(input_at + Duration::from_millis(millis));
    }
    for millis in [10_u64, 20, 30] {
        recorder.record_frame_duration(Duration::from_millis(millis));
    }

    let summary = recorder.summary();
    assert_eq!(summary.sample_count, 5);
    approx_eq(summary.p50_input_to_paint_ms, 30.0);
    approx_eq(summary.p95_input_to_paint_ms, 50.0);
    assert_eq!(summary.frame_count, 3);
    approx_eq(summary.average_frame_ms, 20.0);
    approx_eq(summary.frame_variance_ms2, 66.666666);
}

#[test]
fn platform_smoke_report_markdown_contains_required_fields() {
    let report = RendererSmokeReport {
        command: "cargo run -p legion-desktop -- --smoke".to_string(),
        status: RendererSmokeStatus::Passed,
        workspace: PathBuf::from("."),
        file: Some("Cargo.toml".to_string()),
        duration_ms: 1500,
        timing: FrameTimingSummary {
            sample_count: 1,
            p50_input_to_paint_ms: 1.0,
            p95_input_to_paint_ms: 2.0,
            frame_count: 3,
            average_frame_ms: 16.0,
            frame_variance_ms2: 0.5,
        },
        focus_smoke: "os-observed focused".to_string(),
        menu_smoke: "projection command surface present".to_string(),
        shortcut_smoke: "adapter shortcut targets projected".to_string(),
        clipboard_smoke: "adapter-path passed".to_string(),
        ime_smoke: "adapter-path passed".to_string(),
        theme_smoke: "adapter theme defaults available".to_string(),
        high_dpi_smoke: "not observed".to_string(),
        focus_traversal_smoke: "projection focus traversal nodes 2; viewport focused".to_string(),
        file_dialog_smoke: "adapter-path passed".to_string(),
        accessibility_smoke: "not observed".to_string(),
        accessibility_tree_smoke:
            "metadata-only projection accessibility nodes 2; OS tree not observed".to_string(),
        accessibility_projection_node_count: 2,
        large_file_degraded_status: "not observed".to_string(),
        bounded_search_status: "not observed".to_string(),
        full_text_projection_status: "not observed".to_string(),
        errors: Vec::new(),
    };
    let markdown = report.to_markdown();

    for field in [
        "p50_input_to_paint_ms",
        "p95_input_to_paint_ms",
        "frame_variance_ms2",
        "focus_smoke",
        "menu_smoke",
        "shortcut_smoke",
        "clipboard_smoke",
        "ime_smoke",
        "theme_smoke",
        "high_dpi_smoke",
        "focus_traversal_smoke",
        "file_dialog_smoke",
        "accessibility_smoke",
        "accessibility_tree_smoke",
        "accessibility_projection_node_count",
        "large_file_degraded_status",
        "bounded_search_status",
        "full_text_projection_status",
    ] {
        assert!(markdown.contains(field), "missing {field}");
    }
}

#[test]
fn platform_smoke_report_writes_evidence_file() {
    let root = std::env::temp_dir().join(format!(
        "legion_desktop_platform_smoke_{}",
        std::process::id()
    ));
    let evidence = root.join("evidence.md");
    let report = RendererSmokeReport::blocked(
        "cargo run -p legion-desktop -- --smoke".to_string(),
        &DesktopLaunchConfig::new(PathBuf::from("."), Some("Cargo.toml".to_string())),
        &RendererSmokeConfig::new(1500, evidence.clone()).expect("valid smoke config"),
        "blocked for test",
    );

    report
        .write_evidence(&evidence)
        .expect("evidence should be written");
    let contents = fs::read_to_string(&evidence).expect("evidence should be readable");
    assert!(contents.contains("status: blocked"));
    assert!(contents.contains("blocked for test"));

    if root.starts_with(std::env::temp_dir())
        && root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("legion_desktop_platform_smoke_"))
    {
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn platform_smoke_launch_config_parses_smoke_flags() {
    let config = DesktopLaunchConfig::from_args([
        OsString::from("--smoke"),
        OsString::from("--workspace"),
        OsString::from("."),
        OsString::from("--file"),
        OsString::from("Cargo.toml"),
        OsString::from("--duration-ms"),
        OsString::from("1500"),
        OsString::from("--evidence"),
        OsString::from("plans/evidence/gui-productization/phase-2-renderer-smoke.md"),
    ])
    .expect("smoke flags should parse");

    let smoke = config.smoke.expect("smoke config should be present");
    assert_eq!(config.workspace_root, PathBuf::from("."));
    assert_eq!(config.initial_file.as_deref(), Some("Cargo.toml"));
    assert_eq!(smoke.duration_ms, 1500);
    assert_eq!(
        smoke.evidence_path,
        PathBuf::from("plans/evidence/gui-productization/phase-2-renderer-smoke.md")
    );
}

#[test]
fn platform_smoke_adapter_paths_route_without_metrics_payloads() {
    let mut snapshot = Shell::empty("Smoke").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        buffer_id: Some(BufferId(42)),
        ..ActiveBufferProjection::empty()
    };
    let bridge = DesktopCommandBridge::new();
    let at = coord(0, 0, 0);

    assert!(matches!(
        bridge.translate(
            DesktopAction::ClipboardPaste {
                text: "clipboard".to_string(),
                at,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(_)
    ));
    assert!(matches!(
        bridge.translate(
            DesktopAction::ImeCommit {
                text: "ime".to_string(),
                at,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(_)
    ));
    assert!(matches!(
        bridge.translate(
            DesktopAction::OpenPathDialogSelected("Cargo.toml".to_string()),
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(_)
    ));

    let source = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/metrics.rs"))
        .expect("metrics source should be readable");
    assert!(!source.contains("pub payload"));
    assert!(!source.contains("pub text"));
    assert!(!source.contains("String,"));
}
