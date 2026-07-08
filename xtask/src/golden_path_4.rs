//! GP-4 golden-path smoke orchestration.
//!
//! `xtask golden-path-4` spawns the `golden_path_4` binary from `legion-app`
//! as a subprocess (design choice: xtask may not depend on legion-app per the
//! dependency policy, so the runner lives in `crates/legion-app/src/bin/` and
//! is invoked via `cargo run`).  The subprocess writes
//! `target/golden-path/gp4_report.toml`; this command prints a summary and
//! forwards the subprocess exit code.
//!
//! Key differences from GP-3:
//! - Binary name: `golden_path_4`
//! - Passes `--features test-helpers` for the scripted workflow harness.
//! - Does NOT pass `--no-default-features` - GP-4 needs the `ai` feature.

use std::{path::Path, process};

/// Options for the `golden-path-4` subcommand.
#[derive(Debug)]
pub struct GoldenPath4Options {
    /// Path to the fixture directory (default: `fixtures/gp1-rust`).
    pub fixture_dir: String,
    /// Output directory for the evidence TOML (default: `target/golden-path`).
    pub out_dir: String,
    /// If `Some`, also copy the evidence TOML to this path after a successful run.
    pub record_evidence: Option<String>,
}

impl Default for GoldenPath4Options {
    fn default() -> Self {
        Self {
            fixture_dir: "fixtures/gp1-rust".to_string(),
            out_dir: "target/golden-path".to_string(),
            record_evidence: None,
        }
    }
}

/// Run `golden-path-4`: compile and spawn the legion-app binary subprocess.
///
/// Returns the exit code the subprocess produced (0 = all steps passed or
/// skipped, 1 = one or more steps failed, 2 = argument/setup error).
pub fn run_golden_path_4(workspace_root: &Path, opts: &GoldenPath4Options) -> i32 {
    // Resolve paths relative to the workspace root.
    let fixture_dir = workspace_root.join(&opts.fixture_dir);
    if !fixture_dir.is_dir() {
        eprintln!(
            "golden-path-4: fixture directory not found: {}",
            fixture_dir.display()
        );
        return 2;
    }

    // NOTE: No --no-default-features here - GP-4 requires the `ai` feature
    // (included in default features).  --features test-helpers is required for
    // the scripted workflow harness.
    let mut cargo_args: Vec<String> = vec![
        "run".to_string(),
        "--jobs".to_string(),
        "4".to_string(),
        "-p".to_string(),
        "legion-app".to_string(),
        "--bin".to_string(),
        "golden_path_4".to_string(),
        "--features".to_string(),
        "test-helpers".to_string(),
        "--".to_string(),
        "--fixture-dir".to_string(),
        fixture_dir.to_string_lossy().into_owned(),
        "--out-dir".to_string(),
        opts.out_dir.clone(),
    ];

    if let Some(ref ev_dir) = opts.record_evidence {
        cargo_args.push("--record-evidence".to_string());
        cargo_args.push(ev_dir.clone());
    }

    eprintln!(
        "golden-path-4: spawning subprocess: cargo {}",
        cargo_args.join(" ")
    );

    let status = process::Command::new("cargo")
        .current_dir(workspace_root)
        .args(&cargo_args)
        .status();

    match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(err) => {
            eprintln!("golden-path-4: failed to spawn cargo: {err}");
            1
        }
    }
}
