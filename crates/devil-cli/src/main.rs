//! Devil CLI: diagnostics, index commands, repair tools, headless tests.

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use devil_storage::FileBackedStorage;
use serde_json::Value;

const PHASE_GATE_COMMANDS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo deny check",
];

const PHASE0_EVIDENCE_FILES: &[&str] = &[
    "plans/evidence/phase-0/native-shell-proof-summary.md",
    "plans/evidence/phase-0/platform-boundary-api-map.md",
    "plans/evidence/phase-0/text-index-stress-baseline.md",
];

const PHASE3_REQUIRED_ARTIFACTS: &[&str] = &[
    "semantic-fabric-architecture-map.md",
    "index-dependency-boundary.txt",
    "repository-discovery-ignore-fingerprint.md",
    "lexical-symbol-map-tests.txt",
    "tree-sitter-cache-tests.txt",
    "normalized-graph-contract-tests.txt",
    "semantic-query-api-tests.txt",
    "lsp-supervision-tests.txt",
    "proposal-routing-regression.txt",
    "privacy-redaction-audit.md",
    "vector-deferral-audit.md",
];

const STORAGE_FORBIDDEN_MARKERS: &[&str] = &[
    "raw_source",
    "source_text",
    "full_source",
    "full_text",
    "raw_prompt",
    "prompt_body",
    "terminal_output",
    "provider_payload",
    "secret",
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
    /// Check phase evidence artifacts without changing repository state.
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
    /// Check future runtime activation gates.
    Activation {
        #[command(subcommand)]
        command: ActivationCommand,
    },
    /// Read-only storage diagnostics.
    Storage {
        #[command(subcommand)]
        command: StorageCommand,
    },
    /// Print setup status for roadmap phases and runnable commands.
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
}

#[derive(Debug, Subcommand)]
enum EvidenceCommand {
    /// Check evidence for a phase gate.
    Check {
        /// Phase evidence set to check.
        #[arg(long, value_enum)]
        phase: EvidencePhase,
        /// Workspace root to inspect.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EvidencePhase {
    /// Phase 0 accepted foundation evidence.
    Phase0,
    /// Phase 3 scaffold or acceptance evidence.
    Phase3,
}

#[derive(Debug, Subcommand)]
enum ActivationCommand {
    /// Check that future-gated placeholder runtimes remain inert.
    Check {
        /// Workspace root to inspect.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum StorageCommand {
    /// Inspect persisted storage JSON without creating, migrating, or quarantining it.
    Inspect {
        /// Persisted storage JSON path.
        path: PathBuf,
        /// Require read-only behavior. This flag is intentionally explicit for operator clarity.
        #[arg(long)]
        read_only: bool,
    },
    /// Scan persisted storage JSON for forbidden raw payload markers.
    PrivacyAudit {
        /// Persisted storage JSON path.
        path: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum SetupCommand {
    /// Print roadmap setup status and next safe commands.
    Status {
        /// Workspace root to inspect.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
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
        Command::Evidence { command } => match command {
            EvidenceCommand::Check { phase, workspace } => run_evidence_check(workspace, phase),
        },
        Command::Activation { command } => match command {
            ActivationCommand::Check { workspace } => run_activation_check(workspace),
        },
        Command::Storage { command } => match command {
            StorageCommand::Inspect { path, read_only } => run_storage_inspect(path, read_only),
            StorageCommand::PrivacyAudit { path } => run_storage_privacy_audit(path),
        },
        Command::Setup { command } => match command {
            SetupCommand::Status { workspace } => run_setup_status(workspace),
        },
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

fn run_evidence_check(workspace: PathBuf, phase: EvidencePhase) -> Result<()> {
    let workspace = canonical_workspace(workspace)?;
    let mut issues = Vec::new();
    match phase {
        EvidencePhase::Phase0 => check_phase0_evidence(&workspace, &mut issues),
        EvidencePhase::Phase3 => check_phase3_evidence(&workspace, &mut issues),
    }
    finish_issue_report("Evidence check", &workspace, issues)
}

fn run_activation_check(workspace: PathBuf) -> Result<()> {
    let workspace = canonical_workspace(workspace)?;
    let mut issues = Vec::new();
    require_placeholder_runtime_inert(&workspace, "crates/devil-agent/src/lib.rs", &mut issues);
    require_placeholder_runtime_inert(&workspace, "crates/devil-tracker/src/lib.rs", &mut issues);
    require_placeholder_runtime_inert(&workspace, "crates/devil-memory/src/lib.rs", &mut issues);
    let policy = read_optional(&workspace, "plans/dependency-policy.md", &mut issues);
    if let Some(policy) = policy {
        require_text(
            &policy,
            "Runtime Surface Activation Gates",
            "runtime activation gates are documented",
            &mut issues,
        );
        require_text(
            &policy,
            "remain ADR-gated",
            "placeholder runtime surfaces remain ADR-gated",
            &mut issues,
        );
    }
    finish_issue_report("Activation check", &workspace, issues)
}

fn run_storage_inspect(path: PathBuf, read_only: bool) -> Result<()> {
    if !read_only {
        bail!("storage inspect requires --read-only to avoid accidental migration or quarantine")
    }
    let value = read_storage_json_read_only(&path)?;
    println!("Storage inspect: OK");
    println!("Path: {}", path.display());
    println!(
        "schema_version: {}",
        value
            .get("schema_version")
            .and_then(Value::as_u64)
            .map_or_else(|| "unknown".to_string(), |version| version.to_string())
    );
    for key in [
        "protocol_proposal_audit",
        "protocol_event_metadata",
        "protocol_assisted_ai_audit",
        "protocol_delegated_task_audit_linkage",
        "semantic_metadata",
        "semantic_tombstones",
    ] {
        println!("{key}: {}", json_record_count(value.get(key)));
    }
    Ok(())
}

fn run_storage_privacy_audit(path: PathBuf) -> Result<()> {
    let value = read_storage_json_read_only(&path)?;
    let mut findings = Vec::new();
    find_forbidden_storage_markers(&value, "$", &mut findings);
    if findings.is_empty() {
        println!("Storage privacy audit: OK");
        println!("Path: {}", path.display());
        return Ok(());
    }

    eprintln!("Storage privacy audit found {} issue(s):", findings.len());
    for finding in findings {
        eprintln!("- {finding}");
    }
    bail!("storage privacy audit failed")
}

fn run_setup_status(workspace: PathBuf) -> Result<()> {
    let workspace = canonical_workspace(workspace)?;
    let mut issues = Vec::new();
    require_file(&workspace, "plans/phase-status-ledger.md", &mut issues);
    require_file(&workspace, ".github/workflows/ci.yml", &mut issues);
    require_file(&workspace, "scripts/run-phase-gates.ps1", &mut issues);
    require_file(&workspace, "scripts/run-phase-gates.sh", &mut issues);

    println!("Devil IDE setup status");
    println!("Workspace: {}", workspace.display());
    if let Some(ledger) = read_optional(&workspace, "plans/phase-status-ledger.md", &mut issues) {
        for marker in [
            "Phase 0 — Foundation and freeze",
            "Phase 1 — Editor and text substrate",
            "Phase 2 — Proposal mutation substrate",
            "Phase 3 — Semantic fabric and LSP supervision",
            "Phases 4–8 — AI, plugins, collaboration, remote, hardening",
        ] {
            if let Some(line) = ledger.lines().find(|line| line.contains(marker)) {
                println!("{line}");
            }
        }
    }
    println!("Next safe commands:");
    println!("cargo run -p devil-cli -- doctor");
    println!("cargo run -p devil-cli -- evidence check --phase phase0");
    println!("cargo run -p devil-cli -- evidence check --phase phase3");
    println!("pwsh ./scripts/run-phase-gates.ps1");
    finish_issue_report("Setup status", &workspace, issues)
}

fn require_file(workspace: &std::path::Path, relative: &str, issues: &mut Vec<String>) {
    let path = workspace.join(relative);
    if !path.is_file() {
        issues.push(format!("required file `{relative}` is missing"));
    }
}

fn canonical_workspace(workspace: PathBuf) -> Result<PathBuf> {
    fs::canonicalize(&workspace)
        .with_context(|| format!("resolve workspace `{}`", workspace.display()))
}

fn check_phase0_evidence(workspace: &std::path::Path, issues: &mut Vec<String>) {
    for relative in PHASE0_EVIDENCE_FILES {
        require_file(workspace, relative, issues);
    }
    if let Some(ledger) = read_optional(workspace, "plans/phase-status-ledger.md", issues) {
        require_text(&ledger, "Phase 0", "Phase 0 ledger entry", issues);
        require_text(&ledger, "Accepted", "Phase 0 accepted status", issues);
    }
}

fn check_phase3_evidence(workspace: &std::path::Path, issues: &mut Vec<String>) {
    let relative = "plans/evidence/phase-3/predictive-semantic-fabric.md";
    let Some(phase3) = read_optional(workspace, relative, issues) else {
        return;
    };
    for artifact in PHASE3_REQUIRED_ARTIFACTS {
        require_text(
            &phase3,
            artifact,
            &format!("Phase 3 required artifact `{artifact}` is listed"),
            issues,
        );
    }
    if phase3.contains("Phase 3 acceptance: Accepted.") {
        for artifact in PHASE3_REQUIRED_ARTIFACTS {
            require_file(
                workspace,
                &format!("plans/evidence/phase-3/{artifact}"),
                issues,
            );
        }
        if phase3.contains("- [ ]") {
            issues.push(
                "Phase 3 is marked accepted but checklist still has unchecked items".to_string(),
            );
        }
    } else {
        require_text(
            &phase3,
            "Phase 3 acceptance: Not accepted.",
            "Phase 3 remains gated until evidence is complete",
            issues,
        );
        require_text(
            &phase3,
            "LSP supervision acceptance: Not accepted.",
            "LSP supervision remains gated until evidence is complete",
            issues,
        );
    }
}

fn finish_issue_report(
    label: &str,
    workspace: &std::path::Path,
    issues: Vec<String>,
) -> Result<()> {
    if issues.is_empty() {
        println!("{label}: OK");
        println!("Workspace: {}", workspace.display());
        return Ok(());
    }
    eprintln!("{label} found {} issue(s):", issues.len());
    for issue in issues {
        eprintln!("- {issue}");
    }
    bail!("{label} failed")
}

fn read_storage_json_read_only(path: &std::path::Path) -> Result<Value> {
    let body = fs::read_to_string(path)
        .with_context(|| format!("read storage JSON `{}`", path.display()))?;
    serde_json::from_str(&body).with_context(|| {
        format!(
            "parse storage JSON `{}` without modifying it",
            path.display()
        )
    })
}

fn json_record_count(value: Option<&Value>) -> usize {
    match value {
        Some(Value::Array(items)) => items.len(),
        Some(Value::Object(items)) => items.len(),
        Some(Value::Null) | None => 0,
        Some(_) => 1,
    }
}

fn find_forbidden_storage_markers(value: &Value, path: &str, findings: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                let normalized = key.to_ascii_lowercase();
                if STORAGE_FORBIDDEN_MARKERS
                    .iter()
                    .any(|marker| normalized.contains(marker))
                {
                    findings.push(format!("forbidden storage marker in key `{child_path}`"));
                }
                find_forbidden_storage_markers(child, &child_path, findings);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                find_forbidden_storage_markers(child, &format!("{path}[{index}]"), findings);
            }
        }
        Value::String(text) => {
            let normalized = text.to_ascii_lowercase();
            if STORAGE_FORBIDDEN_MARKERS
                .iter()
                .any(|marker| normalized.contains(marker))
            {
                findings.push(format!("forbidden storage marker in string value `{path}`"));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
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
