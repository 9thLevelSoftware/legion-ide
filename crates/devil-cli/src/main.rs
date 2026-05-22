//! Devil CLI: diagnostics, index commands, repair tools, headless tests.

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use devil_storage::FileBackedStorage;

const PHASE_GATE_COMMANDS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo deny check",
];

#[derive(Debug, Parser)]
#[command(author, version, about = "Devil IDE diagnostics and setup helper")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the commands required by the repository phase gates.
    PhaseGates {
        /// Shell syntax to print.
        #[arg(long, value_enum, default_value_t = ShellSyntax::PowerShell)]
        shell: ShellSyntax,
    },
    /// Run static repository diagnostics without activating future runtimes.
    Doctor {
        /// Workspace root to inspect.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Open file-backed storage and verify corruption quarantine behavior.
    StorageCheck {
        /// Storage JSON path to open or create.
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ShellSyntax {
    /// PowerShell syntax for Windows developer workstations.
    PowerShell,
    /// POSIX shell syntax for CI and Unix-like workstations.
    Sh,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command.unwrap_or(Command::Doctor {
        workspace: PathBuf::from("."),
    }) {
        Command::PhaseGates { shell } => print_phase_gates(shell),
        Command::Doctor { workspace } => run_doctor(workspace),
        Command::StorageCheck { path } => run_storage_check(path),
    }
}

fn print_phase_gates(shell: ShellSyntax) -> Result<()> {
    println!("# Devil IDE phase gates");
    println!("# Plan Phase 0: governance and CI truth lock");
    for command in PHASE_GATE_COMMANDS {
        match shell {
            ShellSyntax::PowerShell => println!("{command}"),
            ShellSyntax::Sh => println!("{command}"),
        }
    }
    Ok(())
}

fn run_doctor(workspace: PathBuf) -> Result<()> {
    let workspace = fs::canonicalize(&workspace)
        .with_context(|| format!("resolve workspace `{}`", workspace.display()))?;
    let mut issues = Vec::new();

    // Plan Phase 0: required governance and phase evidence must exist before runtime expansion.
    require_file(&workspace, "Cargo.toml", &mut issues);
    require_file(&workspace, "AGENTS.md", &mut issues);
    require_file(&workspace, "plans/dependency-policy.md", &mut issues);
    require_file(&workspace, "plans/phase-status-ledger.md", &mut issues);
    require_file(
        &workspace,
        "plans/evidence/phase-3/predictive-semantic-fabric.md",
        &mut issues,
    );
    require_file(&workspace, ".github/workflows/ci.yml", &mut issues);

    // Plan Phase 0: CI should mirror the local phase-gate command set.
    let ci = read_optional(&workspace, ".github/workflows/ci.yml", &mut issues);
    if let Some(ci) = ci {
        for command in PHASE_GATE_COMMANDS {
            if !ci_contains_gate(&ci, command) {
                issues.push(format!("CI does not contain required gate `{command}`"));
            }
        }
    }

    // Plan Phase 0/4: phase status remains conservative until evidence exists.
    let ledger = read_optional(&workspace, "plans/phase-status-ledger.md", &mut issues);
    if let Some(ledger) = ledger {
        require_text(
            &ledger,
            "Phase 0",
            "phase ledger names Phase 0",
            &mut issues,
        );
        require_text(
            &ledger,
            "Phase 1",
            "phase ledger names Phase 1",
            &mut issues,
        );
        require_text(
            &ledger,
            "Phase 2",
            "phase ledger names Phase 2",
            &mut issues,
        );
        require_text(
            &ledger,
            "Partially accepted",
            "Phase 2 remains partial",
            &mut issues,
        );
        require_text(
            &ledger,
            "Phase 3",
            "phase ledger names Phase 3",
            &mut issues,
        );
        require_text(
            &ledger,
            "Not accepted",
            "Phase 3 remains not accepted",
            &mut issues,
        );
        require_text(
            &ledger,
            "Future-gated",
            "future phases remain gated",
            &mut issues,
        );
    }

    let phase3 = read_optional(
        &workspace,
        "plans/evidence/phase-3/predictive-semantic-fabric.md",
        &mut issues,
    );
    if let Some(phase3) = phase3 {
        require_text(
            &phase3,
            "Phase 3 acceptance: Not accepted.",
            "Phase 3 acceptance is still gated",
            &mut issues,
        );
        require_text(
            &phase3,
            "LSP supervision acceptance: Not accepted.",
            "LSP supervision is still gated",
            &mut issues,
        );
        require_text(
            &phase3,
            "vector indexing",
            "vector indexing deferral is documented",
            &mut issues,
        );
    }

    // Plan Phase 2 and future phases: placeholder runtime crates must stay inert.
    require_placeholder_runtime_inert(&workspace, "crates/devil-agent/src/lib.rs", &mut issues);
    require_placeholder_runtime_inert(&workspace, "crates/devil-tracker/src/lib.rs", &mut issues);
    require_placeholder_runtime_inert(&workspace, "crates/devil-memory/src/lib.rs", &mut issues);

    if issues.is_empty() {
        println!("Devil CLI doctor: OK");
        println!("Workspace: {}", workspace.display());
        println!("Next setup command: cargo run -p devil-cli -- phase-gates");
        return Ok(());
    }

    eprintln!("Devil CLI doctor found {} issue(s):", issues.len());
    for issue in issues {
        eprintln!("- {issue}");
    }
    bail!("doctor checks failed")
}

fn run_storage_check(path: PathBuf) -> Result<()> {
    // Plan Phase 3/9: durable metadata storage should open, initialize, or quarantine corruption.
    let storage = FileBackedStorage::open(&path)
        .with_context(|| format!("open file-backed storage `{}`", path.display()))?;
    drop(storage);
    println!("Storage check: OK");
    println!("Path: {}", path.display());
    Ok(())
}

fn require_file(workspace: &std::path::Path, relative: &str, issues: &mut Vec<String>) {
    let path = workspace.join(relative);
    if !path.is_file() {
        issues.push(format!("required file `{relative}` is missing"));
    }
}

fn read_optional(
    workspace: &std::path::Path,
    relative: &str,
    issues: &mut Vec<String>,
) -> Option<String> {
    let path = workspace.join(relative);
    match fs::read_to_string(&path) {
        Ok(contents) => Some(contents),
        Err(err) => {
            issues.push(format!("unable to read `{relative}`: {err}"));
            None
        }
    }
}

fn require_text(contents: &str, needle: &str, label: &str, issues: &mut Vec<String>) {
    if !contents.contains(needle) {
        issues.push(format!("missing marker for {label}: `{needle}`"));
    }
}

fn require_placeholder_runtime_inert(
    workspace: &std::path::Path,
    relative: &str,
    issues: &mut Vec<String>,
) {
    let Some(contents) = read_optional(workspace, relative, issues) else {
        return;
    };

    let implementation_lines = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("//!"))
        .filter(|line| !line.starts_with("#![warn(missing_docs)]"))
        .count();

    if implementation_lines > 0 {
        issues.push(format!(
            "placeholder runtime `{relative}` contains implementation code before activation gates"
        ));
    }
}

fn ci_contains_gate(ci: &str, command: &str) -> bool {
    if ci.contains(command) {
        return true;
    }

    command == "cargo deny check"
        && (ci.contains("cargo-deny-action") || ci.contains("EmbarkStudios/cargo-deny-action"))
}
