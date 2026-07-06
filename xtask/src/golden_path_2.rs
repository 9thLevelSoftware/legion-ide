//! GP-2 golden-path smoke orchestration.
//!
//! `xtask golden-path-2` spawns the `golden_path_2` binary from `legion-app`
//! as a subprocess (design choice: xtask may not depend on legion-app per the
//! dependency policy, so the runner lives in `crates/legion-app/src/bin/` and
//! is invoked via `cargo run`).  The subprocess writes
//! `target/golden-path/gp2_report.toml`; this command prints a summary and
//! forwards the subprocess exit code.
//!
//! Key difference from GP-1: the `cargo run` command does NOT pass
//! `--no-default-features` — GP-2 needs the `ai` feature (included by default).

use std::{path::Path, process};

/// Options for the `golden-path-2` subcommand.
#[derive(Debug)]
pub struct GoldenPath2Options {
    /// Path to the fixture directory (default: `fixtures/gp1-rust`).
    pub fixture_dir: String,
    /// Output directory for the evidence TOML (default: `target/golden-path`).
    pub out_dir: String,
    /// If `Some`, also copy the evidence TOML to this path after a successful run.
    pub record_evidence: Option<String>,
}

impl Default for GoldenPath2Options {
    fn default() -> Self {
        Self {
            fixture_dir: "fixtures/gp1-rust".to_string(),
            out_dir: "target/golden-path".to_string(),
            record_evidence: None,
        }
    }
}

/// Run `golden-path-2`: compile and spawn the legion-app binary subprocess.
///
/// Returns the exit code the subprocess produced (0 = all steps passed or
/// skipped, 1 = one or more steps failed, 2 = argument/setup error).
pub fn run_golden_path_2(workspace_root: &Path, opts: &GoldenPath2Options) -> i32 {
    // Resolve paths relative to the workspace root.
    let fixture_dir = workspace_root.join(&opts.fixture_dir);
    if !fixture_dir.is_dir() {
        eprintln!(
            "golden-path-2: fixture directory not found: {}",
            fixture_dir.display()
        );
        return 2;
    }

    // NOTE: No --no-default-features here — GP-2 requires the `ai` feature
    // (included in default features).  Compare with GP-1 which passes
    // `--no-default-features` explicitly.
    let mut cargo_args: Vec<String> = vec![
        "run".to_string(),
        "--jobs".to_string(),
        "4".to_string(),
        "-p".to_string(),
        "legion-app".to_string(),
        "--bin".to_string(),
        "golden_path_2".to_string(),
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
        "golden-path-2: spawning subprocess: cargo {}",
        cargo_args.join(" ")
    );

    let status = process::Command::new("cargo")
        .current_dir(workspace_root)
        .args(&cargo_args)
        .status();

    match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(err) => {
            eprintln!("golden-path-2: failed to spawn cargo: {err}");
            1
        }
    }
}
