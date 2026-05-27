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
    "cargo run -p devil-cli -- evidence check --phase phase8",
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

const PHASE8_REQUIRED_ARTIFACTS: &[&str] = &[
    "phase-8-architecture-map.md",
    "phase-8-threat-model.md",
    "dependency-boundary.txt",
    "protocol-dto-contract-tests.txt",
    "remote-production-transport-security-tests.txt",
    "remote-agent-packaging-tests.txt",
    "terminal-runtime-policy-tests.txt",
    "terminal-pty-platform-tests.txt",
    "hosted-telemetry-consent-policy-tests.txt",
    "hosted-telemetry-failure-mode-tests.txt",
    "privacy-redaction-classifier-audit.md",
    "raw-source-retention-policy-tests.txt",
    "raw-source-retention-lifecycle-tests.txt",
    "storage-migration-recovery-tests.txt",
    "operational-health-diagnostics.txt",
    "enterprise-policy-profile-ci.txt",
    "performance-budget-tests.txt",
    "metadata-replay-drills.txt",
    "fault-drill-results.txt",
    "platform-matrix-evidence.txt",
    "release-readiness-review.md",
    "cargo-fmt-check.txt",
    "cargo-check-workspace-all-targets.txt",
    "cargo-test-workspace-all-targets.txt",
    "cargo-clippy-workspace-all-targets.txt",
    "cargo-deny-check.txt",
    "xtask-check-deps.txt",
];

const GUI_PHASE6_EVIDENCE_PATH: &str =
    "plans/evidence/gui-productization/phase-6-packaging-platform-accessibility.md";
const GUI_PHASE7_EVIDENCE_PATH: &str =
    "plans/evidence/gui-productization/phase-7-local-ide-beta.md";
const GUI_PHASE8_EVIDENCE_PATH: &str =
    "plans/evidence/gui-productization/phase-8-advanced-platform-gui-ga.md";
const GUI_PHASE6_REQUIRED_ARTIFACTS: &[&str] = &[
    "plans/evidence/gui-productization/phase-6-package-runbook.md",
    "plans/evidence/gui-productization/phase-6-packaging-smoke.md",
    "plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md",
    "plans/evidence/gui-productization/phase-6-session-diagnostics-safety.md",
    "plans/evidence/gui-productization/phase-6-workflow-smoke.md",
    "plans/evidence/gui-productization/phase-6-performance-reliability.md",
    "plans/evidence/gui-productization/phase-6-ci-parity-plan.md",
    ".planning/phases/06-packaging-platform-integration-and-accessibility/06-01-RESULT.md",
    ".planning/phases/06-packaging-platform-integration-and-accessibility/06-02-RESULT.md",
    ".planning/phases/06-packaging-platform-integration-and-accessibility/06-03-RESULT.md",
    ".planning/phases/06-packaging-platform-integration-and-accessibility/06-04-RESULT.md",
    ".planning/phases/06-packaging-platform-integration-and-accessibility/06-05-RESULT.md",
    ".planning/phases/06-packaging-platform-integration-and-accessibility/06-06-RESULT.md",
    ".planning/phases/06-packaging-platform-integration-and-accessibility/06-07-RESULT.md",
];
const GUI_PHASE6_REQUIRED_COMMAND_MARKERS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo deny check",
    "cargo test -p devil-desktop --test packaging -- --nocapture",
    "cargo test -p devil-desktop --test platform_integration -- --nocapture",
    "cargo test -p devil-desktop --test platform_smoke -- --nocapture",
    "cargo test -p devil-desktop --test session_restore -- --nocapture",
    "cargo test -p devil-desktop --test diagnostics_export -- --nocapture",
    "cargo test -p devil-cli gui_phase6 -- --nocapture",
    "scripts/package-windows.ps1 -DryRun",
    "scripts/gui-smoke.ps1 -DryRun",
    "scripts/gui-smoke.sh --dry-run",
    "cargo run -p devil-cli -- evidence check --phase gui-phase6",
];
const GUI_PHASE7_REQUIRED_ARTIFACTS: &[&str] = &[
    "plans/evidence/gui-productization/phase-7-local-workflow-smoke.md",
    "plans/evidence/gui-productization/phase-7-operational-health-diagnostics.md",
    "plans/evidence/gui-productization/phase-7-launch-runbook.md",
    "plans/evidence/gui-productization/phase-7-known-limitations.md",
    "plans/evidence/gui-productization/phase-7-release-readiness.md",
    "plans/evidence/gui-productization/phase-7-manual-beta-evidence.md",
    ".planning/phases/07-fully-functional-local-ide-beta/07-01-RESULT.md",
    ".planning/phases/07-fully-functional-local-ide-beta/07-02-RESULT.md",
    ".planning/phases/07-fully-functional-local-ide-beta/07-03-RESULT.md",
    ".planning/phases/07-fully-functional-local-ide-beta/07-04-RESULT.md",
    ".planning/phases/07-fully-functional-local-ide-beta/07-05-RESULT.md",
    "scripts/gui-smoke.ps1",
    "scripts/gui-smoke.sh",
    ".github/workflows/ci.yml",
];
const GUI_PHASE7_REQUIRED_COMMAND_MARKERS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo deny check",
    "cargo test -p devil-desktop --test beta_workflow -- --nocapture",
    "cargo test -p devil-desktop --test operational_health -- --nocapture",
    "cargo test -p devil-desktop --test diagnostics_export -- --nocapture",
    "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Beta -DryRun",
    "bash scripts/gui-smoke.sh --beta --dry-run",
    "cargo run -p devil-cli -- evidence check --phase gui-phase7",
];
const GUI_PHASE7_REQUIRED_LIMITATION_MARKERS: &[&str] = &[
    "Remote production GUI: unsupported",
    "Collaboration GUI: unsupported",
    "Plugin management GUI: unsupported",
    "Hosted provider activation: unsupported",
    "Signed installer: unsupported",
    "Cross-platform parity: unsupported",
    "Autonomous apply: unsupported",
];
const GUI_PHASE8_REQUIRED_ARTIFACTS: &[&str] = &[
    "plans/evidence/gui-productization/phase-8-plugin-management.md",
    "plans/evidence/gui-productization/phase-8-collaboration-gui.md",
    "plans/evidence/gui-productization/phase-8-remote-workspace-gui.md",
    "plans/evidence/gui-productization/phase-8-delegated-task-command-center.md",
    "plans/evidence/gui-productization/phase-8-ga-release-runbook.md",
    "plans/evidence/gui-productization/phase-8-update-rollback-incident.md",
    "plans/evidence/gui-productization/phase-8-platform-parity.md",
    "plans/evidence/gui-productization/phase-8-final-gates.md",
    ".planning/phases/08-advanced-platform-gui-ga/08-01-RESULT.md",
    ".planning/phases/08-advanced-platform-gui-ga/08-02-RESULT.md",
    ".planning/phases/08-advanced-platform-gui-ga/08-03-RESULT.md",
    ".planning/phases/08-advanced-platform-gui-ga/08-04-RESULT.md",
    ".planning/phases/08-advanced-platform-gui-ga/08-05-RESULT.md",
    ".planning/phases/08-advanced-platform-gui-ga/08-06-RESULT.md",
    ".planning/phases/08-advanced-platform-gui-ga/08-07-RESULT.md",
    "scripts/gui-smoke.ps1",
    "scripts/gui-smoke.sh",
    ".github/workflows/ci.yml",
];
const GUI_PHASE8_REQUIRED_COMMAND_MARKERS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo deny check",
    "cargo test -p devil-desktop plugin_management collaboration_gui remote_workspace_gui delegated_task_command_center -- --nocapture",
    "cargo run -p devil-cli -- evidence check --phase gui-phase8",
    "cargo run -p devil-cli -- evidence check --phase phase8",
    "powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Help",
    "bash scripts/gui-smoke.sh --help",
];
const GUI_PHASE8_REQUIRED_SURFACE_MARKERS: &[&str] = &[
    "Plugin management GUI: supported",
    "Collaboration GUI: supported",
    "Remote workspace GUI: supported",
    "Delegated task command center: approval-gated",
    "Autonomous apply: unsupported",
];
const GUI_PHASE8_REQUIRED_PLATFORM_MARKERS: &[&str] = &[
    "Platform parity: Windows",
    "Platform parity: macOS",
    "Platform parity: Linux",
    "Update rollback: documented",
    "Incident response: documented",
];
const GUI_PHASE8_STALE_UNSUPPORTED_MARKERS: &[&str] = &[
    "Remote production GUI: unsupported",
    "Collaboration GUI: unsupported",
    "Plugin management GUI: unsupported",
    "Cross-platform parity: unsupported",
];

const PHASE8_ACCEPTED_REQUIRED_MARKERS: &[&str] = &[
    "Runtime surface status: Production GA runtime surfaces are active behind accepted policy gates.",
    "Platform matrix: Linux, Windows, and macOS validated.",
    "Release readiness: Security, privacy, operations, rollback, canary, incident, and supply-chain signoff complete.",
    "Final gate outputs archived from current commands.",
];

const PHASE8_PLATFORM_MATRIX_ARTIFACT: &str = "platform-matrix-evidence.txt";
const PHASE8_RELEASE_READINESS_ARTIFACT: &str = "release-readiness-review.md";
const PHASE8_PLATFORM_MATRIX_REQUIRED_MARKERS: &[&str] = &[
    "Workflow: .github/workflows/ci.yml",
    "Run URL:",
    "ubuntu-latest: passed",
    "windows-latest: passed",
    "macos-latest: passed",
    "cargo fmt --all --check: passed",
    "cargo check --workspace --all-targets: passed",
    "cargo test --workspace --all-targets: passed",
    "cargo clippy --workspace --all-targets -- -D warnings: passed",
    "cargo deny check: passed",
    "cargo run -p devil-cli -- evidence check --phase phase8: passed",
    "cargo run -p xtask -- check-deps: passed",
];
const PHASE8_RELEASE_SIGNOFF_REQUIRED_MARKERS: &[&str] = &[
    "Signoff date:",
    "Security signoff: Complete.",
    "Privacy signoff: Complete.",
    "Operations signoff: Complete.",
    "Rollback signoff: Complete.",
    "Canary signoff: Complete.",
    "Incident response signoff: Complete.",
    "Supply-chain signoff: Complete.",
];

const PHASE8_STALE_DEFERRED_MARKERS: &[&str] = &[
    "production transport, native terminal, hosted export, raw-source vault, and operational GA remain deferred",
    "not final GA acceptance evidence",
    "fixture slice is active",
];

const PHASE8_ACCEPTED_ARTIFACT_STALE_MARKERS: &[&str] = &[
    "pending",
    "TODO",
    "Not accepted",
    "not accepted",
    "not final GA acceptance evidence",
    "still pending",
    "final GA signoff still pending",
];

const PHASE8_NOT_ACCEPTED_ALLOWED_MARKERS: &[&str] = &[
    "Runtime surface status:",
    "Phase 8 remains future-gated",
    "Deterministic metadata-only fixture slice",
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
    /// GUI Phase 6 packaging/platform/accessibility productization evidence.
    GuiPhase6,
    /// GUI Phase 7 local IDE beta productization evidence.
    GuiPhase7,
    /// GUI Phase 8 advanced platform GUI GA productization evidence.
    GuiPhase8,
    /// Phase 8 production GA scaffold or acceptance evidence.
    Phase8,
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
            "Phase 2 — Proposal mutation substrate | **Accepted**",
            "Phase 2 accepted status",
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
            "Phase 3 — Semantic fabric and LSP supervision | **Accepted**",
            "Phase 3 accepted status",
            &mut issues,
        );
        require_text(
            &ledger,
            "Future-gated",
            "Phase 8 remains future-gated",
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
            "Phase 3 acceptance: Accepted.",
            "Phase 3 acceptance is accepted",
            &mut issues,
        );
        require_text(
            &phase3,
            "LSP supervision acceptance: Accepted.",
            "LSP supervision is accepted",
            &mut issues,
        );
        require_text(
            &phase3,
            "vector indexing",
            "vector indexing deferral is documented",
            &mut issues,
        );
    }

    // Accepted Phase 4 crates may contain runtime code, but must stay protocol-mediated.
    require_phase4_runtime_boundaries(&workspace, "crates/devil-agent", &mut issues);
    require_phase4_runtime_boundaries(&workspace, "crates/devil-tracker", &mut issues);
    require_phase4_runtime_boundaries(&workspace, "crates/devil-memory", &mut issues);

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
        EvidencePhase::GuiPhase6 => check_gui_phase6_evidence(&workspace, &mut issues),
        EvidencePhase::GuiPhase7 => check_gui_phase7_evidence(&workspace, &mut issues),
        EvidencePhase::GuiPhase8 => check_gui_phase8_evidence(&workspace, &mut issues),
        EvidencePhase::Phase8 => check_phase8_evidence(&workspace, &mut issues),
    }
    finish_issue_report("Evidence check", &workspace, issues)
}

fn run_activation_check(workspace: PathBuf) -> Result<()> {
    let workspace = canonical_workspace(workspace)?;
    let mut issues = Vec::new();
    require_phase4_runtime_boundaries(&workspace, "crates/devil-agent", &mut issues);
    require_phase4_runtime_boundaries(&workspace, "crates/devil-tracker", &mut issues);
    require_phase4_runtime_boundaries(&workspace, "crates/devil-memory", &mut issues);
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
            "remain ADR/evidence gated",
            "placeholder runtime surfaces remain ADR/evidence gated",
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
            "Phase 4 — Native agentic AI execution context",
            "Phase 5 — WASM isolated extension ecosystem",
            "Phase 6 — Collaboration substrate",
            "Phase 7 — Remote development",
            "Phase 8 — Hardening",
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
    println!("cargo run -p devil-cli -- evidence check --phase gui-phase6");
    println!("cargo run -p devil-cli -- evidence check --phase gui-phase7");
    println!("cargo run -p devil-cli -- evidence check --phase phase8");
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

fn check_gui_phase6_evidence(workspace: &std::path::Path, issues: &mut Vec<String>) {
    let Some(evidence) = read_optional(workspace, GUI_PHASE6_EVIDENCE_PATH, issues) else {
        return;
    };

    require_text(
        &evidence,
        "GUI Phase 6",
        "GUI Phase 6 evidence names productization scope",
        issues,
    );
    if evidence.contains("Phase 6 acceptance: Accepted.") {
        if evidence.contains("- [ ]") {
            issues.push(
                "GUI Phase 6 is marked accepted but checklist still has unchecked items"
                    .to_string(),
            );
        }
        if evidence.contains("This document is GUI Phase 6 scaffold evidence") {
            issues.push(
                "GUI Phase 6 is marked accepted but still declares scaffold evidence".to_string(),
            );
        }
        for artifact in GUI_PHASE6_REQUIRED_ARTIFACTS {
            require_text(
                &evidence,
                artifact,
                &format!("GUI Phase 6 required artifact `{artifact}` is listed"),
                issues,
            );
            require_file(workspace, artifact, issues);
        }
        for command in GUI_PHASE6_REQUIRED_COMMAND_MARKERS {
            require_text(
                &evidence,
                command,
                &format!("GUI Phase 6 required command `{command}` is listed"),
                issues,
            );
        }
    } else {
        require_text(
            &evidence,
            "Phase 6 acceptance: Not accepted.",
            "GUI Phase 6 remains gated until evidence is complete",
            issues,
        );
    }
}

fn check_gui_phase7_evidence(workspace: &std::path::Path, issues: &mut Vec<String>) {
    let Some(evidence) = read_optional(workspace, GUI_PHASE7_EVIDENCE_PATH, issues) else {
        return;
    };

    require_text(
        &evidence,
        "GUI Phase 7",
        "GUI Phase 7 evidence names local beta productization scope",
        issues,
    );
    if evidence.contains("Phase 7 acceptance: Accepted.") {
        if evidence.contains("- [ ]") {
            issues.push(
                "GUI Phase 7 is marked accepted but checklist still has unchecked items"
                    .to_string(),
            );
        }
        if evidence.contains("This document is GUI Phase 7 scaffold evidence") {
            issues.push(
                "GUI Phase 7 is marked accepted but still declares scaffold evidence".to_string(),
            );
        }
        for artifact in GUI_PHASE7_REQUIRED_ARTIFACTS {
            require_text(
                &evidence,
                artifact,
                &format!("GUI Phase 7 required artifact `{artifact}` is listed"),
                issues,
            );
            require_file(workspace, artifact, issues);
        }
        for command in GUI_PHASE7_REQUIRED_COMMAND_MARKERS {
            require_text(
                &evidence,
                command,
                &format!("GUI Phase 7 required command `{command}` is listed"),
                issues,
            );
        }
        for marker in GUI_PHASE7_REQUIRED_LIMITATION_MARKERS {
            require_text(
                &evidence,
                marker,
                &format!("GUI Phase 7 required limitation marker `{marker}` is listed"),
                issues,
            );
        }
    } else {
        require_text(
            &evidence,
            "Phase 7 acceptance: Not accepted.",
            "GUI Phase 7 remains gated until evidence is complete",
            issues,
        );
    }
}

fn check_gui_phase8_evidence(workspace: &std::path::Path, issues: &mut Vec<String>) {
    let Some(evidence) = read_optional(workspace, GUI_PHASE8_EVIDENCE_PATH, issues) else {
        return;
    };

    require_text(
        &evidence,
        "GUI Phase 8",
        "GUI Phase 8 evidence names advanced platform GUI GA productization scope",
        issues,
    );
    if evidence.contains("Phase 8 acceptance: Accepted.") {
        if evidence.contains("- [ ]") {
            issues.push(
                "GUI Phase 8 is marked accepted but checklist still has unchecked items"
                    .to_string(),
            );
        }
        if evidence.contains("This document is GUI Phase 8 scaffold evidence") {
            issues.push(
                "GUI Phase 8 is marked accepted but still declares scaffold evidence".to_string(),
            );
        }
        for artifact in GUI_PHASE8_REQUIRED_ARTIFACTS {
            require_text(
                &evidence,
                artifact,
                &format!("GUI Phase 8 required artifact `{artifact}` is listed"),
                issues,
            );
            require_file(workspace, artifact, issues);
        }
        for command in GUI_PHASE8_REQUIRED_COMMAND_MARKERS {
            require_text(
                &evidence,
                command,
                &format!("GUI Phase 8 required command `{command}` is listed"),
                issues,
            );
        }
        for marker in GUI_PHASE8_REQUIRED_SURFACE_MARKERS {
            require_text(
                &evidence,
                marker,
                &format!("GUI Phase 8 required supported surface marker `{marker}` is listed"),
                issues,
            );
        }
        for marker in GUI_PHASE8_REQUIRED_PLATFORM_MARKERS {
            require_text(
                &evidence,
                marker,
                &format!("GUI Phase 8 required platform marker `{marker}` is listed"),
                issues,
            );
        }
        for marker in GUI_PHASE8_STALE_UNSUPPORTED_MARKERS {
            if evidence.contains(marker) {
                issues.push(format!(
                    "GUI Phase 8 is marked accepted but still contains stale unsupported marker `{marker}`"
                ));
            }
        }
    } else {
        require_text(
            &evidence,
            "Phase 8 acceptance: Not accepted.",
            "GUI Phase 8 remains gated until evidence is complete",
            issues,
        );
    }
}

fn check_phase8_evidence(workspace: &std::path::Path, issues: &mut Vec<String>) {
    let relative = "plans/evidence/phase-8/phase-8-architecture-map.md";
    let Some(phase8) = read_optional(workspace, relative, issues) else {
        return;
    };
    for artifact in PHASE8_REQUIRED_ARTIFACTS {
        require_text(
            &phase8,
            artifact,
            &format!("Phase 8 required artifact `{artifact}` is listed"),
            issues,
        );
    }
    if phase8.contains("Phase 8 acceptance: Accepted.") {
        for artifact in PHASE8_REQUIRED_ARTIFACTS {
            require_file(
                workspace,
                &format!("plans/evidence/phase-8/{artifact}"),
                issues,
            );
        }
        if phase8.contains("- [ ]") {
            issues.push(
                "Phase 8 is marked accepted but checklist still has unchecked items".to_string(),
            );
        }
        if phase8.contains("This document is Phase 8 scaffold evidence") {
            issues.push(
                "Phase 8 is marked accepted but still declares scaffold evidence".to_string(),
            );
        }
        for marker in PHASE8_STALE_DEFERRED_MARKERS {
            if phase8.contains(marker) {
                issues.push(format!(
                    "Phase 8 is marked accepted but still contains stale deferred marker `{marker}`"
                ));
            }
        }
        for marker in PHASE8_ACCEPTED_REQUIRED_MARKERS {
            require_text(
                &phase8,
                marker,
                &format!("Phase 8 final GA marker `{marker}`"),
                issues,
            );
        }
        validate_phase8_final_artifact_contents(workspace, issues);
    } else {
        require_text(
            &phase8,
            "Phase 8 acceptance: Not accepted.",
            "Phase 8 remains gated until GA evidence is complete",
            issues,
        );
        if !PHASE8_NOT_ACCEPTED_ALLOWED_MARKERS
            .iter()
            .any(|marker| phase8.contains(marker))
        {
            issues.push(
                "Phase 8 is not accepted but does not describe the current gated runtime posture"
                    .to_string(),
            );
        }
    }
}

fn validate_phase8_final_artifact_contents(workspace: &std::path::Path, issues: &mut Vec<String>) {
    validate_artifact_markers(
        workspace,
        PHASE8_PLATFORM_MATRIX_ARTIFACT,
        PHASE8_PLATFORM_MATRIX_REQUIRED_MARKERS,
        "Phase 8 platform matrix evidence",
        issues,
    );
    validate_artifact_markers(
        workspace,
        PHASE8_RELEASE_READINESS_ARTIFACT,
        PHASE8_RELEASE_SIGNOFF_REQUIRED_MARKERS,
        "Phase 8 release signoff evidence",
        issues,
    );
}

fn validate_artifact_markers(
    workspace: &std::path::Path,
    artifact: &str,
    markers: &[&str],
    label: &str,
    issues: &mut Vec<String>,
) {
    let path = workspace.join("plans/evidence/phase-8").join(artifact);
    let Ok(source) = fs::read_to_string(&path) else {
        issues.push(format!(
            "{label} artifact `{}` could not be read",
            path.display()
        ));
        return;
    };
    for marker in markers {
        if !source.contains(marker) {
            issues.push(format!(
                "{label} artifact `{artifact}` is missing required marker `{marker}`"
            ));
        }
    }
    for marker in PHASE8_ACCEPTED_ARTIFACT_STALE_MARKERS {
        if contains_phase8_accepted_artifact_stale_marker(&source, marker) {
            issues.push(format!(
                "{label} artifact `{artifact}` still contains stale marker `{marker}`"
            ));
        }
    }
}

fn contains_phase8_accepted_artifact_stale_marker(source: &str, marker: &str) -> bool {
    match marker {
        "pending" => contains_ascii_token_case_insensitive(source, "pending"),
        "TODO" => contains_ascii_token(source, "TODO"),
        _ => source.contains(marker),
    }
}

fn contains_ascii_token_case_insensitive(source: &str, token: &str) -> bool {
    contains_ascii_token(&source.to_ascii_lowercase(), &token.to_ascii_lowercase())
}

fn contains_ascii_token(source: &str, token: &str) -> bool {
    source.match_indices(token).any(|(start, _)| {
        let end = start + token.len();
        let before_is_boundary = source[..start]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_ascii_word_char(ch));
        let after_is_boundary = source[end..]
            .chars()
            .next()
            .is_none_or(|ch| !is_ascii_word_char(ch));
        before_is_boundary && after_is_boundary
    })
}

fn is_ascii_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
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

fn require_phase4_runtime_boundaries(
    workspace: &std::path::Path,
    crate_dir: &str,
    issues: &mut Vec<String>,
) {
    let lib_path = format!("{crate_dir}/src/lib.rs");
    let manifest_path = format!("{crate_dir}/Cargo.toml");
    let Some(source) = read_optional(workspace, &lib_path, issues) else {
        return;
    };
    let Some(manifest) = read_optional(workspace, &manifest_path, issues) else {
        return;
    };

    if !source.contains("devil_protocol") {
        issues.push(format!(
            "Phase 4 runtime `{crate_dir}` must use protocol DTOs as its boundary"
        ));
    }
    for forbidden in [
        "devil-app",
        "devil-ui",
        "devil-editor",
        "devil-project",
        "WorkspaceActor",
        "EditorSession",
    ] {
        if source.contains(forbidden) || manifest.contains(forbidden) {
            issues.push(format!(
                "Phase 4 runtime `{crate_dir}` must not depend on `{forbidden}` ownership"
            ));
        }
    }
}

fn ci_contains_gate(ci: &str, command: &str) -> bool {
    if ci.contains(command) {
        return true;
    }

    command == "cargo deny check"
        && (ci.contains("cargo-deny-action") || ci.contains("EmbarkStudios/cargo-deny-action"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        GUI_PHASE6_EVIDENCE_PATH, GUI_PHASE6_REQUIRED_ARTIFACTS,
        GUI_PHASE6_REQUIRED_COMMAND_MARKERS, GUI_PHASE7_EVIDENCE_PATH,
        GUI_PHASE7_REQUIRED_ARTIFACTS, GUI_PHASE7_REQUIRED_COMMAND_MARKERS,
        GUI_PHASE7_REQUIRED_LIMITATION_MARKERS, GUI_PHASE8_EVIDENCE_PATH,
        GUI_PHASE8_REQUIRED_ARTIFACTS, GUI_PHASE8_REQUIRED_COMMAND_MARKERS,
        GUI_PHASE8_REQUIRED_PLATFORM_MARKERS, GUI_PHASE8_REQUIRED_SURFACE_MARKERS,
        check_gui_phase6_evidence, check_gui_phase7_evidence, check_gui_phase8_evidence,
    };

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempWorkspace {
        root: PathBuf,
    }

    impl TempWorkspace {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos();
            let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
            let root = std::env::temp_dir().join(format!(
                "{prefix}_{}_{}_{}",
                std::process::id(),
                nanos,
                id
            ));
            fs::create_dir(&root).expect("temp workspace should be created");
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent directory should be created");
            }
            fs::write(path, contents).expect("test file should be written");
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            if self.root.starts_with(std::env::temp_dir()) {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    fn accepted_gui_phase6_evidence(checklist_checked: bool) -> String {
        let checklist = if checklist_checked { "x" } else { " " };
        let artifacts = GUI_PHASE6_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let commands = GUI_PHASE6_REQUIRED_COMMAND_MARKERS
            .iter()
            .map(|command| format!("- `{command}`\n"))
            .collect::<String>();

        format!(
            r#"# GUI Phase 6 evidence

## Acceptance Status

- Phase 6 acceptance: Accepted.

## Required Artifacts

{artifacts}
## Required Commands

{commands}
## Final Validation Checklist

- [{checklist}] Required validation is complete.
"#
        )
    }

    fn accepted_gui_phase7_evidence(checklist_checked: bool) -> String {
        let checklist = if checklist_checked { "x" } else { " " };
        let artifacts = GUI_PHASE7_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let commands = GUI_PHASE7_REQUIRED_COMMAND_MARKERS
            .iter()
            .map(|command| format!("- `{command}`\n"))
            .collect::<String>();
        let limitations = GUI_PHASE7_REQUIRED_LIMITATION_MARKERS
            .iter()
            .map(|marker| format!("- {marker}\n"))
            .collect::<String>();

        format!(
            r#"# GUI Phase 7 local IDE beta evidence

## Acceptance Status

- Phase 7 acceptance: Accepted.

## Required Artifacts

{artifacts}
## Required Commands

{commands}
## Known Limitations Required For Acceptance

{limitations}
## Final Validation Checklist

- [{checklist}] Required validation is complete.
"#
        )
    }

    fn accepted_gui_phase8_evidence(checklist_checked: bool) -> String {
        let checklist = if checklist_checked { "x" } else { " " };
        let artifacts = GUI_PHASE8_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let commands = GUI_PHASE8_REQUIRED_COMMAND_MARKERS
            .iter()
            .map(|command| format!("- `{command}`\n"))
            .collect::<String>();
        let surfaces = GUI_PHASE8_REQUIRED_SURFACE_MARKERS
            .iter()
            .map(|marker| format!("- {marker}\n"))
            .collect::<String>();
        let platforms = GUI_PHASE8_REQUIRED_PLATFORM_MARKERS
            .iter()
            .map(|marker| format!("- {marker}\n"))
            .collect::<String>();

        format!(
            r#"# GUI Phase 8 advanced platform GUI GA evidence

## Acceptance Status

- Phase 8 acceptance: Accepted.

## Required Artifacts

{artifacts}
## Required Commands

{commands}
## Supported Advanced GUI Surface Markers

{surfaces}
## Platform Parity Markers

{platforms}
## Final Validation Checklist

- [{checklist}] Required validation is complete.
"#
        )
    }

    #[test]
    fn gui_phase6_not_accepted_scaffold_passes_without_artifacts() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase6_not_accepted");
        workspace.write(
            GUI_PHASE6_EVIDENCE_PATH,
            r#"# GUI Phase 6 evidence

## Acceptance Status

- Phase 6 acceptance: Not accepted.
"#,
        );
        let mut issues = Vec::new();

        check_gui_phase6_evidence(workspace.path(), &mut issues);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase6_acceptance_requires_artifacts_commands_and_checked_checklist() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase6_incomplete");
        workspace.write(
            GUI_PHASE6_EVIDENCE_PATH,
            &accepted_gui_phase6_evidence(false),
        );
        let mut issues = Vec::new();

        check_gui_phase6_evidence(workspace.path(), &mut issues);

        assert!(issues.iter().any(|issue| {
            issue.contains("GUI Phase 6 is marked accepted but checklist still has unchecked items")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "required file `plans/evidence/gui-productization/phase-6-package-runbook.md` is missing",
            )
        }));
    }

    #[test]
    fn gui_phase6_acceptance_passes_with_artifacts_commands_and_checked_checklist() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase6_complete");
        workspace.write(
            GUI_PHASE6_EVIDENCE_PATH,
            &accepted_gui_phase6_evidence(true),
        );
        for artifact in GUI_PHASE6_REQUIRED_ARTIFACTS {
            workspace.write(artifact, "artifact present\n");
        }
        let mut issues = Vec::new();

        check_gui_phase6_evidence(workspace.path(), &mut issues);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase7_not_accepted_scaffold_passes_without_artifacts() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase7_not_accepted");
        workspace.write(
            GUI_PHASE7_EVIDENCE_PATH,
            r#"# GUI Phase 7 local IDE beta evidence

## Acceptance Status

- Phase 7 acceptance: Not accepted.
"#,
        );
        let mut issues = Vec::new();

        check_gui_phase7_evidence(workspace.path(), &mut issues);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase7_acceptance_requires_artifacts_commands_limits_and_checked_checklist() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase7_incomplete");
        let evidence = accepted_gui_phase7_evidence(false)
            .replace(
                "- `cargo run -p devil-cli -- evidence check --phase gui-phase7`\n",
                "",
            )
            .replace("- Autonomous apply: unsupported\n", "");
        workspace.write(GUI_PHASE7_EVIDENCE_PATH, &evidence);
        let mut issues = Vec::new();

        check_gui_phase7_evidence(workspace.path(), &mut issues);

        assert!(issues.iter().any(|issue| {
            issue.contains("GUI Phase 7 is marked accepted but checklist still has unchecked items")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "required file `plans/evidence/gui-productization/phase-7-local-workflow-smoke.md` is missing",
            )
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "missing marker for GUI Phase 7 required command `cargo run -p devil-cli -- evidence check --phase gui-phase7` is listed",
            )
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "missing marker for GUI Phase 7 required limitation marker `Autonomous apply: unsupported` is listed",
            )
        }));
    }

    #[test]
    fn gui_phase7_acceptance_passes_with_artifacts_commands_limits_and_checked_checklist() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase7_complete");
        workspace.write(
            GUI_PHASE7_EVIDENCE_PATH,
            &accepted_gui_phase7_evidence(true),
        );
        for artifact in GUI_PHASE7_REQUIRED_ARTIFACTS {
            workspace.write(artifact, "artifact present\n");
        }
        let mut issues = Vec::new();

        check_gui_phase7_evidence(workspace.path(), &mut issues);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase8_not_accepted_scaffold_passes_without_artifacts() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase8_not_accepted");
        workspace.write(
            GUI_PHASE8_EVIDENCE_PATH,
            r#"# GUI Phase 8 advanced platform GUI GA evidence

## Acceptance Status

- Phase 8 acceptance: Not accepted.
"#,
        );
        let mut issues = Vec::new();

        check_gui_phase8_evidence(workspace.path(), &mut issues);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase8_acceptance_requires_artifacts_commands_markers_and_checked_checklist() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase8_incomplete");
        let evidence = accepted_gui_phase8_evidence(false)
            .replace(
                "- `cargo run -p devil-cli -- evidence check --phase gui-phase8`\n",
                "",
            )
            .replace("- Plugin management GUI: supported\n", "")
            .replace("- Platform parity: Linux\n", "");
        workspace.write(GUI_PHASE8_EVIDENCE_PATH, &evidence);
        let mut issues = Vec::new();

        check_gui_phase8_evidence(workspace.path(), &mut issues);

        assert!(issues.iter().any(|issue| {
            issue.contains("GUI Phase 8 is marked accepted but checklist still has unchecked items")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "required file `plans/evidence/gui-productization/phase-8-plugin-management.md` is missing",
            )
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "missing marker for GUI Phase 8 required command `cargo run -p devil-cli -- evidence check --phase gui-phase8` is listed",
            )
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "missing marker for GUI Phase 8 required supported surface marker `Plugin management GUI: supported` is listed",
            )
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "missing marker for GUI Phase 8 required platform marker `Platform parity: Linux` is listed",
            )
        }));
    }

    #[test]
    fn gui_phase8_acceptance_rejects_phase7_unsupported_labels() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase8_stale_label");
        let mut evidence = accepted_gui_phase8_evidence(true);
        evidence.push_str("\nCollaboration GUI: unsupported\n");
        workspace.write(GUI_PHASE8_EVIDENCE_PATH, &evidence);
        for artifact in GUI_PHASE8_REQUIRED_ARTIFACTS {
            workspace.write(artifact, "artifact present\n");
        }
        let mut issues = Vec::new();

        check_gui_phase8_evidence(workspace.path(), &mut issues);

        assert!(issues.iter().any(|issue| {
            issue.contains(
                "GUI Phase 8 is marked accepted but still contains stale unsupported marker `Collaboration GUI: unsupported`",
            )
        }));
    }

    #[test]
    fn gui_phase8_acceptance_passes_with_artifacts_commands_markers_and_checked_checklist() {
        let workspace = TempWorkspace::new("devil_cli_gui_phase8_complete");
        workspace.write(
            GUI_PHASE8_EVIDENCE_PATH,
            &accepted_gui_phase8_evidence(true),
        );
        for artifact in GUI_PHASE8_REQUIRED_ARTIFACTS {
            workspace.write(artifact, "artifact present\n");
        }
        let mut issues = Vec::new();

        check_gui_phase8_evidence(workspace.path(), &mut issues);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }
}
