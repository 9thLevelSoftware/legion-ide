//! GAP-01.1: package an unsigned desktop layout and run windowed GUI E2E.
//!
//! xtask may not depend on `legion-desktop`. This command builds the desktop
//! binary, copies it plus legal files into `target/windowed-gui/package/`, and
//! spawns that binary with `--windowed-e2e` (`eframe::run_native`, not
//! `--beta-smoke`, not AppComposition `golden-path-5`).

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const LEGAL_NOTICE_FILES: &[(&str, &str)] = &[
    ("LICENSE", "LICENSE"),
    ("docs/PRIVACY.md", "PRIVACY.md"),
    ("THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md"),
];

/// Options for `xtask windowed-gui-e2e`.
#[derive(Debug)]
pub struct WindowedGuiE2eOptions {
    /// Output directory for the package layout and report.
    pub out_dir: String,
    /// Build the desktop binary with `--release`.
    pub release: bool,
    /// Optional directory to copy the report into after a successful run.
    pub record_evidence: Option<String>,
}

impl Default for WindowedGuiE2eOptions {
    fn default() -> Self {
        Self {
            out_dir: "target/windowed-gui".to_string(),
            release: false,
            record_evidence: None,
        }
    }
}

/// Build an unsigned package layout and run `--windowed-e2e` on that binary.
///
/// Returns the subprocess exit code (0 = passed).
pub fn run_windowed_gui_e2e(workspace_root: &Path, opts: &WindowedGuiE2eOptions) -> i32 {
    let out_dir = workspace_root.join(&opts.out_dir);
    let package_dir = out_dir.join("package");
    let workspace_dir = out_dir.join("workspace");
    let report_path = out_dir.join("report.toml");
    let notes_path = workspace_dir.join("notes.txt");

    if let Err(error) = fs::create_dir_all(&package_dir) {
        eprintln!("windowed-gui-e2e: cannot create package dir: {error}");
        return 2;
    }
    if let Err(error) = fs::create_dir_all(&workspace_dir) {
        eprintln!("windowed-gui-e2e: cannot create fixture workspace: {error}");
        return 2;
    }
    if let Err(error) = fs::write(&notes_path, "seed\n") {
        eprintln!("windowed-gui-e2e: cannot write fixture file: {error}");
        return 2;
    }

    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(workspace_root)
        .arg("build")
        .arg("-p")
        .arg("legion-desktop");
    if opts.release {
        cargo.arg("--release");
    }
    eprintln!(
        "windowed-gui-e2e: building legion-desktop ({})",
        if opts.release { "release" } else { "debug" }
    );
    match cargo.status() {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("windowed-gui-e2e: cargo build failed with {status}");
            return status.code().unwrap_or(1);
        }
        Err(error) => {
            eprintln!("windowed-gui-e2e: failed to spawn cargo: {error}");
            return 2;
        }
    }

    let profile = if opts.release { "release" } else { "debug" };
    let exe_name = if cfg!(windows) {
        "legion-desktop.exe"
    } else {
        "legion-desktop"
    };
    let built = workspace_root.join("target").join(profile).join(exe_name);
    let packaged = package_dir.join(exe_name);
    if let Err(error) = fs::copy(&built, &packaged) {
        eprintln!(
            "windowed-gui-e2e: cannot copy {} -> {}: {error}",
            built.display(),
            packaged.display()
        );
        return 2;
    }
    for (source, dest) in LEGAL_NOTICE_FILES {
        let from = workspace_root.join(source);
        let to = package_dir.join(dest);
        if let Err(error) = fs::copy(&from, &to) {
            eprintln!(
                "windowed-gui-e2e: cannot copy legal file {} -> {}: {error}",
                from.display(),
                to.display()
            );
            return 2;
        }
    }

    let git_sha = Command::new("git")
        .current_dir(workspace_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    eprintln!(
        "windowed-gui-e2e: launching {} --windowed-e2e (eframe::run_native, not --beta-smoke, not golden-path-5)",
        packaged.display()
    );
    let mut child = Command::new(&packaged);
    child
        .current_dir(workspace_root)
        .arg("--windowed-e2e")
        .arg("--workspace")
        .arg(&workspace_dir)
        .arg("--file")
        .arg(&notes_path)
        .arg("--report")
        .arg(&report_path)
        .env("LEGION_WINDOWED_E2E_GIT_SHA", &git_sha)
        .env(
            "LEGION_WINDOWED_E2E_PACKAGE_DIR",
            package_dir.display().to_string(),
        )
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let code = match child.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("windowed-gui-e2e: failed to spawn packaged binary: {error}");
            return 2;
        }
    };

    if !report_path.is_file() {
        eprintln!(
            "windowed-gui-e2e: report was not written: {}",
            report_path.display()
        );
        return 1;
    }
    let report = match fs::read_to_string(&report_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("windowed-gui-e2e: cannot read report: {error}");
            return 1;
        }
    };
    if !report.contains("not_golden_path_5 = true") || !report.contains("not_beta_smoke = true") {
        eprintln!("windowed-gui-e2e: report is missing GAP-01.1 identity markers");
        return 1;
    }
    if code == 0 && !report.contains("window_created = true") {
        eprintln!("windowed-gui-e2e: exit 0 without window_created = true is not GAP-01.1");
        return 1;
    }
    if let Some(dest_dir) = &opts.record_evidence {
        let dest = PathBuf::from(dest_dir);
        let dest_file = if dest.is_dir() || dest_dir.ends_with('/') || dest_dir.ends_with('\\') {
            dest.join("gap-01-1-windowed-gui-e2e.toml")
        } else {
            dest
        };
        if let Some(parent) = dest_file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(error) = fs::copy(&report_path, &dest_file) {
            eprintln!(
                "windowed-gui-e2e: cannot copy report to {}: {error}",
                dest_file.display()
            );
            return 1;
        }
    }
    if code == 0 {
        println!(
            "windowed-gui-e2e: passed; report written to {}",
            report_path.display()
        );
    } else {
        eprintln!(
            "windowed-gui-e2e: exited {code}; see {}",
            report_path.display()
        );
    }
    code
}
