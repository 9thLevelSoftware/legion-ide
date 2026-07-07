//! Update-drill subprocess orchestration (M12 / PKT-UPDATER).
//!
//! `xtask update-drill` spawns the `upd-drill` binary from `legion-app` as
//! a subprocess (xtask cannot depend on `legion-app` per the dependency policy).
//! The subprocess writes `target/update-drill/update_drill_report.toml`; this
//! command prints a summary and forwards the subprocess exit code.
//!
//! The binary is named `upd-drill` (not `update_drill`) to avoid Windows'
//! installer-detection heuristic, which auto-elevates any executable whose name
//! contains the substring "update" — preventing it from launching without UAC.
//!
//! This is the **19th standing gate**.

use std::{path::Path, process};

/// Options for the `update-drill` subcommand.
#[derive(Debug)]
pub struct UpdateDrillOptions {
    /// Output directory for the evidence TOML (default: `target/update-drill`).
    pub out_dir: String,
}

impl Default for UpdateDrillOptions {
    fn default() -> Self {
        Self {
            out_dir: "target/update-drill".to_string(),
        }
    }
}

/// Run `update-drill`: compile and spawn the `update_drill` legion-app binary.
///
/// Returns the exit code the subprocess produced (0 = all steps passed,
/// 1 = one or more steps failed, 2 = argument/setup error).
pub fn run_update_drill(workspace_root: &Path, opts: &UpdateDrillOptions) -> i32 {
    let cargo_args: Vec<String> = vec![
        "run".to_string(),
        "--jobs".to_string(),
        "4".to_string(),
        "-p".to_string(),
        "legion-app".to_string(),
        "--bin".to_string(),
        "upd-drill".to_string(),
        "--no-default-features".to_string(),
        "--".to_string(),
        "--out-dir".to_string(),
        opts.out_dir.clone(),
    ];

    eprintln!(
        "update-drill: spawning subprocess: cargo {}",
        cargo_args.join(" ")
    );

    let status = process::Command::new("cargo")
        .current_dir(workspace_root)
        .args(&cargo_args)
        .status();

    match status {
        Ok(s) => {
            let code = s.code().unwrap_or(1);
            if code == 0 {
                eprintln!(
                    "update-drill: subprocess passed; report at {}/update_drill_report.toml",
                    opts.out_dir
                );
            } else {
                eprintln!(
                    "update-drill: subprocess exited with code {code}; check {}/update_drill_report.toml",
                    opts.out_dir
                );
            }
            code
        }
        Err(err) => {
            eprintln!("update-drill: failed to spawn cargo: {err}");
            1
        }
    }
}
