//! B12: live DAP cargo prebuild helpers + real cargo smoke.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use legion_app::{live_dap_should_prebuild, run_live_dap_prebuild};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn prebuild_skipped_for_fake_or_empty_args() {
    assert!(!live_dap_should_prebuild(true, &["build".into()]));
    assert!(!live_dap_should_prebuild(false, &[]));
    assert!(live_dap_should_prebuild(
        false,
        &["build".into(), "--bin".into(), "x".into()]
    ));
}

#[test]
fn prebuild_runs_cargo_build_in_temp_workspace() {
    let root = std::env::temp_dir().join(format!(
        "legion-prebuild-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("mkdir");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "prebuild_probe"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("toml");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");

    let args = vec![
        "build".to_string(),
        "--package".to_string(),
        "prebuild_probe".to_string(),
        "--bin".to_string(),
        "prebuild_probe".to_string(),
    ];
    let note = run_live_dap_prebuild(
        root.to_str().expect("utf8 path"),
        &args,
        Duration::from_secs(120),
    )
    .expect("cargo prebuild should succeed");
    assert!(
        note.contains("cargo build") && note.contains("ok"),
        "unexpected note: {note}"
    );

    let bin = if cfg!(windows) {
        PathBuf::from("target/debug/prebuild_probe.exe")
    } else {
        PathBuf::from("target/debug/prebuild_probe")
    };
    assert!(
        root.join(&bin).is_file(),
        "expected built binary at {}",
        root.join(bin).display()
    );

    fs::remove_dir_all(root).ok();
}
