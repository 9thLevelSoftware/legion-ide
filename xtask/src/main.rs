use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::Path,
    process,
};

use cargo_metadata::{Metadata, MetadataCommand};
use clap::{Parser, Subcommand};

const DEFAULT_POLICY_PATH: &str = "plans/dependency-policy.md";
const DEFAULT_PROTOCOL_PATH: &str = "crates/legion-protocol/src/lib.rs";
const DEFAULT_UI_MANIFEST_PATH: &str = "crates/legion-ui/Cargo.toml";
const DEFAULT_PHASE3_EVIDENCE_PATH: &str = "plans/evidence/phase-3/predictive-semantic-fabric.md";
const DEFAULT_PHASE4_EVIDENCE_PATH: &str = "plans/evidence/phase-4/agentic-ai-architecture-map.md";
const DEFAULT_PHASE5_EVIDENCE_PATH: &str = "plans/evidence/phase-5/plugin-architecture-map.md";
const DEFAULT_GUI_PHASE5_EVIDENCE_PATH: &str =
    "plans/evidence/gui-productization/phase-5-control-trust-assisted-ai.md";
const DEFAULT_GUI_PHASE6_EVIDENCE_PATH: &str =
    "plans/evidence/gui-productization/phase-6-packaging-platform-accessibility.md";
const DEFAULT_GUI_PHASE7_EVIDENCE_PATH: &str =
    "plans/evidence/gui-productization/phase-7-local-ide-beta.md";
const DEFAULT_GUI_PHASE8_EVIDENCE_PATH: &str =
    "plans/evidence/gui-productization/phase-8-advanced-platform-gui-ga.md";
const DEFAULT_PHASE13_EVIDENCE_PATH: &str =
    "plans/evidence/gui-productization/phase-13-legion-workflow-orchestration.md";
const DEFAULT_PHASE13_FINAL_GATES_PATH: &str =
    "plans/evidence/gui-productization/phase-13-final-gates.md";
const DEFAULT_PHASE13_RUNBOOK_PATH: &str = "plans/evidence/gui-productization/phase-13-runbook.md";
const DEFAULT_DOCS_HYGIENE_ALLOWLIST_PATH: &str = "docs/hygiene-allowlist.toml";
const DEFAULT_NO_EGUI_TEXTEDIT_CONFIG_PATH: &str = "xtask/no-egui-textedit.toml";
const DEFAULT_RELEASE_PIPELINE_CONFIG_PATH: &str = "xtask/release-pipeline.example.toml";
const DEFAULT_RELEASE_PIPELINE_OUTPUT_PATH: &str = "target/release-pipeline";
const DEFAULT_PERF_HARNESS_OUTPUT_PATH: &str = "target/perf-harness";
const DEFAULT_BENCH_OUTPUT_PATH: &str = "target/legion-bench";
const DEFAULT_PHASE6_EVIDENCE_PATH: &str =
    "plans/evidence/phase-6/collaboration-architecture-map.md";
const DEFAULT_PHASE7_EVIDENCE_PATH: &str = "plans/evidence/phase-7/remote-architecture-map.md";
const DEFAULT_PHASE8_EVIDENCE_PATH: &str = "plans/evidence/phase-8/phase-8-architecture-map.md";
const PHASE3_STATUS_HEADING: &str = "## Acceptance status";
const PHASE3_FINAL_CHECKLIST_HEADING: &str = "## Final validation checklist";
const PHASE4_STATUS_HEADING: &str = "## Acceptance status";
const PHASE4_FINAL_CHECKLIST_HEADING: &str = "## Final validation checklist";
const PHASE5_STATUS_HEADING: &str = "## Acceptance Status";
const PHASE5_FINAL_CHECKLIST_HEADING: &str = "## Final Validation Checklist";
const PHASE6_STATUS_HEADING: &str = "## Acceptance Status";
const PHASE6_FINAL_CHECKLIST_HEADING: &str = "## Final Validation Checklist";
const PHASE7_STATUS_HEADING: &str = "## Acceptance Status";
const PHASE7_FINAL_CHECKLIST_HEADING: &str = "## Final Validation Checklist";
const PHASE8_STATUS_HEADING: &str = "## Acceptance Status";
const PHASE8_FINAL_CHECKLIST_HEADING: &str = "## Final Validation Checklist";
const PHASE13_FINAL_CHECKLIST_HEADING: &str = "## Final Validation Checklist";
const PHASE3_PARTIAL_RUNTIME_MARKER: &str = "Runtime surface status: Partial `legion-index` indexing behavior is active; acceptance evidence is incomplete.";
const PHASE3_NOT_ACCEPTED_MARKER: &str = "Phase 3 acceptance: Not accepted.";
const PHASE3_ACCEPTED_MARKER: &str = "Phase 3 acceptance: Accepted.";
const LSP_NOT_ACCEPTED_MARKER: &str = "LSP supervision acceptance: Not accepted.";
const LSP_ACCEPTED_MARKER: &str = "LSP supervision acceptance: Accepted.";
const PHASE4_NOT_ACCEPTED_MARKER: &str = "Phase 4 acceptance: Not accepted.";
const PHASE4_ACCEPTED_MARKER: &str = "Phase 4 acceptance: Accepted.";
const PHASE5_NOT_ACCEPTED_MARKER: &str = "Phase 5 acceptance: Not accepted.";
const PHASE5_ACCEPTED_MARKER: &str = "Phase 5 acceptance: Accepted.";
const PHASE6_NOT_ACCEPTED_MARKER: &str = "Phase 6 acceptance: Not accepted.";
const PHASE6_ACCEPTED_MARKER: &str = "Phase 6 acceptance: Accepted.";
const PHASE7_NOT_ACCEPTED_MARKER: &str = "Phase 7 acceptance: Not accepted.";
const PHASE7_ACCEPTED_MARKER: &str = "Phase 7 acceptance: Accepted.";
const PHASE8_NOT_ACCEPTED_MARKER: &str = "Phase 8 acceptance: Not accepted.";
const PHASE8_ACCEPTED_MARKER: &str = "Phase 8 acceptance: Accepted.";
const PHASE13_ACCEPTED_MARKER: &str = "Phase 13 acceptance: Accepted";
const PHASE8_ACCEPTED_REQUIRED_MARKERS: &[&str] = &[
    "Runtime surface status: Production GA runtime surfaces are active behind accepted policy gates.",
    "Platform matrix: Linux, Windows, and macOS validated.",
    "Release readiness: Security, privacy, operations, rollback, canary, incident, and supply-chain signoff complete.",
    "Final gate outputs archived from current commands.",
];
const PHASE13_REQUIRED_EVIDENCE_MARKERS: &[&str] = &[
    PHASE13_ACCEPTED_MARKER,
    "Legion workflow orchestration: approval-gated",
    "Autonomous merge: unsupported until approval",
    "Provider-backed workers: routed through assisted-AI consent",
    "Final gate outputs archived from current commands",
];
const PHASE13_REQUIRED_RUNBOOK_MARKERS: &[&str] = &[
    "Autonomous merge: unsupported until approval",
    "Local workers: isolated delegated-task sandbox",
    "Provider-backed workers: routed through assisted-AI consent",
    "Merge readiness: proposal-mediated approval gate",
    "Raw payload retention: disabled by default",
];
const PHASE13_STALE_ACCEPTANCE_MARKERS: &[&str] = &[
    "TODO",
    "Not accepted",
    "not accepted",
    "acceptance pending",
    "final gates pending",
    "pending final gates",
    "still pending",
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
    "cargo run -p legion-cli -- evidence check --phase phase8: passed",
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
const PHASE4_REQUIRED_ARTIFACTS: &[&str] = &[
    "agentic-ai-architecture-map.md",
    "dependency-boundary.txt",
    "provider-router-contract-tests.txt",
    "local-provider-adapter-tests.txt",
    "air-gap-provider-egress-tests.txt",
    "privacy-inspector-context-manifest-tests.txt",
    "agent-state-machine-tests.txt",
    "tracker-run-ledger-tests.txt",
    "memory-retention-consent-tests.txt",
    "proposal-routing-regression.txt",
    "observability-redaction-audit.md",
    "cloud-provider-deferral-audit.md",
    "vector-deferral-audit.md",
    "cargo-fmt-check.txt",
    "cargo-check-workspace-all-targets.txt",
    "cargo-test-workspace-all-targets.txt",
    "cargo-clippy-workspace-all-targets.txt",
];
const PHASE5_REQUIRED_ARTIFACTS: &[&str] = &[
    "plugin-architecture-map.md",
    "dependency-boundary.txt",
    "wasm-abi-contract-tests.txt",
    "manifest-golden-tests.txt",
    "host-call-capability-tests.txt",
    "sandbox-denial-tests.txt",
    "plugin-crash-isolation-tests.txt",
    "plugin-storage-quota-tests.txt",
    "plugin-proposal-routing-tests.txt",
    "plugin-observability-redaction-audit.md",
    "future-surface-deferral-audit.md",
    "cargo-fmt-check.txt",
    "cargo-check-workspace-all-targets.txt",
    "cargo-test-workspace-all-targets.txt",
    "cargo-clippy-workspace-all-targets.txt",
];
const GUI_PHASE5_REQUIRED_ARTIFACTS: &[&str] = &[
    "plans/evidence/gui-productization/phase-5-control-trust-safety.md",
    ".planning/phases/05-control-trust-and-assisted-ai-surfaces/05-01-RESULT.md",
    ".planning/phases/05-control-trust-and-assisted-ai-surfaces/05-02-RESULT.md",
    ".planning/phases/05-control-trust-and-assisted-ai-surfaces/05-03-RESULT.md",
    ".planning/phases/05-control-trust-and-assisted-ai-surfaces/05-04-RESULT.md",
    ".planning/phases/05-control-trust-and-assisted-ai-surfaces/05-05-RESULT.md",
    ".planning/phases/05-control-trust-and-assisted-ai-surfaces/05-06-RESULT.md",
];
const GUI_PHASE5_REQUIRED_COMMAND_MARKERS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
];
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
    "cargo test -p legion-desktop --test packaging -- --nocapture",
    "cargo test -p legion-desktop --test platform_integration -- --nocapture",
    "cargo test -p legion-desktop --test platform_smoke -- --nocapture",
    "cargo test -p legion-desktop --test session_restore -- --nocapture",
    "cargo test -p legion-desktop --test diagnostics_export -- --nocapture",
    "cargo test -p legion-cli gui_phase6 -- --nocapture",
    "scripts/package-windows.ps1 -DryRun",
    "scripts/gui-smoke.ps1 -DryRun",
    "scripts/gui-smoke.sh --dry-run",
    "cargo run -p legion-cli -- evidence check --phase gui-phase6",
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
];
const GUI_PHASE7_REQUIRED_COMMAND_MARKERS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo deny check",
    "cargo test -p legion-desktop --test beta_workflow -- --nocapture",
    "cargo test -p legion-desktop --test operational_health -- --nocapture",
    "cargo test -p legion-desktop --test diagnostics_export -- --nocapture",
    "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Beta -DryRun",
    "bash scripts/gui-smoke.sh --beta --dry-run",
    "cargo run -p legion-cli -- evidence check --phase gui-phase7",
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
];
const GUI_PHASE8_REQUIRED_COMMAND_MARKERS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
    "cargo deny check",
    "cargo test -p legion-desktop --test plugin_management -- --nocapture",
    "cargo test -p legion-desktop --test collaboration_gui -- --nocapture",
    "cargo test -p legion-desktop --test remote_workspace_gui -- --nocapture",
    "cargo test -p legion-desktop --test delegated_task_command_center -- --nocapture",
    "cargo run -p legion-cli -- evidence check --phase gui-phase8",
    "cargo run -p legion-cli -- evidence check --phase phase8",
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
const PHASE13_REQUIRED_ARTIFACTS: &[&str] = &[
    "plans/adrs/ADR-0031-legion-workflow-orchestration.md",
    "plans/evidence/gui-productization/phase-13-governance.md",
    "plans/evidence/gui-productization/phase-13-final-gates.md",
    "plans/evidence/gui-productization/phase-13-runbook.md",
    ".planning/phases/13-legion-workflow-orchestration/13-01-RESULT.md",
    ".planning/phases/13-legion-workflow-orchestration/13-02-RESULT.md",
    ".planning/phases/13-legion-workflow-orchestration/13-03-RESULT.md",
    ".planning/phases/13-legion-workflow-orchestration/13-04-RESULT.md",
    ".planning/phases/13-legion-workflow-orchestration/13-05-RESULT.md",
    ".planning/phases/13-legion-workflow-orchestration/13-06-RESULT.md",
    ".planning/phases/13-legion-workflow-orchestration/13-07-RESULT.md",
];
const PHASE13_REQUIRED_COMMAND_MARKERS: &[&str] = &[
    "cargo run -p xtask -- check-deps",
    "cargo fmt --all --check",
    "cargo check --workspace --all-targets",
    "cargo test --workspace --all-targets",
    "cargo clippy --workspace --all-targets -- -D warnings",
];
const PHASE6_REQUIRED_ARTIFACTS: &[&str] = &[
    "collaboration-architecture-map.md",
    "dependency-boundary.txt",
    "protocol-dto-contract-tests.txt",
    "collaboration-convergence-tests.txt",
    "undo-semantics-tests.txt",
    "dirty-buffer-conflict-tests.txt",
    "shared-proposal-approval-tests.txt",
    "presence-ui-projection-tests.txt",
    "collaboration-security-capability-tests.txt",
    "disconnect-reconnect-replay-tests.txt",
    "storage-observability-redaction-audit.md",
    "future-surface-deferral-audit.md",
    "performance-budget-tests.txt",
    "cargo-fmt-check.txt",
    "cargo-check-workspace-all-targets.txt",
    "cargo-test-workspace-all-targets.txt",
    "cargo-clippy-workspace-all-targets.txt",
    "cargo-deny-check.txt",
];
const PHASE7_REQUIRED_ARTIFACTS: &[&str] = &[
    "remote-architecture-map.md",
    "dependency-boundary.txt",
    "protocol-dto-contract-tests.txt",
    "remote-security-threat-model.md",
    "transport-security-tests.txt",
    "remote-agent-lifecycle-tests.txt",
    "remote-filesystem-proposal-tests.txt",
    "remote-stale-conflict-tests.txt",
    "remote-process-terminal-policy-tests.txt",
    "remote-lsp-policy-tests.txt",
    "remote-semantic-index-query-tests.txt",
    "latency-reconnect-offline-resume-tests.txt",
    "collaboration-remote-integration-tests.txt",
    "storage-observability-redaction-audit.md",
    "performance-budget-tests.txt",
    "future-surface-deferral-audit.md",
    "cargo-fmt-check.txt",
    "cargo-check-workspace-all-targets.txt",
    "cargo-test-workspace-all-targets.txt",
    "cargo-clippy-workspace-all-targets.txt",
    "cargo-deny-check.txt",
    "xtask-check-deps.txt",
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
const RENDERER_BOUNDARY_POLICY_MARKERS: &[&str] = &[
    "`legion-desktop` may depend on:",
    "`eframe`",
    "`egui`",
    "renderer dependencies",
    "adapter-only",
    "any core substrate crate",
];
const RENDERER_DEPENDENCY_ALLOWED_PACKAGES: &[&str] = &["legion-desktop"];
const FORBIDDEN_RENDERER_DEPS: &[&str] = &[
    "eframe",
    "egui",
    "egui-winit",
    "egui-wgpu",
    "winit",
    "wgpu",
    "accesskit",
    "slint",
    "tauri",
    "wry",
    "tao",
    "gpui",
];
const PARSER_BOUNDARY_POLICY_MARKERS: &[&str] = &[
    "`legion-index` may depend on:",
    "`tree-sitter`",
    "`tree-sitter-rust`",
    "direct renderer/UI parser ownership",
];
const PARSER_DEPENDENCY_ALLOWED_PACKAGES: &[&str] = &["legion-index"];
const FORBIDDEN_PARSER_DEPS: &[&str] = &["tree-sitter", "tree-sitter-rust"];

#[derive(Parser)]
#[command(author, version, about = "Repository maintenance and validation tasks")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate workspace crate dependencies against architecture policy
    CheckDeps {
        /// Path to the markdown policy document.
        #[arg(long, default_value = DEFAULT_POLICY_PATH)]
        policy: String,
    },
    /// Validate Markdown links and stale documentation markers.
    DocsHygiene {
        /// Path to docs hygiene allowlist TOML.
        #[arg(long, default_value = DEFAULT_DOCS_HYGIENE_ALLOWLIST_PATH)]
        allowlist: String,
    },
    /// Forbid egui::TextEdit in the desktop code-canvas/editor render path.
    NoEguiTextedit {
        /// Path to no-egui-textedit TOML configuration.
        #[arg(long, default_value = DEFAULT_NO_EGUI_TEXTEDIT_CONFIG_PATH)]
        config: String,
    },
    /// Generate dry-run release pipeline installer descriptors.
    ReleasePipeline {
        /// Path to release pipeline TOML configuration.
        #[arg(long, default_value = DEFAULT_RELEASE_PIPELINE_CONFIG_PATH)]
        config: String,
        /// Output directory for generated descriptors.
        #[arg(long, default_value = DEFAULT_RELEASE_PIPELINE_OUTPUT_PATH)]
        out: String,
        /// Release channel: stable or preview.
        #[arg(long, default_value = "stable")]
        channel: String,
        /// Generate descriptors only; do not build, sign, or hash artifacts.
        #[arg(long)]
        dry_run: bool,
    },
    /// Verify previously-written release pipeline descriptors.
    VerifyReleasePipeline {
        /// Output directory holding descriptors and version stamp.
        #[arg(long, default_value = DEFAULT_RELEASE_PIPELINE_OUTPUT_PATH)]
        out: String,
    },
    /// Run the M0 performance-harness skeleton and write a perf report.
    ///
    /// The M0 deliverable is a deterministic in-process micro-benchmark
    /// that exercises a stand-in for the editor input-to-paint hot path.
    /// The post-M0 follow-on replaces this stand-in with the real
    /// `legion-editor` workload, the 100K-file fixture, and the 100MB
    /// file per master-plan §11. The CI leg can tighten the budget via
    /// `LEGION_PERF_FAIL_ON_BUDGET_MS=<ms>` to demonstrate the failing
    /// gate.
    PerfHarness {
        /// Output directory for the perf report.
        #[arg(long, default_value = DEFAULT_PERF_HARNESS_OUTPUT_PATH)]
        out: String,
        /// Treat any failed skeleton as a CI failure (default: true).
        /// Set `--no-strict` to keep the report-only behavior even when
        /// measurements exceed the configured budget.
        #[arg(long, default_value_t = true)]
        strict: bool,
    },
    /// Verify a previously-written perf-harness report.
    VerifyPerfHarness {
        /// Output directory holding the perf report.
        #[arg(long, default_value = DEFAULT_PERF_HARNESS_OUTPUT_PATH)]
        out: String,
        /// Treat any failed skeleton as a CI failure (default: true).
        /// Set `--no-strict` to keep the report-only behavior even when
        /// measurements exceed the configured budget.
        #[arg(long, default_value_t = true)]
        strict: bool,
    },
    /// Run the Legion-Bench v0 eval suite and write a bench report.
    LegionBench {
        /// Output directory for the bench report.
        #[arg(long, default_value = DEFAULT_BENCH_OUTPUT_PATH)]
        out: String,
        /// Run mode for the baseline. Recorded is the offline CI default;
        /// live is reserved for the weekly external run.
        #[arg(long, default_value = "recorded")]
        mode: String,
        /// Treat any failed task as a CI failure.
        #[arg(long, default_value_t = true)]
        strict: bool,
    },
    /// Verify a previously-written Legion-Bench report.
    VerifyLegionBench {
        /// Output directory holding the bench report.
        #[arg(long, default_value = DEFAULT_BENCH_OUTPUT_PATH)]
        out: String,
        /// Treat any failed task as a CI failure.
        #[arg(long, default_value_t = true)]
        strict: bool,
    },
}

fn main() {
    let args = Args::parse();

    let code = match args.command {
        Commands::CheckDeps { policy } => {
            if let Err(err) = run_check_deps(&policy) {
                eprintln!("dependency check failed: {err}");
                1
            } else {
                println!("dependency policy checks passed");
                0
            }
        }
        Commands::DocsHygiene { allowlist } => run_docs_hygiene_command(&allowlist),
        Commands::NoEguiTextedit { config } => run_no_egui_textedit_command(&config),
        Commands::ReleasePipeline {
            config,
            out,
            channel,
            dry_run,
        } => run_release_pipeline_command(&config, &out, &channel, dry_run),
        Commands::VerifyReleasePipeline { out } => run_verify_release_pipeline_command(&out),
        Commands::PerfHarness { out, strict } => run_perf_harness_command(&out, strict),
        Commands::VerifyPerfHarness { out, strict } => {
            run_verify_perf_harness_command(&out, strict)
        }
        Commands::LegionBench { out, mode, strict } => {
            run_legion_bench_command(&out, &mode, strict)
        }
        Commands::VerifyLegionBench { out, strict } => {
            run_verify_legion_bench_command(&out, strict)
        }
    };

    process::exit(code);
}

fn run_docs_hygiene_command(allowlist: &str) -> i32 {
    let workspace_root = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("docs hygiene failed: unable to resolve current directory: {err}");
            return 1;
        }
    };
    let allowlist_path = workspace_root.join(allowlist);
    let config = match xtask::docs_hygiene::DocsHygieneConfig::from_file(&allowlist_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("docs hygiene failed: {err}");
            return 1;
        }
    };
    match xtask::docs_hygiene::run_docs_hygiene(&workspace_root, &config) {
        Ok(()) => {
            println!("documentation hygiene checks passed");
            0
        }
        Err(violations) => {
            eprintln!(
                "documentation hygiene found {} violation(s):",
                violations.len()
            );
            for violation in violations.iter().take(200) {
                eprintln!(
                    "{}:{}: {:?}: {}",
                    violation.path.display(),
                    violation.line,
                    violation.kind,
                    violation.message
                );
            }
            if violations.len() > 200 {
                eprintln!("... {} more violation(s) omitted", violations.len() - 200);
            }
            1
        }
    }
}

fn run_no_egui_textedit_command(config_path: &str) -> i32 {
    let workspace_root = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("no-egui-textedit failed: unable to resolve current directory: {err}");
            return 1;
        }
    };
    let config_path = workspace_root.join(config_path);
    let config = match xtask::no_egui_textedit::NoEguiTextEditConfig::from_file(&config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("no-egui-textedit failed: {err}");
            return 1;
        }
    };
    match xtask::no_egui_textedit::run_no_egui_textedit(&workspace_root, &config) {
        Ok(()) => {
            println!("no-egui-textedit checks passed");
            0
        }
        Err(violations) => {
            eprintln!("no-egui-textedit found {} violation(s):", violations.len());
            for violation in violations.iter().take(200) {
                eprintln!(
                    "{}:{}: {:?}: {}",
                    violation.path.display(),
                    violation.line,
                    violation.kind,
                    violation.message
                );
            }
            if violations.len() > 200 {
                eprintln!("... {} more violation(s) omitted", violations.len() - 200);
            }
            1
        }
    }
}

fn run_release_pipeline_command(config_path: &str, out: &str, channel: &str, dry_run: bool) -> i32 {
    let workspace_root = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("release pipeline failed: unable to resolve current directory: {err}");
            return 1;
        }
    };
    let config_path = workspace_root.join(config_path);
    let config = match xtask::release_pipeline::ReleasePipelineConfig::from_file(&config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("release pipeline failed: {err}");
            return 1;
        }
    };
    let channel = match xtask::release_pipeline::ReleaseChannel::parse(channel) {
        Ok(channel) => channel,
        Err(err) => {
            eprintln!("release pipeline failed: {err}");
            return 1;
        }
    };
    let plan = match xtask::release_pipeline::plan_release_pipeline(
        &workspace_root,
        &config,
        channel,
        dry_run,
    ) {
        Ok(plan) => plan,
        Err(err) => {
            eprintln!("release pipeline failed: {err}");
            return 1;
        }
    };
    let out_dir = workspace_root.join(out);
    let written = match xtask::release_pipeline::write_descriptors(&plan, &out_dir) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("release pipeline failed: {err}");
            return 1;
        }
    };
    println!(
        "release pipeline dry-run wrote {} descriptor(s) to {}",
        written.len(),
        out_dir.display()
    );
    0
}

fn run_verify_release_pipeline_command(out: &str) -> i32 {
    let workspace_root = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("release pipeline verify failed: unable to resolve current directory: {err}");
            return 1;
        }
    };
    let config_path = workspace_root.join(DEFAULT_RELEASE_PIPELINE_CONFIG_PATH);
    let config = match xtask::release_pipeline::ReleasePipelineConfig::from_file(&config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("release pipeline verify failed: {err}");
            return 1;
        }
    };
    let out_dir = workspace_root.join(out);
    let stamp_path = out_dir.join(xtask::release_pipeline::VERSION_STAMP_FILE);
    let channel = match fs::read_to_string(&stamp_path) {
        Ok(text) => match toml::from_str::<xtask::release_pipeline::VersionStamp>(&text) {
            Ok(stamp) => match xtask::release_pipeline::ReleaseChannel::parse(&stamp.channel) {
                Ok(channel) => channel,
                Err(err) => {
                    eprintln!("release pipeline verify failed: {err}");
                    return 1;
                }
            },
            Err(err) => {
                eprintln!(
                    "release pipeline verify failed: unable to parse `{}`: {err}",
                    stamp_path.display()
                );
                return 1;
            }
        },
        Err(err) => {
            eprintln!(
                "release pipeline verify failed: unable to read `{}` (run `cargo run -p xtask -- release-pipeline --dry-run` first): {err}",
                stamp_path.display()
            );
            return 1;
        }
    };
    let plan = match xtask::release_pipeline::plan_release_pipeline(
        &workspace_root,
        &config,
        channel,
        true,
    ) {
        Ok(plan) => plan,
        Err(err) => {
            eprintln!("release pipeline verify failed: {err}");
            return 1;
        }
    };
    let report = match xtask::release_pipeline::verify_descriptors(&workspace_root, &plan, &out_dir)
    {
        Ok(report) => report,
        Err(err) => {
            eprintln!("release pipeline verify failed: {err}");
            return 1;
        }
    };
    println!(
        "release pipeline verify: total={} passed={} failed={} unchecked={} channel={} report={}",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.unchecked,
        report.channel,
        out_dir
            .join(xtask::release_pipeline::VERIFY_REPORT_FILE)
            .display(),
    );
    if report.summary.failed > 0 { 1 } else { 0 }
}

fn run_perf_harness_command(out: &str, strict: bool) -> i32 {
    let workspace_root = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("perf harness failed: unable to resolve current directory: {err}");
            return 1;
        }
    };
    let out_dir = workspace_root.join(out);
    let mut skeletons = vec![
        xtask::perf_harness::SkeletonDescriptor::m0_input_to_paint(),
        xtask::perf_harness::SkeletonDescriptor::m1_line_galley_shaping_cache(),
    ];
    for skeleton in &mut skeletons {
        xtask::perf_harness::apply_fail_on_budget_override(skeleton);
    }
    let package_name = "legion-desktop".to_string();
    let git_sha = xtask::perf_harness::resolve_workspace_git_sha(&workspace_root);
    let mut report = xtask::perf_harness::plan_perf_skeletons(&package_name, &git_sha, &skeletons);
    append_manual_renderer_measurement(&workspace_root, &out_dir, &mut report);
    let path = match xtask::perf_harness::write_report(&out_dir, &report) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("perf harness failed: {err}");
            return 1;
        }
    };
    println!(
        "perf harness: total={} passed={} failed={} skipped={} report={} strict={}",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.skipped,
        path.display(),
        strict,
    );
    for skeleton in &report.skeletons {
        println!(
            "  skeleton={} kind={} total_us={} p50_us={} p95_us={} budget_ms={} status={} message={}",
            skeleton.name,
            skeleton.kind.as_str(),
            skeleton.total_micros,
            skeleton.p50_micros,
            skeleton.p95_micros,
            skeleton.budget_millis,
            skeleton.status.as_str(),
            skeleton.message,
        );
    }
    if strict && report.summary.failed > 0 {
        1
    } else {
        0
    }
}

fn append_manual_renderer_measurement(
    workspace_root: &Path,
    out_dir: &Path,
    report: &mut xtask::perf_harness::PerfReport,
) {
    let manual_report_path = out_dir.join(xtask::perf_harness::MANUAL_RENDERER_PERF_REPORT_FILE);
    match fs::remove_file(&manual_report_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            report.skeletons.push(manual_renderer_placeholder_measurement(
                xtask::perf_harness::SkeletonStatus::Failed,
                format!(
                    "renderer-backed Manual measurement failed: unable to clear stale report `{}`: {err}",
                    manual_report_path.display()
                ),
            ));
            report.summary = xtask::perf_harness::summarize_measurements(&report.skeletons);
            return;
        }
    }

    let budgets = xtask::perf_harness::manual_renderer_budgets();
    let sample_count = budgets.sample_count.to_string();
    let output = process::Command::new("cargo")
        .current_dir(workspace_root)
        .args([
            "run",
            "--release",
            "-p",
            "legion-desktop",
            "--no-default-features",
            "--features",
            "offline",
            "--",
            "--manual-perf",
            "--workspace",
            ".",
            "--file",
            "Cargo.toml",
            "--perf-report",
        ])
        .arg(&manual_report_path)
        .args(["--perf-samples", &sample_count])
        .output();

    let measurement = match output {
        Err(err) => manual_renderer_placeholder_measurement(
            xtask::perf_harness::SkeletonStatus::Skipped,
            format!(
                "renderer-backed Manual measurement blocked: unable to spawn cargo release/offline desktop subprocess: {err}"
            ),
        ),
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "perf harness: Manual renderer subprocess exited with status {}",
                    output.status
                );
            }
            match xtask::perf_harness::read_manual_renderer_perf_report(&manual_report_path) {
                Ok(manual_report) => {
                    xtask::perf_harness::manual_renderer_perf_measurement(&manual_report)
                }
                Err(read_err) => {
                    eprintln!("perf harness: {read_err}");
                    let output_text = process_output_text(&output);
                    if !output.status.success() && manual_renderer_environment_blocked(&output_text)
                    {
                        manual_renderer_placeholder_measurement(
                            xtask::perf_harness::SkeletonStatus::Skipped,
                            format!(
                                "renderer-backed Manual measurement blocked: {}",
                                truncate_report_message(&output_text)
                            ),
                        )
                    } else {
                        manual_renderer_placeholder_measurement(
                            xtask::perf_harness::SkeletonStatus::Failed,
                            format!(
                                "renderer-backed Manual measurement failed: {read_err}{}",
                                command_output_suffix(&output_text)
                            ),
                        )
                    }
                }
            }
        }
    };

    report.skeletons.push(measurement);
    report.summary = xtask::perf_harness::summarize_measurements(&report.skeletons);
}

fn manual_renderer_placeholder_measurement(
    status: xtask::perf_harness::SkeletonStatus,
    message: String,
) -> xtask::perf_harness::SkeletonMeasurement {
    let budgets = xtask::perf_harness::manual_renderer_budgets();
    xtask::perf_harness::SkeletonMeasurement {
        name: "manual.renderer_input_to_paint".to_string(),
        kind: xtask::perf_harness::SkeletonKind::RendererBackedManualInputToPaint,
        fixture_bytes: 0,
        sample_count: budgets.sample_count,
        total_micros: 0,
        p50_micros: 0,
        p95_micros: 0,
        budget_millis: budgets.keypress_p95_millis.max(budgets.scroll_p95_millis),
        status,
        message,
    }
}

fn process_output_text(output: &process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("stdout:\n{stdout}\nstderr:\n{stderr}")
}

fn command_output_suffix(output_text: &str) -> String {
    let output_text = truncate_report_message(output_text);
    if output_text.is_empty() {
        String::new()
    } else {
        format!("; subprocess output: {output_text}")
    }
}

fn truncate_report_message(message: &str) -> String {
    let normalized = message.replace("\r\n", "\n");
    let trimmed = normalized.trim();
    const LIMIT: usize = 800;
    if trimmed.chars().count() <= LIMIT {
        trimmed.to_string()
    } else {
        format!("{}...", trimmed.chars().take(LIMIT).collect::<String>())
    }
}

fn manual_renderer_environment_blocked(output_text: &str) -> bool {
    let lower = output_text.to_ascii_lowercase();
    let renderer_context = lower.contains("renderer")
        || lower.contains("native")
        || lower.contains("window")
        || lower.contains("display")
        || lower.contains("gpu");
    let blocked_context = lower.contains("blocked")
        || lower.contains("unavailable")
        || lower.contains("not available")
        || lower.contains("headless")
        || lower.contains("display not set")
        || lower.contains("no display")
        || lower.contains("no available display")
        || lower.contains("renderer unavailable")
        || lower.contains("native window unavailable")
        || lower.contains("gpu unavailable");
    renderer_context && blocked_context
}

fn run_verify_perf_harness_command(out: &str, strict: bool) -> i32 {
    let workspace_root = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("perf harness verify failed: unable to resolve current directory: {err}");
            return 1;
        }
    };
    let out_dir = workspace_root.join(out);
    let report_path = out_dir.join(xtask::perf_harness::PERF_REPORT_FILE);
    let report = match xtask::perf_harness::read_report(&report_path) {
        Ok(report) => report,
        Err(err) => {
            eprintln!(
                "perf harness verify failed: {err} (run `cargo run -p xtask -- perf-harness` first)"
            );
            return 1;
        }
    };
    println!(
        "perf harness verify: total={} passed={} failed={} skipped={} report={} strict={}",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.skipped,
        report_path.display(),
        strict,
    );
    if strict && report.summary.failed > 0 {
        1
    } else {
        0
    }
}

fn run_legion_bench_command(out: &str, mode: &str, strict: bool) -> i32 {
    let workspace_root = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("legion bench failed: unable to resolve current directory: {err}");
            return 1;
        }
    };
    let out_dir = workspace_root.join(out);
    let mode = match parse_legion_bench_mode(mode) {
        Ok(mode) => mode,
        Err(err) => {
            eprintln!("legion bench failed: {err}");
            return 1;
        }
    };
    let suite = xtask::legion_bench::plan_default_legion_bench_suite();
    let git_sha = xtask::perf_harness::resolve_workspace_git_sha(&workspace_root);
    let report =
        xtask::legion_bench::plan_legion_bench_report("legion-desktop", &git_sha, mode, &suite);
    let path = match xtask::legion_bench::write_report(&out_dir, &report) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("legion bench failed: {err}");
            return 1;
        }
    };
    println!(
        "legion bench: total={} passed={} failed={} regressed={} report={} strict={} mode={} provider={} fingerprint={}",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.regressed,
        path.display(),
        strict,
        report.mode.as_str(),
        report.provider_profile,
        report.suite_fingerprint,
    );
    if strict && report.summary.failed > 0 {
        1
    } else {
        0
    }
}

fn run_verify_legion_bench_command(out: &str, strict: bool) -> i32 {
    let workspace_root = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("legion bench verify failed: unable to resolve current directory: {err}");
            return 1;
        }
    };
    let out_dir = workspace_root.join(out);
    let report_path = out_dir.join(xtask::legion_bench::BENCH_REPORT_FILE);
    let report = match xtask::legion_bench::read_report(&report_path) {
        Ok(report) => report,
        Err(err) => {
            eprintln!(
                "legion bench verify failed: {err} (run `cargo run -p xtask -- legion-bench` first)"
            );
            return 1;
        }
    };
    let suite = xtask::legion_bench::plan_default_legion_bench_suite();
    if let Err(err) = xtask::legion_bench::verify_legion_bench_report(&report, &suite) {
        eprintln!("legion bench verify failed: {err}");
        return 1;
    }
    println!(
        "legion bench verify: total={} passed={} failed={} regressed={} report={} strict={} mode={} provider={} fingerprint={}",
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.regressed,
        report_path.display(),
        strict,
        report.mode.as_str(),
        report.provider_profile,
        report.suite_fingerprint,
    );
    if strict && report.summary.failed > 0 {
        1
    } else {
        0
    }
}

fn parse_legion_bench_mode(value: &str) -> Result<xtask::legion_bench::LegionBenchRunMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "recorded" | "recorded_offline" | "offline" => {
            Ok(xtask::legion_bench::LegionBenchRunMode::RecordedOffline)
        }
        "live" | "live_weekly" | "weekly" => {
            Ok(xtask::legion_bench::LegionBenchRunMode::LiveWeekly)
        }
        other => Err(format!(
            "unknown legion-bench mode `{other}`; expected recorded or live"
        )),
    }
}

fn run_check_deps(policy_path: &str) -> Result<(), String> {
    let workspace_root =
        env::current_dir().map_err(|err| format!("unable to resolve current directory: {err}"))?;

    let policy_text = fs::read_to_string(policy_path)
        .map_err(|err| format!("unable to read policy at `{policy_path}`: {err}"))?;
    let policy = Policy::from_markdown(&policy_text)
        .map_err(|err| format!("unable to parse policy: {err}"))?;

    let metadata = load_workspace_metadata(&workspace_root)?;
    let packages = workspace_packages(&metadata);
    let package_dependency_names = workspace_package_dependency_names(&metadata);
    let violations = validate_dependency_policy(&packages, &policy);
    let renderer_violations =
        validate_renderer_dependency_gate(&policy_text, &package_dependency_names);
    let parser_violations =
        validate_parser_dependency_gate(&policy_text, &package_dependency_names);

    let protocol_violations = validate_protocol_contracts(
        &workspace_root.join(DEFAULT_PROTOCOL_PATH),
        policy.protocol_symbols(),
    )?;

    let phase3_evidence_path = workspace_root.join(DEFAULT_PHASE3_EVIDENCE_PATH);
    let phase3_evidence = fs::read_to_string(&phase3_evidence_path).map_err(|err| {
        format!(
            "unable to read Phase 3 evidence at `{}`: {err}",
            phase3_evidence_path.display()
        )
    })?;
    let phase3_evidence_dir = phase3_evidence_path
        .parent()
        .ok_or_else(|| "unable to resolve Phase 3 evidence directory".to_string())?;
    let phase3_violations = validate_phase3_acceptance_governance(&phase3_evidence, |artifact| {
        phase3_evidence_dir.join(artifact).is_file()
    });

    let phase4_evidence_path = workspace_root.join(DEFAULT_PHASE4_EVIDENCE_PATH);
    let phase4_evidence = fs::read_to_string(&phase4_evidence_path).map_err(|err| {
        format!(
            "unable to read Phase 4 evidence at `{}`: {err}",
            phase4_evidence_path.display()
        )
    })?;
    let phase4_evidence_dir = phase4_evidence_path
        .parent()
        .ok_or_else(|| "unable to resolve Phase 4 evidence directory".to_string())?;
    let phase4_violations = validate_phase4_acceptance_governance(&phase4_evidence, |artifact| {
        phase4_evidence_dir.join(artifact).is_file()
    });

    let phase5_evidence_path = workspace_root.join(DEFAULT_PHASE5_EVIDENCE_PATH);
    let phase5_evidence = fs::read_to_string(&phase5_evidence_path).map_err(|err| {
        format!(
            "unable to read Phase 5 evidence at `{}`: {err}",
            phase5_evidence_path.display()
        )
    })?;
    let phase5_evidence_dir = phase5_evidence_path
        .parent()
        .ok_or_else(|| "unable to resolve Phase 5 evidence directory".to_string())?;
    let phase5_violations = validate_phase5_acceptance_governance(&phase5_evidence, |artifact| {
        phase5_evidence_dir.join(artifact).is_file()
    });

    let gui_phase5_evidence_path = workspace_root.join(DEFAULT_GUI_PHASE5_EVIDENCE_PATH);
    let gui_phase5_evidence = fs::read_to_string(&gui_phase5_evidence_path).map_err(|err| {
        format!(
            "unable to read GUI Phase 5 evidence at `{}`: {err}",
            gui_phase5_evidence_path.display()
        )
    })?;
    let gui_phase5_violations =
        validate_gui_phase5_acceptance_governance(&gui_phase5_evidence, |artifact| {
            workspace_root.join(artifact).is_file()
        });

    let gui_phase6_evidence_path = workspace_root.join(DEFAULT_GUI_PHASE6_EVIDENCE_PATH);
    let gui_phase6_evidence = fs::read_to_string(&gui_phase6_evidence_path).map_err(|err| {
        format!(
            "unable to read GUI Phase 6 evidence at `{}`: {err}",
            gui_phase6_evidence_path.display()
        )
    })?;
    let gui_phase6_violations =
        validate_gui_phase6_acceptance_governance(&gui_phase6_evidence, |artifact| {
            workspace_root.join(artifact).is_file()
        });

    let gui_phase7_evidence_path = workspace_root.join(DEFAULT_GUI_PHASE7_EVIDENCE_PATH);
    let gui_phase7_evidence = fs::read_to_string(&gui_phase7_evidence_path).map_err(|err| {
        format!(
            "unable to read GUI Phase 7 evidence at `{}`: {err}",
            gui_phase7_evidence_path.display()
        )
    })?;
    let gui_phase7_violations =
        validate_gui_phase7_acceptance_governance(&gui_phase7_evidence, |artifact| {
            workspace_root.join(artifact).is_file()
        });

    let gui_phase8_evidence_path = workspace_root.join(DEFAULT_GUI_PHASE8_EVIDENCE_PATH);
    let gui_phase8_evidence = fs::read_to_string(&gui_phase8_evidence_path).map_err(|err| {
        format!(
            "unable to read GUI Phase 8 evidence at `{}`: {err}",
            gui_phase8_evidence_path.display()
        )
    })?;
    let gui_phase8_violations =
        validate_gui_phase8_acceptance_governance(&gui_phase8_evidence, |artifact| {
            workspace_root.join(artifact).is_file()
        });

    let phase13_evidence_path = workspace_root.join(DEFAULT_PHASE13_EVIDENCE_PATH);
    let phase13_evidence = fs::read_to_string(&phase13_evidence_path).map_err(|err| {
        format!(
            "unable to read Phase 13 evidence at `{}`: {err}",
            phase13_evidence_path.display()
        )
    })?;
    let phase13_final_gates_path = workspace_root.join(DEFAULT_PHASE13_FINAL_GATES_PATH);
    let phase13_final_gates = fs::read_to_string(&phase13_final_gates_path).map_err(|err| {
        format!(
            "unable to read Phase 13 final gates at `{}`: {err}",
            phase13_final_gates_path.display()
        )
    })?;
    let phase13_runbook_path = workspace_root.join(DEFAULT_PHASE13_RUNBOOK_PATH);
    let phase13_runbook = fs::read_to_string(&phase13_runbook_path).map_err(|err| {
        format!(
            "unable to read Phase 13 runbook at `{}`: {err}",
            phase13_runbook_path.display()
        )
    })?;
    let phase13_violations = validate_phase13_acceptance_evidence(
        &phase13_evidence,
        &phase13_final_gates,
        &phase13_runbook,
        |artifact| workspace_root.join(artifact).is_file(),
    );

    let phase6_evidence_path = workspace_root.join(DEFAULT_PHASE6_EVIDENCE_PATH);
    let phase6_evidence = fs::read_to_string(&phase6_evidence_path).map_err(|err| {
        format!(
            "unable to read Phase 6 evidence at `{}`: {err}",
            phase6_evidence_path.display()
        )
    })?;
    let phase6_evidence_dir = phase6_evidence_path
        .parent()
        .ok_or_else(|| "unable to resolve Phase 6 evidence directory".to_string())?;
    let phase6_violations = validate_phase6_acceptance_governance(&phase6_evidence, |artifact| {
        phase6_evidence_dir.join(artifact).is_file()
    });

    let phase7_evidence_path = workspace_root.join(DEFAULT_PHASE7_EVIDENCE_PATH);
    let phase7_evidence = fs::read_to_string(&phase7_evidence_path).map_err(|err| {
        format!(
            "unable to read Phase 7 evidence at `{}`: {err}",
            phase7_evidence_path.display()
        )
    })?;
    let phase7_evidence_dir = phase7_evidence_path
        .parent()
        .ok_or_else(|| "unable to resolve Phase 7 evidence directory".to_string())?;
    let phase7_violations = validate_phase7_acceptance_governance(&phase7_evidence, |artifact| {
        phase7_evidence_dir.join(artifact).is_file()
    });

    let phase8_evidence_path = workspace_root.join(DEFAULT_PHASE8_EVIDENCE_PATH);
    let phase8_evidence = fs::read_to_string(&phase8_evidence_path).map_err(|err| {
        format!(
            "unable to read Phase 8 evidence at `{}`: {err}",
            phase8_evidence_path.display()
        )
    })?;
    let phase8_evidence_dir = phase8_evidence_path
        .parent()
        .ok_or_else(|| "unable to resolve Phase 8 evidence directory".to_string())?;
    let phase8_violations = validate_phase8_acceptance_governance(
        &phase8_evidence,
        |artifact| phase8_evidence_dir.join(artifact).is_file(),
        |artifact| fs::read_to_string(phase8_evidence_dir.join(artifact)).ok(),
    );

    let mut all = violations;
    all.extend(renderer_violations);
    all.extend(parser_violations);
    all.extend(protocol_violations);
    all.extend(phase3_violations);
    all.extend(phase4_violations);
    all.extend(phase5_violations);
    all.extend(gui_phase5_violations);
    all.extend(gui_phase6_violations);
    all.extend(gui_phase7_violations);
    all.extend(gui_phase8_violations);
    all.extend(phase13_violations);
    all.extend(phase6_violations);
    all.extend(phase7_violations);
    all.extend(phase8_violations);

    if !all.is_empty() {
        let mut output = String::new();
        output.push_str("dependency policy violations:\n");
        for item in all {
            output.push_str(&format!("- {item}\n"));
        }
        return Err(output);
    }

    Ok(())
}

fn load_workspace_metadata(workspace_root: &Path) -> Result<Metadata, String> {
    MetadataCommand::new()
        .current_dir(workspace_root)
        .manifest_path(workspace_root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .map_err(|err| format!("cargo metadata failed: {err}"))
}

fn workspace_packages(metadata: &Metadata) -> HashMap<String, HashSet<String>> {
    let internal = metadata
        .packages
        .iter()
        .filter(|package| package.source.is_none())
        .map(|package| package.name.clone())
        .collect::<HashSet<_>>();

    metadata
        .packages
        .iter()
        .filter(|package| internal.contains(&package.name))
        .map(|package| {
            let package_deps = package
                .dependencies
                .iter()
                .filter(|dep| dep.kind == cargo_metadata::DependencyKind::Normal)
                .filter(|dep| internal.contains(&dep.name))
                .map(|dep| dep.name.clone())
                .collect::<HashSet<_>>();

            (package.name.clone(), package_deps)
        })
        .collect()
}

fn workspace_package_dependency_names(metadata: &Metadata) -> HashMap<String, HashSet<String>> {
    metadata
        .packages
        .iter()
        .filter(|package| package.source.is_none())
        .map(|package| {
            let dependencies = package
                .dependencies
                .iter()
                .map(|dependency| dependency.name.clone())
                .collect();

            (package.name.clone(), dependencies)
        })
        .collect()
}

fn validate_dependency_policy(
    packages: &HashMap<String, HashSet<String>>,
    policy: &Policy,
) -> Vec<String> {
    let mut issues = Vec::new();

    // Structural rule set defined by the policy.
    let forbidden_pairs = policy.forbidden_pairs();

    let mut sources = packages.keys().cloned().collect::<Vec<_>>();
    sources.sort();

    for source in sources {
        let deps = packages
            .get(&source)
            .expect("sorted package source must exist in package map");
        let Some(allowed_deps) = policy.allowed_internal(&source) else {
            issues.push(format!(
                "`{source}` lacks dependency policy coverage in `plans/dependency-policy.md`"
            ));
            continue;
        };

        let mut unexpected: Vec<String> = deps
            .iter()
            .filter(|dep| !allowed_deps.contains(*dep))
            .cloned()
            .collect();
        unexpected.sort();
        for unexpected_dep in unexpected {
            issues.push(format!(
                "`{source}` depends on `{unexpected_dep}`, which is not in the allowed policy set"
            ));
        }
    }

    let mut forbidden = forbidden_pairs.iter().cloned().collect::<Vec<_>>();
    forbidden.sort();
    for (source, destination) in forbidden {
        if let Some(deps) = packages.get(&source)
            && deps.contains(&destination)
        {
            issues.push(format!(
                "forbidden dependency `{source}` -> `{destination}` detected"
            ));
        }
    }

    let mut required = policy.required_dependencies().iter().collect::<Vec<_>>();
    required.sort_by_key(|(source, _)| *source);
    for (source, required_targets) in required {
        let Some(deps) = packages.get(source) else {
            continue;
        };

        let mut required_targets = required_targets.iter().cloned().collect::<Vec<_>>();
        required_targets.sort();
        for required in required_targets {
            if !deps.contains(&required) {
                issues.push(format!("`{source}` is required to depend on `{required}`"));
            }
        }
    }

    issues.sort();
    issues
}

fn validate_renderer_dependency_gate(
    policy_text: &str,
    package_dependencies: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    let mut issues = Vec::new();

    for marker in RENDERER_BOUNDARY_POLICY_MARKERS {
        if !policy_text.contains(marker) {
            issues.push(format!(
                "`plans/dependency-policy.md` must document renderer boundary marker `{marker}`"
            ));
        }
    }

    let allowed_packages = RENDERER_DEPENDENCY_ALLOWED_PACKAGES
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut packages = package_dependencies.keys().collect::<Vec<_>>();
    packages.sort();

    for package in packages {
        if allowed_packages.contains(package.as_str()) {
            continue;
        }

        let dependencies = package_dependencies
            .get(package)
            .expect("sorted package key must exist in dependency map");
        let mut forbidden_declared = FORBIDDEN_RENDERER_DEPS
            .iter()
            .filter(|dependency| dependencies.contains(**dependency))
            .copied()
            .collect::<Vec<_>>();
        forbidden_declared.sort();

        if !forbidden_declared.is_empty() {
            let package_label = if package == "legion-ui" {
                format!("`{DEFAULT_UI_MANIFEST_PATH}`")
            } else {
                format!("workspace package `{package}`")
            };
            issues.push(format!(
                "{package_label} must not declare renderer/windowing dependencies outside `legion-desktop`: {}",
                forbidden_declared.join(", ")
            ));
        }
    }

    issues.sort();
    issues
}

fn validate_parser_dependency_gate(
    policy_text: &str,
    package_dependencies: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    let mut issues = Vec::new();

    for marker in PARSER_BOUNDARY_POLICY_MARKERS {
        if !policy_text.contains(marker) {
            issues.push(format!(
                "`plans/dependency-policy.md` must document parser boundary marker `{marker}`"
            ));
        }
    }

    let allowed_packages = PARSER_DEPENDENCY_ALLOWED_PACKAGES
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut packages = package_dependencies.keys().collect::<Vec<_>>();
    packages.sort();

    for package in packages {
        if allowed_packages.contains(package.as_str()) {
            continue;
        }

        let dependencies = package_dependencies
            .get(package)
            .expect("sorted package key must exist in dependency map");
        let mut forbidden_declared = FORBIDDEN_PARSER_DEPS
            .iter()
            .filter(|dependency| dependencies.contains(**dependency))
            .copied()
            .collect::<Vec<_>>();
        forbidden_declared.sort();

        if !forbidden_declared.is_empty() {
            issues.push(format!(
                "workspace package `{package}` must not declare parser/runtime dependencies outside `legion-index`: {}",
                forbidden_declared.join(", ")
            ));
        }
    }

    issues.sort();
    issues
}

fn validate_protocol_contracts(
    protocol_file: &Path,
    expected_symbols: &HashSet<String>,
) -> Result<Vec<String>, String> {
    let protocol_text = fs::read_to_string(protocol_file).map_err(|err| {
        format!(
            "unable to read protocol file `{}`: {err}",
            protocol_file.display()
        )
    })?;

    let missing = expected_symbols
        .iter()
        .filter(|symbol| !protocol_contains_symbol(&protocol_text, symbol))
        .map(|symbol| format!("protocol contract symbol `{symbol}` missing from `crates/legion-protocol/src/lib.rs`"))
        .collect();

    Ok(missing)
}

fn protocol_contains_symbol(text: &str, symbol: &str) -> bool {
    for line in text.lines() {
        let line = line.trim();
        if protocol_definition_has_token(line, "struct", symbol)
            || protocol_definition_has_token(line, "enum", symbol)
            || protocol_definition_has_token(line, "trait", symbol)
            || protocol_definition_has_token(line, "type", symbol)
        {
            return true;
        }
    }

    false
}

fn protocol_definition_has_token(line: &str, keyword: &str, symbol: &str) -> bool {
    let mut words = line.split_whitespace();
    match words.next() {
        Some("pub") => {
            let Some(second_word) = words.next() else {
                return false;
            };
            if second_word != keyword {
                return false;
            }
        }
        Some(word) if word == keyword => {}
        _ => return false,
    }

    let Some(candidate_symbol) = words.next() else {
        return false;
    };

    let Some(found_symbol) = candidate_symbol
        .split(&['(', ';', ':', '{', '<', '[', ','][..])
        .next()
    else {
        return false;
    };

    found_symbol == symbol
}

fn validate_phase3_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE3_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_PHASE3_EVIDENCE_PATH}` must include `{PHASE3_STATUS_HEADING}` with explicit Phase 3 and LSP acceptance status"
        ));
        return issues;
    };

    let phase3_not_accepted = status_section.contains(PHASE3_NOT_ACCEPTED_MARKER);
    let phase3_accepted = status_section.contains(PHASE3_ACCEPTED_MARKER);
    match (phase3_not_accepted, phase3_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_PHASE3_EVIDENCE_PATH}` must not declare both `{PHASE3_NOT_ACCEPTED_MARKER}` and `{PHASE3_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_PHASE3_EVIDENCE_PATH}` must declare either `{PHASE3_NOT_ACCEPTED_MARKER}` or `{PHASE3_ACCEPTED_MARKER}`"
        )),
    }

    let lsp_not_accepted = status_section.contains(LSP_NOT_ACCEPTED_MARKER);
    let lsp_accepted = status_section.contains(LSP_ACCEPTED_MARKER);
    match (lsp_not_accepted, lsp_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_PHASE3_EVIDENCE_PATH}` must not declare both `{LSP_NOT_ACCEPTED_MARKER}` and `{LSP_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_PHASE3_EVIDENCE_PATH}` must declare either `{LSP_NOT_ACCEPTED_MARKER}` or `{LSP_ACCEPTED_MARKER}`"
        )),
    }

    if (phase3_not_accepted || lsp_not_accepted)
        && !status_section.contains(PHASE3_PARTIAL_RUNTIME_MARKER)
    {
        issues.push(format!(
            "`{DEFAULT_PHASE3_EVIDENCE_PATH}` partial acceptance status must state `{PHASE3_PARTIAL_RUNTIME_MARKER}`"
        ));
    }

    if phase3_accepted || lsp_accepted {
        issues.extend(validate_phase3_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_phase3_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if evidence.contains("This document is not implementation evidence yet") {
        issues.push(format!(
            "`{DEFAULT_PHASE3_EVIDENCE_PATH}` claims acceptance while still saying it is not implementation evidence yet"
        ));
    }

    if let Some(checklist) = markdown_section(evidence, PHASE3_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_PHASE3_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_PHASE3_EVIDENCE_PATH}` claims acceptance but `{PHASE3_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in PHASE3_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE3_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE3_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing from `plans/evidence/phase-3`"
            ));
        }
    }

    issues
}

fn validate_phase4_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE4_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_PHASE4_EVIDENCE_PATH}` must include `{PHASE4_STATUS_HEADING}` with explicit Phase 4 acceptance status"
        ));
        return issues;
    };

    let phase4_not_accepted = status_section.contains(PHASE4_NOT_ACCEPTED_MARKER);
    let phase4_accepted = status_section.contains(PHASE4_ACCEPTED_MARKER);
    match (phase4_not_accepted, phase4_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_PHASE4_EVIDENCE_PATH}` must not declare both `{PHASE4_NOT_ACCEPTED_MARKER}` and `{PHASE4_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_PHASE4_EVIDENCE_PATH}` must declare either `{PHASE4_NOT_ACCEPTED_MARKER}` or `{PHASE4_ACCEPTED_MARKER}`"
        )),
    }

    if phase4_accepted {
        issues.extend(validate_phase4_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_phase4_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if evidence.contains("This document is not implementation evidence yet") {
        issues.push(format!(
            "`{DEFAULT_PHASE4_EVIDENCE_PATH}` claims acceptance while still saying it is not implementation evidence yet"
        ));
    }

    if let Some(checklist) = markdown_section(evidence, PHASE4_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_PHASE4_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_PHASE4_EVIDENCE_PATH}` claims acceptance but `{PHASE4_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in PHASE4_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE4_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE4_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing from `plans/evidence/phase-4`"
            ));
        }
    }

    issues
}

fn validate_phase5_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE5_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_PHASE5_EVIDENCE_PATH}` must include `{PHASE5_STATUS_HEADING}` with explicit Phase 5 acceptance status"
        ));
        return issues;
    };

    let phase5_not_accepted = status_section.contains(PHASE5_NOT_ACCEPTED_MARKER);
    let phase5_accepted = status_section.contains(PHASE5_ACCEPTED_MARKER);
    match (phase5_not_accepted, phase5_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_PHASE5_EVIDENCE_PATH}` must not declare both `{PHASE5_NOT_ACCEPTED_MARKER}` and `{PHASE5_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_PHASE5_EVIDENCE_PATH}` must declare either `{PHASE5_NOT_ACCEPTED_MARKER}` or `{PHASE5_ACCEPTED_MARKER}`"
        )),
    }

    if phase5_accepted {
        issues.extend(validate_phase5_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_phase5_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if let Some(checklist) = markdown_section(evidence, PHASE5_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_PHASE5_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_PHASE5_EVIDENCE_PATH}` claims acceptance but `{PHASE5_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in PHASE5_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE5_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE5_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing from `plans/evidence/phase-5`"
            ));
        }
    }

    issues
}

fn validate_gui_phase5_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE5_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE5_EVIDENCE_PATH}` must include `{PHASE5_STATUS_HEADING}` with explicit GUI Phase 5 acceptance status"
        ));
        return issues;
    };

    let phase5_not_accepted = status_section.contains(PHASE5_NOT_ACCEPTED_MARKER);
    let phase5_accepted = status_section.contains(PHASE5_ACCEPTED_MARKER);
    match (phase5_not_accepted, phase5_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_GUI_PHASE5_EVIDENCE_PATH}` must not declare both `{PHASE5_NOT_ACCEPTED_MARKER}` and `{PHASE5_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_GUI_PHASE5_EVIDENCE_PATH}` must declare either `{PHASE5_NOT_ACCEPTED_MARKER}` or `{PHASE5_ACCEPTED_MARKER}`"
        )),
    }

    if phase5_accepted {
        issues.extend(validate_gui_phase5_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_gui_phase5_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if let Some(checklist) = markdown_section(evidence, PHASE5_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE5_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE5_EVIDENCE_PATH}` claims acceptance but `{PHASE5_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in GUI_PHASE5_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE5_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE5_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing"
            ));
        }
    }

    for command in GUI_PHASE5_REQUIRED_COMMAND_MARKERS {
        if !contains_current_or_historical_marker(evidence, command) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE5_EVIDENCE_PATH}` claims acceptance but required command `{command}` is not listed"
            ));
        }
    }

    issues
}

fn validate_gui_phase6_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE6_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE6_EVIDENCE_PATH}` must include `{PHASE6_STATUS_HEADING}` with explicit GUI Phase 6 acceptance status"
        ));
        return issues;
    };

    let phase6_not_accepted = status_section.contains(PHASE6_NOT_ACCEPTED_MARKER);
    let phase6_accepted = status_section.contains(PHASE6_ACCEPTED_MARKER);
    match (phase6_not_accepted, phase6_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_GUI_PHASE6_EVIDENCE_PATH}` must not declare both `{PHASE6_NOT_ACCEPTED_MARKER}` and `{PHASE6_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_GUI_PHASE6_EVIDENCE_PATH}` must declare either `{PHASE6_NOT_ACCEPTED_MARKER}` or `{PHASE6_ACCEPTED_MARKER}`"
        )),
    }

    if phase6_accepted {
        issues.extend(validate_gui_phase6_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_gui_phase6_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if let Some(checklist) = markdown_section(evidence, PHASE6_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE6_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE6_EVIDENCE_PATH}` claims acceptance but `{PHASE6_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in GUI_PHASE6_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE6_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE6_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing"
            ));
        }
    }

    for command in GUI_PHASE6_REQUIRED_COMMAND_MARKERS {
        if !contains_current_or_historical_marker(evidence, command) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE6_EVIDENCE_PATH}` claims acceptance but required command `{command}` is not listed"
            ));
        }
    }

    issues
}

fn validate_gui_phase7_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE7_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` must include `{PHASE7_STATUS_HEADING}` with explicit GUI Phase 7 acceptance status"
        ));
        return issues;
    };

    let phase7_not_accepted = status_section.contains(PHASE7_NOT_ACCEPTED_MARKER);
    let phase7_accepted = status_section.contains(PHASE7_ACCEPTED_MARKER);
    match (phase7_not_accepted, phase7_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` must not declare both `{PHASE7_NOT_ACCEPTED_MARKER}` and `{PHASE7_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` must declare either `{PHASE7_NOT_ACCEPTED_MARKER}` or `{PHASE7_ACCEPTED_MARKER}`"
        )),
    }

    if phase7_accepted {
        issues.extend(validate_gui_phase7_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_gui_phase7_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if evidence.contains("This document is GUI Phase 7 scaffold evidence") {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` claims acceptance while still saying it is scaffold evidence"
        ));
    }

    if let Some(checklist) = markdown_section(evidence, PHASE7_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` claims acceptance but `{PHASE7_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in GUI_PHASE7_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing"
            ));
        }
    }

    for command in GUI_PHASE7_REQUIRED_COMMAND_MARKERS {
        if !contains_current_or_historical_marker(evidence, command) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` claims acceptance but required command `{command}` is not listed"
            ));
        }
    }

    for marker in GUI_PHASE7_REQUIRED_LIMITATION_MARKERS {
        if !evidence.contains(marker) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE7_EVIDENCE_PATH}` claims acceptance but required limitation marker `{marker}` is not listed"
            ));
        }
    }

    issues
}

fn validate_gui_phase8_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE8_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` must include `{PHASE8_STATUS_HEADING}` with explicit GUI Phase 8 acceptance status"
        ));
        return issues;
    };

    let phase8_not_accepted = status_section.contains(PHASE8_NOT_ACCEPTED_MARKER);
    let phase8_accepted = status_section.contains(PHASE8_ACCEPTED_MARKER);
    match (phase8_not_accepted, phase8_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` must not declare both `{PHASE8_NOT_ACCEPTED_MARKER}` and `{PHASE8_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` must declare either `{PHASE8_NOT_ACCEPTED_MARKER}` or `{PHASE8_ACCEPTED_MARKER}`"
        )),
    }

    if phase8_accepted {
        issues.extend(validate_gui_phase8_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_gui_phase8_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if evidence.contains("This document is GUI Phase 8 scaffold evidence") {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance while still saying it is scaffold evidence"
        ));
    }

    if let Some(checklist) = markdown_section(evidence, PHASE8_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance but `{PHASE8_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in GUI_PHASE8_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing"
            ));
        }
    }

    for command in GUI_PHASE8_REQUIRED_COMMAND_MARKERS {
        if !contains_current_or_historical_marker(evidence, command) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance but required command `{command}` is not listed"
            ));
        }
    }

    for marker in GUI_PHASE8_REQUIRED_SURFACE_MARKERS {
        if !evidence.contains(marker) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance but required supported surface marker `{marker}` is not listed"
            ));
        }
    }

    for marker in GUI_PHASE8_REQUIRED_PLATFORM_MARKERS {
        if !evidence.contains(marker) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance but required platform marker `{marker}` is not listed"
            ));
        }
    }

    for marker in GUI_PHASE8_STALE_UNSUPPORTED_MARKERS {
        if evidence.contains(marker) {
            issues.push(format!(
                "`{DEFAULT_GUI_PHASE8_EVIDENCE_PATH}` claims acceptance while still containing stale unsupported marker `{marker}`"
            ));
        }
    }

    issues
}

fn validate_phase13_acceptance_evidence<F>(
    evidence: &str,
    final_gates: &str,
    runbook: &str,
    artifact_exists: F,
) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    for marker in PHASE13_REQUIRED_EVIDENCE_MARKERS {
        if !evidence.contains(marker) {
            issues.push(format!(
                "`{DEFAULT_PHASE13_EVIDENCE_PATH}` is missing required marker `{marker}`"
            ));
        }
    }

    if let Some(checklist) = markdown_section(evidence, PHASE13_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_PHASE13_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_PHASE13_EVIDENCE_PATH}` must include `{PHASE13_FINAL_CHECKLIST_HEADING}`"
        ));
    }

    for artifact in PHASE13_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE13_EVIDENCE_PATH}` is missing required artifact listing `{artifact}`"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE13_EVIDENCE_PATH}` references missing required artifact `{artifact}`"
            ));
        }
    }

    for command in PHASE13_REQUIRED_COMMAND_MARKERS {
        if !contains_current_or_historical_marker(evidence, command) {
            issues.push(format!(
                "`{DEFAULT_PHASE13_EVIDENCE_PATH}` is missing required command `{command}`"
            ));
        }

        if !contains_current_or_historical_marker(final_gates, command) {
            issues.push(format!(
                "`{DEFAULT_PHASE13_FINAL_GATES_PATH}` is missing required command `{command}`"
            ));
        }
    }

    if !final_gates.contains("Final gate outputs archived from current commands") {
        issues.push(format!(
            "`{DEFAULT_PHASE13_FINAL_GATES_PATH}` must state `Final gate outputs archived from current commands`"
        ));
    }

    for marker in PHASE13_REQUIRED_RUNBOOK_MARKERS {
        if !runbook.contains(marker) {
            issues.push(format!(
                "`{DEFAULT_PHASE13_RUNBOOK_PATH}` is missing required runbook marker `{marker}`"
            ));
        }
    }

    for marker in PHASE13_STALE_ACCEPTANCE_MARKERS {
        if evidence.contains(marker) || final_gates.contains(marker) {
            issues.push(format!(
                "Phase 13 accepted evidence contains stale marker `{marker}`"
            ));
        }
    }

    issues.sort();
    issues
}

fn validate_phase6_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE6_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_PHASE6_EVIDENCE_PATH}` must include `{PHASE6_STATUS_HEADING}` with explicit Phase 6 acceptance status"
        ));
        return issues;
    };

    let phase6_not_accepted = status_section.contains(PHASE6_NOT_ACCEPTED_MARKER);
    let phase6_accepted = status_section.contains(PHASE6_ACCEPTED_MARKER);
    match (phase6_not_accepted, phase6_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_PHASE6_EVIDENCE_PATH}` must not declare both `{PHASE6_NOT_ACCEPTED_MARKER}` and `{PHASE6_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_PHASE6_EVIDENCE_PATH}` must declare either `{PHASE6_NOT_ACCEPTED_MARKER}` or `{PHASE6_ACCEPTED_MARKER}`"
        )),
    }

    if phase6_accepted {
        issues.extend(validate_phase6_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_phase6_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if evidence.contains("This document is Phase 6 scaffold evidence, not acceptance evidence yet")
    {
        issues.push(format!(
            "`{DEFAULT_PHASE6_EVIDENCE_PATH}` claims acceptance while still saying it is scaffold evidence"
        ));
    }

    if let Some(checklist) = markdown_section(evidence, PHASE6_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_PHASE6_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_PHASE6_EVIDENCE_PATH}` claims acceptance but `{PHASE6_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in PHASE6_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE6_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE6_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing from `plans/evidence/phase-6`"
            ));
        }
    }

    issues
}

fn validate_phase7_acceptance_governance<F>(evidence: &str, artifact_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE7_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_PHASE7_EVIDENCE_PATH}` must include `{PHASE7_STATUS_HEADING}` with explicit Phase 7 acceptance status"
        ));
        return issues;
    };

    let phase7_not_accepted = status_section.contains(PHASE7_NOT_ACCEPTED_MARKER);
    let phase7_accepted = status_section.contains(PHASE7_ACCEPTED_MARKER);
    match (phase7_not_accepted, phase7_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_PHASE7_EVIDENCE_PATH}` must not declare both `{PHASE7_NOT_ACCEPTED_MARKER}` and `{PHASE7_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_PHASE7_EVIDENCE_PATH}` must declare either `{PHASE7_NOT_ACCEPTED_MARKER}` or `{PHASE7_ACCEPTED_MARKER}`"
        )),
    }

    if phase7_accepted {
        issues.extend(validate_phase7_completion_evidence(
            evidence,
            &artifact_exists,
        ));
    }

    issues.sort();
    issues
}

fn validate_phase7_completion_evidence<F>(evidence: &str, artifact_exists: &F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();

    if evidence.contains("This document is Phase 7 scaffold evidence, not acceptance evidence yet")
    {
        issues.push(format!(
            "`{DEFAULT_PHASE7_EVIDENCE_PATH}` claims acceptance while still saying it is scaffold evidence"
        ));
    }

    if let Some(checklist) = markdown_section(evidence, PHASE7_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_PHASE7_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_PHASE7_EVIDENCE_PATH}` claims acceptance but `{PHASE7_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in PHASE7_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE7_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE7_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing from `plans/evidence/phase-7`"
            ));
        }
    }

    issues
}

fn validate_phase8_acceptance_governance<F, G>(
    evidence: &str,
    artifact_exists: F,
    artifact_text: G,
) -> Vec<String>
where
    F: Fn(&str) -> bool,
    G: Fn(&str) -> Option<String>,
{
    let mut issues = Vec::new();

    let Some(status_section) = markdown_section(evidence, PHASE8_STATUS_HEADING) else {
        issues.push(format!(
            "`{DEFAULT_PHASE8_EVIDENCE_PATH}` must include `{PHASE8_STATUS_HEADING}` with explicit Phase 8 acceptance status"
        ));
        return issues;
    };

    let phase8_not_accepted = status_section.contains(PHASE8_NOT_ACCEPTED_MARKER);
    let phase8_accepted = status_section.contains(PHASE8_ACCEPTED_MARKER);
    match (phase8_not_accepted, phase8_accepted) {
        (true, false) | (false, true) => {}
        (true, true) => issues.push(format!(
            "`{DEFAULT_PHASE8_EVIDENCE_PATH}` must not declare both `{PHASE8_NOT_ACCEPTED_MARKER}` and `{PHASE8_ACCEPTED_MARKER}`"
        )),
        (false, false) => issues.push(format!(
            "`{DEFAULT_PHASE8_EVIDENCE_PATH}` must declare either `{PHASE8_NOT_ACCEPTED_MARKER}` or `{PHASE8_ACCEPTED_MARKER}`"
        )),
    }

    if phase8_accepted {
        issues.extend(validate_phase8_completion_evidence(
            evidence,
            &artifact_exists,
            &artifact_text,
        ));
    }

    issues.sort();
    issues
}

fn validate_phase8_completion_evidence<F, G>(
    evidence: &str,
    artifact_exists: &F,
    artifact_text: &G,
) -> Vec<String>
where
    F: Fn(&str) -> bool,
    G: Fn(&str) -> Option<String>,
{
    let mut issues = Vec::new();

    if evidence.contains("This document is Phase 8 scaffold evidence, not acceptance evidence yet")
    {
        issues.push(format!(
            "`{DEFAULT_PHASE8_EVIDENCE_PATH}` claims acceptance while still saying it is scaffold evidence"
        ));
    }

    for marker in PHASE8_STALE_DEFERRED_MARKERS {
        if evidence.contains(marker) {
            issues.push(format!(
                "`{DEFAULT_PHASE8_EVIDENCE_PATH}` claims acceptance while still containing stale deferred marker `{marker}`"
            ));
        }
    }

    for marker in PHASE8_ACCEPTED_REQUIRED_MARKERS {
        if !evidence.contains(marker) {
            issues.push(format!(
                "`{DEFAULT_PHASE8_EVIDENCE_PATH}` claims acceptance but final GA marker `{marker}` is missing"
            ));
        }
    }

    if let Some(checklist) = markdown_section(evidence, PHASE8_FINAL_CHECKLIST_HEADING) {
        if checklist
            .lines()
            .any(|line| line.trim_start().starts_with("- [ ]"))
        {
            issues.push(format!(
                "`{DEFAULT_PHASE8_EVIDENCE_PATH}` claims acceptance while final validation checklist items remain unchecked"
            ));
        }
    } else {
        issues.push(format!(
            "`{DEFAULT_PHASE8_EVIDENCE_PATH}` claims acceptance but `{PHASE8_FINAL_CHECKLIST_HEADING}` is missing"
        ));
    }

    for artifact in PHASE8_REQUIRED_ARTIFACTS {
        if !evidence.contains(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE8_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is not listed"
            ));
        }

        if !artifact_exists(artifact) {
            issues.push(format!(
                "`{DEFAULT_PHASE8_EVIDENCE_PATH}` claims acceptance but required artifact `{artifact}` is missing from `plans/evidence/phase-8`"
            ));
        }
    }

    issues.extend(validate_phase8_final_artifact_contents(artifact_text));

    issues
}

fn validate_phase8_final_artifact_contents<G>(artifact_text: &G) -> Vec<String>
where
    G: Fn(&str) -> Option<String>,
{
    let mut issues = Vec::new();
    match artifact_text(PHASE8_PLATFORM_MATRIX_ARTIFACT) {
        Some(matrix) => {
            for marker in PHASE8_PLATFORM_MATRIX_REQUIRED_MARKERS {
                if !contains_current_or_historical_marker(&matrix, marker) {
                    issues.push(format!(
                        "`{PHASE8_PLATFORM_MATRIX_ARTIFACT}` is required for accepted Phase 8 but marker `{marker}` is missing"
                    ));
                }
            }
            for marker in PHASE8_ACCEPTED_ARTIFACT_STALE_MARKERS {
                if contains_phase8_accepted_artifact_stale_marker(&matrix, marker) {
                    issues.push(format!(
                        "`{PHASE8_PLATFORM_MATRIX_ARTIFACT}` is required for accepted Phase 8 but still contains stale marker `{marker}`"
                    ));
                }
            }
        }
        None => issues.push(format!(
            "`{PHASE8_PLATFORM_MATRIX_ARTIFACT}` is required for accepted Phase 8 but could not be read"
        )),
    }

    match artifact_text(PHASE8_RELEASE_READINESS_ARTIFACT) {
        Some(release) => {
            for marker in PHASE8_RELEASE_SIGNOFF_REQUIRED_MARKERS {
                if !contains_current_or_historical_marker(&release, marker) {
                    issues.push(format!(
                        "`{PHASE8_RELEASE_READINESS_ARTIFACT}` is required for accepted Phase 8 but signoff marker `{marker}` is missing"
                    ));
                }
            }
            for marker in PHASE8_ACCEPTED_ARTIFACT_STALE_MARKERS {
                if contains_phase8_accepted_artifact_stale_marker(&release, marker) {
                    issues.push(format!(
                        "`{PHASE8_RELEASE_READINESS_ARTIFACT}` is required for accepted Phase 8 but still contains stale marker `{marker}`"
                    ));
                }
            }
        }
        None => issues.push(format!(
            "`{PHASE8_RELEASE_READINESS_ARTIFACT}` is required for accepted Phase 8 but could not be read"
        )),
    }

    issues
}

fn contains_phase8_accepted_artifact_stale_marker(source: &str, marker: &str) -> bool {
    match marker {
        "pending" => contains_ascii_token_case_insensitive(source, "pending"),
        "TODO" => contains_ascii_token(source, "TODO"),
        _ => source.contains(marker),
    }
}

fn contains_current_or_historical_marker(source: &str, marker: &str) -> bool {
    if source.contains(marker) {
        return true;
    }

    let historical = marker
        .replace("legion-", "devil-")
        .replace("legion_", "devil_");
    historical != marker && source.contains(&historical)
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

fn markdown_section<'a>(source: &'a str, heading: &str) -> Option<&'a str> {
    let start = source.find(heading)?;
    let tail = &source[start..];
    let body_start = tail.find('\n').map_or(tail.len(), |idx| idx + 1);
    let body = &tail[body_start..];
    let end = body.find("\n## ").unwrap_or(body.len());
    Some(&body[..end])
}

#[derive(Default)]
struct Policy {
    // Crate -> allowed internal workspace dependencies.
    allowed: HashMap<String, HashSet<String>>,
    // Crate -> required direct dependencies.
    required: HashMap<String, HashSet<String>>,
    // Explicitly forbidden crate dependency pairs.
    forbidden: HashSet<(String, String)>,
    // Boundary symbols expected to exist in protocol crate.
    protocol_symbols: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectionalList {
    Allowed,
    Required,
}

impl Policy {
    fn from_markdown(source: &str) -> Result<Self, String> {
        let mut policy = Self::default();

        let mut section = String::new();
        let mut active_crate: Option<String> = None;
        let mut active_list = DirectionalList::Allowed;

        for raw_line in source.lines() {
            let line = raw_line.trim();
            let items = extract_backticked_items(line);

            match line {
                l if l.starts_with("### 1.") => {
                    section = "directional".to_string();
                    active_crate = None;
                }
                l if l.starts_with("### 2.") => {
                    section = "contracts".to_string();
                    active_crate = None;
                }
                l if l.starts_with("###") => {
                    section.clear();
                    active_crate = None;
                }
                _ => {}
            }

            match section.as_str() {
                "directional" => {
                    if line.contains("MUST NOT depend on") {
                        if items.len() >= 2 {
                            policy
                                .forbidden
                                .insert((items[0].clone(), items[1].clone()));
                        }
                        active_crate = None;
                        continue;
                    }

                    if line.contains("MUST directly depend on") || line.contains("MUST depend on") {
                        if let Some(crate_name) = items.first() {
                            active_crate = Some(crate_name.clone());
                            active_list = DirectionalList::Required;
                            policy.required.entry(crate_name.clone()).or_default();
                        }
                        continue;
                    }

                    if line.contains("may depend on") {
                        if let Some(crate_name) = items.first() {
                            active_crate = Some(crate_name.clone());
                            active_list = DirectionalList::Allowed;
                            policy.allowed.entry(crate_name.clone()).or_default();
                        }
                        continue;
                    }

                    if (line.starts_with("- ") || line.starts_with("  -"))
                        && let Some(source) = active_crate.clone()
                    {
                        for dep in items {
                            if dep.starts_with("legion-") {
                                match active_list {
                                    DirectionalList::Allowed => {
                                        policy
                                            .allowed
                                            .entry(source.clone())
                                            .or_default()
                                            .insert(dep);
                                    }
                                    DirectionalList::Required => {
                                        policy
                                            .required
                                            .entry(source.clone())
                                            .or_default()
                                            .insert(dep);
                                    }
                                }
                            }
                        }
                    }
                }

                "contracts" if raw_line.starts_with("  -") => {
                    for item in items {
                        policy.protocol_symbols.insert(item);
                    }
                }
                _ => {}
            }
        }

        Ok(policy)
    }

    fn allowed_internal(&self, package: &str) -> Option<&HashSet<String>> {
        self.allowed.get(package)
    }

    fn forbidden_pairs(&self) -> &HashSet<(String, String)> {
        &self.forbidden
    }

    fn required_dependencies(&self) -> &HashMap<String, HashSet<String>> {
        &self.required
    }

    fn protocol_symbols(&self) -> &HashSet<String> {
        &self.protocol_symbols
    }
}

fn extract_backticked_items(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = line;

    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };

        values.push(after_start[..end].to_string());
        rest = &after_start[end + 1..];
    }

    values
}

#[test]
fn renderer_dependency_gate_preserves_projection_boundary() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest should live under workspace root");
    let policy = fs::read_to_string(workspace_root.join(DEFAULT_POLICY_PATH))
        .expect("policy should be readable");
    let package_dependencies = HashMap::from([
        (
            "legion-ui".to_string(),
            HashSet::from([
                "legion-protocol".to_string(),
                "thiserror".to_string(),
                "uuid".to_string(),
            ]),
        ),
        (
            "legion-app".to_string(),
            HashSet::from(["legion-editor".to_string(), "legion-ui".to_string()]),
        ),
        (
            "legion-desktop".to_string(),
            HashSet::from([
                "legion-app".to_string(),
                "legion-ui".to_string(),
                "egui".to_string(),
                "eframe".to_string(),
            ]),
        ),
    ]);

    let issues = validate_renderer_dependency_gate(&policy, &package_dependencies);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");

    let mut violating_dependencies = package_dependencies.clone();
    violating_dependencies
        .get_mut("legion-app")
        .expect("legion-app fixture must exist")
        .insert("eframe".to_string());
    let issues = validate_renderer_dependency_gate(&policy, &violating_dependencies);
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("legion-app") && issue.contains("eframe")),
        "core crate renderer dependency violation should be reported, got: {issues:?}"
    );

    let mut violating_dependencies = package_dependencies;
    violating_dependencies
        .get_mut("legion-ui")
        .expect("legion-ui fixture must exist")
        .insert("egui".to_string());
    let issues = validate_renderer_dependency_gate(&policy, &violating_dependencies);
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains(DEFAULT_UI_MANIFEST_PATH) && issue.contains("egui")),
        "legion-ui renderer dependency violation should be reported, got: {issues:?}"
    );
}

#[test]
fn parser_dependency_gate_keeps_tree_sitter_in_index_crate() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest should live under workspace root");
    let policy = fs::read_to_string(workspace_root.join(DEFAULT_POLICY_PATH))
        .expect("policy should be readable");
    let package_dependencies = HashMap::from([
        (
            "legion-index".to_string(),
            HashSet::from([
                "legion-protocol".to_string(),
                "legion-text".to_string(),
                "tree-sitter".to_string(),
                "tree-sitter-rust".to_string(),
            ]),
        ),
        (
            "legion-app".to_string(),
            HashSet::from(["legion-editor".to_string(), "legion-ui".to_string()]),
        ),
        (
            "legion-desktop".to_string(),
            HashSet::from(["legion-app".to_string(), "legion-ui".to_string()]),
        ),
    ]);

    let issues = validate_parser_dependency_gate(&policy, &package_dependencies);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");

    let mut violating_dependencies = package_dependencies;
    violating_dependencies
        .get_mut("legion-desktop")
        .expect("legion-desktop fixture must exist")
        .insert("tree-sitter".to_string());
    let issues = validate_parser_dependency_gate(&policy, &violating_dependencies);
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("legion-desktop") && issue.contains("tree-sitter")),
        "desktop parser dependency violation should be reported, got: {issues:?}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask manifest must live under workspace root")
            .to_path_buf()
    }

    fn read_workspace_file(relative_path: &str) -> String {
        fs::read_to_string(workspace_root().join(relative_path))
            .unwrap_or_else(|err| panic!("unable to read `{relative_path}`: {err}"))
    }

    fn source_block(text: &str, marker: &str) -> String {
        let start = text
            .find(marker)
            .unwrap_or_else(|| panic!("marker `{marker}` should exist"));
        let tail = &text[start..];
        let end = tail
            .find("\n}")
            .unwrap_or_else(|| panic!("marker `{marker}` should terminate with a block"));
        tail[..end + 2].to_string()
    }

    fn assert_phase4_runtime_surface_preserves_boundaries(relative_path: &str) {
        let source = read_workspace_file(relative_path);

        assert!(
            source.contains("legion_protocol"),
            "Phase 4 runtime crate `{relative_path}` must use protocol DTOs as its boundary"
        );
        assert!(
            source.contains("metadata") || source.contains("Metadata"),
            "Phase 4 runtime crate `{relative_path}` must keep runtime records metadata-oriented"
        );
        assert!(
            !source.contains("legion_app") && !source.contains("legion_ui"),
            "Phase 4 runtime crate `{relative_path}` must not depend on app or UI ownership"
        );
        assert!(
            !source.contains("WorkspaceActor") && !source.contains("EditorSession"),
            "Phase 4 runtime crate `{relative_path}` must not own workspace/editor mutation"
        );
    }

    fn accepted_phase3_evidence(scaffold_disclaimer: bool, checklist_checked: bool) -> String {
        let artifacts = PHASE3_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let disclaimer = if scaffold_disclaimer {
            "This document is not implementation evidence yet.\n"
        } else {
            ""
        };
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# Phase 3 evidence

## Acceptance status

- {PHASE3_ACCEPTED_MARKER}
- {LSP_ACCEPTED_MARKER}

{disclaimer}
## Expected evidence artifacts

{artifacts}
## Final validation checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_phase4_evidence(scaffold_disclaimer: bool, checklist_checked: bool) -> String {
        let artifacts = PHASE4_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let disclaimer = if scaffold_disclaimer {
            "This document is not implementation evidence yet.\n"
        } else {
            ""
        };
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# Phase 4 evidence

## Acceptance status

- {PHASE4_ACCEPTED_MARKER}

{disclaimer}
## Expected evidence artifacts

{artifacts}
## Final validation checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_phase5_evidence(checklist_checked: bool) -> String {
        let artifacts = PHASE5_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# Phase 5 evidence

## Acceptance Status

- {PHASE5_ACCEPTED_MARKER}

## Expected Evidence Artifacts

{artifacts}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_gui_phase5_evidence(checklist_checked: bool) -> String {
        let artifacts = GUI_PHASE5_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let commands = GUI_PHASE5_REQUIRED_COMMAND_MARKERS
            .iter()
            .map(|command| format!("- `{command}`\n"))
            .collect::<String>();
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# GUI Phase 5 evidence

## Acceptance Status

- {PHASE5_ACCEPTED_MARKER}

## Required Artifacts

{artifacts}
## Required Commands

{commands}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_gui_phase6_evidence(checklist_checked: bool) -> String {
        let artifacts = GUI_PHASE6_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let commands = GUI_PHASE6_REQUIRED_COMMAND_MARKERS
            .iter()
            .map(|command| format!("- `{command}`\n"))
            .collect::<String>();
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# GUI Phase 6 evidence

## Acceptance Status

- {PHASE6_ACCEPTED_MARKER}

## Required Artifacts

{artifacts}
## Required Commands

{commands}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_gui_phase7_evidence(checklist_checked: bool) -> String {
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
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# GUI Phase 7 local IDE beta evidence

## Acceptance Status

- {PHASE7_ACCEPTED_MARKER}

## Required Artifacts

{artifacts}
## Required Commands

{commands}
## Known Limitations Required For Acceptance

{limitations}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_gui_phase8_evidence(checklist_checked: bool) -> String {
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
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# GUI Phase 8 advanced platform GUI GA evidence

## Acceptance Status

- {PHASE8_ACCEPTED_MARKER}

## Required Artifacts

{artifacts}
## Required Commands

{commands}
## Supported Advanced GUI Surface Markers

{surfaces}
## Platform Parity Markers

{platforms}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_phase13_evidence(checklist_checked: bool) -> String {
        let artifacts = PHASE13_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let commands = PHASE13_REQUIRED_COMMAND_MARKERS
            .iter()
            .map(|command| format!("- `{command}`\n"))
            .collect::<String>();
        let markers = PHASE13_REQUIRED_EVIDENCE_MARKERS
            .iter()
            .map(|marker| format!("- {marker}\n"))
            .collect::<String>();
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# Phase 13 Legion Workflow Orchestration evidence

## Acceptance Status

- {PHASE13_ACCEPTED_MARKER}

## Required Artifacts

{artifacts}
## Required Commands

{commands}
## Required Markers

{markers}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_phase13_final_gates() -> String {
        let commands = PHASE13_REQUIRED_COMMAND_MARKERS
            .iter()
            .map(|command| format!("- `{command}`: passed\n"))
            .collect::<String>();

        format!(
            r#"# Phase 13 final gates

Final gate outputs archived from current commands

## Required Commands

{commands}"#
        )
    }

    fn accepted_phase13_runbook() -> String {
        PHASE13_REQUIRED_RUNBOOK_MARKERS
            .iter()
            .map(|marker| format!("- {marker}\n"))
            .collect::<String>()
    }

    fn accepted_phase6_evidence(scaffold_disclaimer: bool, checklist_checked: bool) -> String {
        let artifacts = PHASE6_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let disclaimer = if scaffold_disclaimer {
            "This document is Phase 6 scaffold evidence, not acceptance evidence yet.\n"
        } else {
            ""
        };
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# Phase 6 collaboration evidence

## Acceptance Status

- {PHASE6_ACCEPTED_MARKER}

{disclaimer}
## Expected Evidence Artifacts

{artifacts}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_phase7_evidence(scaffold_disclaimer: bool, checklist_checked: bool) -> String {
        let artifacts = PHASE7_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let disclaimer = if scaffold_disclaimer {
            "This document is Phase 7 scaffold evidence, not acceptance evidence yet.\n"
        } else {
            ""
        };
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# Phase 7 remote evidence

## Acceptance Status

- {PHASE7_ACCEPTED_MARKER}

{disclaimer}
## Expected Evidence Artifacts

{artifacts}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_phase8_evidence(scaffold_disclaimer: bool, checklist_checked: bool) -> String {
        let artifacts = PHASE8_REQUIRED_ARTIFACTS
            .iter()
            .map(|artifact| format!("- `{artifact}`\n"))
            .collect::<String>();
        let disclaimer = if scaffold_disclaimer {
            "This document is Phase 8 scaffold evidence, not acceptance evidence yet.\n"
        } else {
            ""
        };
        let checklist_marker = if checklist_checked { "x" } else { " " };

        format!(
            r#"# Phase 8 evidence

## Acceptance Status

- {PHASE8_ACCEPTED_MARKER}

{disclaimer}
Runtime surface status: Production GA runtime surfaces are active behind accepted policy gates.

Platform matrix: Linux, Windows, and macOS validated.

Release readiness: Security, privacy, operations, rollback, canary, incident, and supply-chain signoff complete.

Final gate outputs archived from current commands.

## Expected Evidence Artifacts

{artifacts}
## Final Validation Checklist

- [{checklist_marker}] Required validation is complete.
"#
        )
    }

    fn accepted_phase8_artifact_text(artifact: &str) -> Option<String> {
        match artifact {
            PHASE8_PLATFORM_MATRIX_ARTIFACT => Some(
                [
                    "Workflow: .github/workflows/ci.yml",
                    "Run URL: https://github.example/legion-ide/actions/runs/1",
                    "ubuntu-latest: passed",
                    "windows-latest: passed",
                    "macos-latest: passed",
                    "cargo fmt --all --check: passed",
                    "cargo check --workspace --all-targets: passed",
                    "cargo test --workspace --all-targets: passed",
                    "cargo clippy --workspace --all-targets -- -D warnings: passed",
                    "cargo deny check: passed",
                    "cargo run -p legion-cli -- evidence check --phase phase8: passed",
                    "cargo run -p xtask -- check-deps: passed",
                ]
                .join("\n"),
            ),
            PHASE8_RELEASE_READINESS_ARTIFACT => Some(
                [
                    "Signoff date: 2026-05-26",
                    "Security signoff: Complete.",
                    "Privacy signoff: Complete.",
                    "Operations signoff: Complete.",
                    "Rollback signoff: Complete.",
                    "Canary signoff: Complete.",
                    "Incident response signoff: Complete.",
                    "Supply-chain signoff: Complete.",
                ]
                .join("\n"),
            ),
            _ => Some("Result: passed".to_string()),
        }
    }

    #[test]
    fn missing_workspace_crate_policy_is_reported() {
        let packages = HashMap::from([(
            "legion-ui".to_string(),
            HashSet::from(["legion-protocol".to_string()]),
        )]);

        let issues = validate_dependency_policy(&packages, &Policy::default());

        assert!(issues.iter().any(|issue| {
            issue.contains(
                "`legion-ui` lacks dependency policy coverage in `plans/dependency-policy.md`",
            )
        }));
    }

    #[test]
    fn policy_parses_required_dependencies_from_markdown() {
        let markdown = r#"
### 1. Directional Intent
- `legion-ui` may depend on:
  - `legion-protocol`
- `legion-ui` MUST directly depend on:
  - `legion-protocol`
- `legion-ui` MUST NOT depend on `legion-editor`.
- `legion-ui` MUST NOT depend on `legion-project`.

### 2. Shared Contracts Boundary
  - `WorkspaceId`
"#;

        let policy = Policy::from_markdown(markdown).expect("policy should parse");

        assert_eq!(
            policy.allowed_internal("legion-ui"),
            Some(&HashSet::from(["legion-protocol".to_string()]))
        );
        assert_eq!(
            policy.required_dependencies().get("legion-ui"),
            Some(&HashSet::from(["legion-protocol".to_string()]))
        );
        assert!(
            policy
                .forbidden_pairs()
                .contains(&("legion-ui".to_string(), "legion-editor".to_string()))
        );
        assert!(
            policy
                .forbidden_pairs()
                .contains(&("legion-ui".to_string(), "legion-project".to_string()))
        );
        assert!(policy.protocol_symbols().contains("WorkspaceId"));
    }

    #[test]
    fn phase3_acceptance_status_section_is_required() {
        let issues = validate_phase3_acceptance_governance(
            "# Phase 3 evidence\n\n## Final validation checklist\n\n- [ ] pending\n",
            |_| false,
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.contains(PHASE3_STATUS_HEADING))
        );
    }

    #[test]
    fn phase3_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# Phase 3 evidence

## Acceptance status

- {PHASE3_PARTIAL_RUNTIME_MARKER}
- {PHASE3_NOT_ACCEPTED_MARKER}
- {LSP_NOT_ACCEPTED_MARKER}

## Final validation checklist

- [ ] pending
"#
        );

        let issues = validate_phase3_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase3_acceptance_claim_requires_artifacts_and_checked_checklist() {
        let evidence = accepted_phase3_evidence(true, false);
        let issues = validate_phase3_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while still saying it is not implementation evidence yet"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains("required artifact `lsp-supervision-tests.txt` is missing")
        }));
    }

    #[test]
    fn phase3_acceptance_claim_passes_with_checked_checklist_and_artifacts() {
        let evidence = accepted_phase3_evidence(false, true);
        let issues = validate_phase3_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase3_evidence_declares_accepted_status_with_artifacts() {
        let source = read_workspace_file(DEFAULT_PHASE3_EVIDENCE_PATH);
        let evidence_path = workspace_root().join(DEFAULT_PHASE3_EVIDENCE_PATH);
        let evidence_dir = evidence_path
            .parent()
            .expect("Phase 3 evidence path should have a parent directory");
        let issues = validate_phase3_acceptance_governance(&source, |artifact| {
            evidence_dir.join(artifact).is_file()
        });

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert!(source.contains(PHASE3_ACCEPTED_MARKER));
        assert!(source.contains(LSP_ACCEPTED_MARKER));
        assert!(!source.contains("This document is not implementation evidence yet"));
        for artifact in PHASE3_REQUIRED_ARTIFACTS {
            assert!(
                source.contains(artifact),
                "Phase 3 evidence must list required artifact `{artifact}`"
            );
        }
    }

    #[test]
    fn phase4_acceptance_status_section_is_required() {
        let issues = validate_phase4_acceptance_governance(
            "# Phase 4 evidence\n\n## Final validation checklist\n\n- [ ] pending\n",
            |_| false,
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.contains(PHASE4_STATUS_HEADING))
        );
    }

    #[test]
    fn phase4_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# Phase 4 evidence

## Acceptance status

- {PHASE4_NOT_ACCEPTED_MARKER}

## Final validation checklist

- [ ] pending
"#
        );

        let issues = validate_phase4_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase4_acceptance_claim_requires_artifacts_and_checked_checklist() {
        let evidence = accepted_phase4_evidence(true, false);
        let issues = validate_phase4_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while still saying it is not implementation evidence yet"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains("required artifact `provider-router-contract-tests.txt` is missing")
        }));
    }

    #[test]
    fn phase4_acceptance_claim_passes_with_checked_checklist_and_artifacts() {
        let evidence = accepted_phase4_evidence(false, true);
        let issues = validate_phase4_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase4_evidence_declares_accepted_status_with_artifacts() {
        let source = read_workspace_file(DEFAULT_PHASE4_EVIDENCE_PATH);
        let evidence_path = workspace_root().join(DEFAULT_PHASE4_EVIDENCE_PATH);
        let evidence_dir = evidence_path
            .parent()
            .expect("Phase 4 evidence path should have a parent directory");
        let issues = validate_phase4_acceptance_governance(&source, |artifact| {
            evidence_dir.join(artifact).is_file()
        });

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert!(source.contains(PHASE4_ACCEPTED_MARKER));
        assert!(!source.contains("This document is not implementation evidence yet"));
        for artifact in PHASE4_REQUIRED_ARTIFACTS {
            assert!(
                source.contains(artifact),
                "Phase 4 evidence must list required artifact `{artifact}`"
            );
        }
    }

    #[test]
    fn phase5_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# Phase 5 evidence

## Acceptance Status

- {PHASE5_NOT_ACCEPTED_MARKER}

## Final Validation Checklist

- [ ] pending
"#
        );

        let issues = validate_phase5_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase5_acceptance_claim_requires_artifacts_and_checked_checklist() {
        let evidence = accepted_phase5_evidence(false);
        let issues = validate_phase5_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains("required artifact `host-call-capability-tests.txt` is missing")
        }));
    }

    #[test]
    fn phase5_acceptance_claim_passes_with_checked_checklist_and_artifacts() {
        let evidence = accepted_phase5_evidence(true);
        let issues = validate_phase5_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase5_evidence_declares_accepted_status_with_artifacts() {
        let source = read_workspace_file(DEFAULT_PHASE5_EVIDENCE_PATH);
        let evidence_path = workspace_root().join(DEFAULT_PHASE5_EVIDENCE_PATH);
        let evidence_dir = evidence_path
            .parent()
            .expect("Phase 5 evidence path should have a parent directory");
        let issues = validate_phase5_acceptance_governance(&source, |artifact| {
            evidence_dir.join(artifact).is_file()
        });

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert!(source.contains(PHASE5_ACCEPTED_MARKER));
        for artifact in PHASE5_REQUIRED_ARTIFACTS {
            assert!(
                source.contains(artifact),
                "Phase 5 evidence must list required artifact `{artifact}`"
            );
        }
    }

    #[test]
    fn gui_phase5_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# GUI Phase 5 control and trust evidence

## Acceptance Status

- {PHASE5_NOT_ACCEPTED_MARKER}

## Final Validation Checklist

- [ ] pending
"#
        );

        let issues = validate_gui_phase5_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase5_acceptance_claim_requires_artifacts_commands_and_checked_checklist() {
        let evidence = accepted_gui_phase5_evidence(false);
        let issues = validate_gui_phase5_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "required artifact `plans/evidence/gui-productization/phase-5-control-trust-safety.md` is missing"
        )));
    }

    #[test]
    fn gui_phase5_acceptance_claim_passes_with_checked_checklist_artifacts_and_commands() {
        let evidence = accepted_gui_phase5_evidence(true);
        let issues = validate_gui_phase5_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase6_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# GUI Phase 6 packaging platform accessibility evidence

## Acceptance Status

- {PHASE6_NOT_ACCEPTED_MARKER}

## Final Validation Checklist

- [ ] pending
"#
        );

        let issues = validate_gui_phase6_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase6_acceptance_claim_requires_artifacts_commands_and_checked_checklist() {
        let evidence = accepted_gui_phase6_evidence(false).replace(
            "- `cargo run -p legion-cli -- evidence check --phase gui-phase6`\n",
            "",
        );
        let issues = validate_gui_phase6_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "required artifact `plans/evidence/gui-productization/phase-6-package-runbook.md` is missing"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "required command `cargo run -p legion-cli -- evidence check --phase gui-phase6` is not listed"
        )));
    }

    #[test]
    fn gui_phase6_acceptance_claim_passes_with_checked_checklist_artifacts_and_commands() {
        let evidence = accepted_gui_phase6_evidence(true);
        let issues = validate_gui_phase6_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase7_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# GUI Phase 7 local IDE beta evidence

## Acceptance Status

- {PHASE7_NOT_ACCEPTED_MARKER}

## Final Validation Checklist

- [ ] pending
"#
        );

        let issues = validate_gui_phase7_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase7_acceptance_status_rejects_conflicting_markers() {
        let evidence = format!(
            r#"# GUI Phase 7 local IDE beta evidence

## Acceptance Status

- {PHASE7_NOT_ACCEPTED_MARKER}
- {PHASE7_ACCEPTED_MARKER}
"#
        );

        let issues = validate_gui_phase7_acceptance_governance(&evidence, |_| true);

        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("must not declare both"))
        );
    }

    #[test]
    fn gui_phase7_acceptance_claim_requires_artifacts_commands_limits_and_checked_checklist() {
        let evidence = accepted_gui_phase7_evidence(false)
            .replace(
                "- `cargo run -p legion-cli -- evidence check --phase gui-phase7`\n",
                "",
            )
            .replace("- Autonomous apply: unsupported\n", "");
        let issues = validate_gui_phase7_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "required artifact `plans/evidence/gui-productization/phase-7-local-workflow-smoke.md` is missing"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "required command `cargo run -p legion-cli -- evidence check --phase gui-phase7` is not listed"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "required limitation marker `Autonomous apply: unsupported` is not listed",
            )
        }));
    }

    #[test]
    fn gui_phase7_acceptance_claim_passes_with_checked_checklist_artifacts_commands_and_limits() {
        let evidence = accepted_gui_phase7_evidence(true);
        let issues = validate_gui_phase7_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase8_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# GUI Phase 8 advanced platform GUI GA evidence

## Acceptance Status

- {PHASE8_NOT_ACCEPTED_MARKER}

## Final Validation Checklist

- [ ] pending
"#
        );

        let issues = validate_gui_phase8_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn gui_phase8_acceptance_status_rejects_conflicting_markers() {
        let evidence = format!(
            r#"# GUI Phase 8 advanced platform GUI GA evidence

## Acceptance Status

- {PHASE8_NOT_ACCEPTED_MARKER}
- {PHASE8_ACCEPTED_MARKER}
"#
        );

        let issues = validate_gui_phase8_acceptance_governance(&evidence, |_| true);

        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("must not declare both"))
        );
    }

    #[test]
    fn gui_phase8_acceptance_claim_requires_artifacts_commands_markers_and_checked_checklist() {
        let evidence = accepted_gui_phase8_evidence(false)
            .replace(
                "- `cargo run -p legion-cli -- evidence check --phase gui-phase8`\n",
                "",
            )
            .replace("- Plugin management GUI: supported\n", "")
            .replace("- Platform parity: macOS\n", "");
        let issues = validate_gui_phase8_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "required artifact `plans/evidence/gui-productization/phase-8-plugin-management.md` is missing"
        )));
        assert!(issues.iter().any(|issue| issue.contains(
            "required command `cargo run -p legion-cli -- evidence check --phase gui-phase8` is not listed"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "required supported surface marker `Plugin management GUI: supported` is not listed",
            )
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("required platform marker `Platform parity: macOS` is not listed")
        }));
    }

    #[test]
    fn gui_phase8_acceptance_claim_rejects_phase7_unsupported_labels() {
        let mut evidence = accepted_gui_phase8_evidence(true);
        evidence.push_str("\nPlugin management GUI: unsupported\n");

        let issues = validate_gui_phase8_acceptance_governance(&evidence, |_| true);

        assert!(issues.iter().any(|issue| {
            issue.contains("stale unsupported marker `Plugin management GUI: unsupported`")
        }));
    }

    #[test]
    fn gui_phase8_acceptance_claim_passes_with_checked_checklist_artifacts_commands_and_markers() {
        let evidence = accepted_gui_phase8_evidence(true);
        let issues = validate_gui_phase8_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase13_acceptance_claim_requires_artifacts_markers_commands_and_runbook() {
        let evidence = accepted_phase13_evidence(false)
            .replace("- Legion workflow orchestration: approval-gated\n", "")
            .replace("- `cargo test --workspace --all-targets`\n", "");
        let final_gates = accepted_phase13_final_gates()
            .replace("- `cargo check --workspace --all-targets`: passed\n", "");
        let runbook = accepted_phase13_runbook()
            .replace("- Autonomous merge: unsupported until approval\n", "");
        let issues =
            validate_phase13_acceptance_evidence(&evidence, &final_gates, &runbook, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains("required artifact `.planning/phases/13-legion-workflow-orchestration/13-01-RESULT.md`")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("required marker `Legion workflow orchestration: approval-gated`")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("required command `cargo test --workspace --all-targets`")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("`plans/evidence/gui-productization/phase-13-final-gates.md` is missing required command `cargo check --workspace --all-targets`")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("required runbook marker `Autonomous merge: unsupported until approval`")
        }));
    }

    #[test]
    fn phase13_acceptance_claim_rejects_stale_pending_markers() {
        let mut evidence = accepted_phase13_evidence(true);
        evidence.push_str("\nacceptance pending final gates\n");

        let issues = validate_phase13_acceptance_evidence(
            &evidence,
            &accepted_phase13_final_gates(),
            &accepted_phase13_runbook(),
            |_| true,
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("stale marker `acceptance pending`"))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("stale marker `pending final gates`"))
        );
    }

    #[test]
    fn phase13_acceptance_claim_passes_with_required_evidence() {
        let evidence = accepted_phase13_evidence(true);
        let issues = validate_phase13_acceptance_evidence(
            &evidence,
            &accepted_phase13_final_gates(),
            &accepted_phase13_runbook(),
            |_| true,
        );

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase13_evidence_declares_accepted_final_gate_artifacts() {
        let source = read_workspace_file(DEFAULT_PHASE13_EVIDENCE_PATH);
        let final_gates = read_workspace_file(DEFAULT_PHASE13_FINAL_GATES_PATH);
        let runbook = read_workspace_file(DEFAULT_PHASE13_RUNBOOK_PATH);
        let root = workspace_root();
        let issues =
            validate_phase13_acceptance_evidence(&source, &final_gates, &runbook, |artifact| {
                root.join(artifact).is_file()
            });

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert!(source.contains(PHASE13_ACCEPTED_MARKER));
    }

    #[test]
    fn phase6_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# Phase 6 collaboration evidence

## Acceptance Status

- {PHASE6_NOT_ACCEPTED_MARKER}

## Final Validation Checklist

- [ ] pending
"#
        );

        let issues = validate_phase6_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase6_acceptance_status_rejects_conflicting_markers() {
        let evidence = format!(
            r#"# Phase 6 collaboration evidence

## Acceptance Status

- {PHASE6_NOT_ACCEPTED_MARKER}
- {PHASE6_ACCEPTED_MARKER}
"#
        );

        let issues = validate_phase6_acceptance_governance(&evidence, |_| true);

        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("must not declare both"))
        );
    }

    #[test]
    fn phase6_acceptance_claim_requires_artifacts_and_checked_checklist() {
        let evidence = accepted_phase6_evidence(true, false);
        let issues = validate_phase6_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains("claims acceptance while still saying it is scaffold evidence")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("required artifact `collaboration-convergence-tests.txt` is missing")
        }));
    }

    #[test]
    fn phase6_acceptance_claim_passes_with_checked_checklist_and_artifacts() {
        let evidence = accepted_phase6_evidence(false, true);
        let issues = validate_phase6_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase6_evidence_declares_accepted_status_with_artifacts() {
        let source = read_workspace_file(DEFAULT_PHASE6_EVIDENCE_PATH);
        let evidence_path = workspace_root().join(DEFAULT_PHASE6_EVIDENCE_PATH);
        let evidence_dir = evidence_path
            .parent()
            .expect("Phase 6 evidence path should have a parent directory");
        let issues = validate_phase6_acceptance_governance(&source, |artifact| {
            evidence_dir.join(artifact).is_file()
        });

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert!(source.contains(PHASE6_ACCEPTED_MARKER));
        for artifact in PHASE6_REQUIRED_ARTIFACTS {
            assert!(
                source.contains(artifact),
                "Phase 6 evidence must list required artifact `{artifact}`"
            );
        }
    }

    #[test]
    fn phase7_not_accepted_status_allows_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# Phase 7 remote evidence

## Acceptance Status

- {PHASE7_NOT_ACCEPTED_MARKER}

## Final Validation Checklist

- [ ] pending
"#
        );

        let issues = validate_phase7_acceptance_governance(&evidence, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase7_acceptance_status_rejects_conflicting_markers() {
        let evidence = format!(
            r#"# Phase 7 remote evidence

## Acceptance Status

- {PHASE7_NOT_ACCEPTED_MARKER}
- {PHASE7_ACCEPTED_MARKER}
"#
        );

        let issues = validate_phase7_acceptance_governance(&evidence, |_| true);

        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("must not declare both"))
        );
    }

    #[test]
    fn phase7_acceptance_claim_requires_artifacts_and_checked_checklist() {
        let evidence = accepted_phase7_evidence(true, false);
        let issues = validate_phase7_acceptance_governance(&evidence, |_| false);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains("claims acceptance while still saying it is scaffold evidence")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("required artifact `remote-agent-lifecycle-tests.txt` is missing")
        }));
    }

    #[test]
    fn phase7_acceptance_claim_passes_with_checked_checklist_and_artifacts() {
        let evidence = accepted_phase7_evidence(false, true);
        let issues = validate_phase7_acceptance_governance(&evidence, |_| true);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase7_evidence_declares_accepted_status_with_artifacts() {
        let source = read_workspace_file(DEFAULT_PHASE7_EVIDENCE_PATH);
        let evidence_path = workspace_root().join(DEFAULT_PHASE7_EVIDENCE_PATH);
        let evidence_dir = evidence_path
            .parent()
            .expect("Phase 7 evidence path should have a parent directory");
        let issues = validate_phase7_acceptance_governance(&source, |artifact| {
            evidence_dir.join(artifact).is_file()
        });

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert!(source.contains(PHASE7_ACCEPTED_MARKER));
        for artifact in PHASE7_REQUIRED_ARTIFACTS {
            assert!(
                source.contains(artifact),
                "Phase 7 evidence must list required artifact `{artifact}`"
            );
        }
    }

    #[test]
    fn phase8_not_accepted_status_allows_fixture_scaffold_without_artifacts() {
        let evidence = format!(
            r#"# Phase 8 fixture evidence

## Acceptance Status

- {PHASE8_NOT_ACCEPTED_MARKER}

This document is Phase 8 scaffold evidence, not acceptance evidence yet.

## Final Validation Checklist

- [ ] pending
"#
        );

        let issues = validate_phase8_acceptance_governance(&evidence, |_| false, |_| None);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase8_acceptance_claim_requires_artifacts_and_checked_checklist() {
        let evidence = accepted_phase8_evidence(true, false);
        let issues = validate_phase8_acceptance_governance(&evidence, |_| false, |_| None);

        assert!(issues.iter().any(|issue| issue.contains(
            "claims acceptance while final validation checklist items remain unchecked"
        )));
        assert!(issues.iter().any(|issue| {
            issue.contains("claims acceptance while still saying it is scaffold evidence")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains(
                "required artifact `remote-production-transport-security-tests.txt` is missing",
            )
        }));
    }

    #[test]
    fn phase8_acceptance_claim_passes_with_checked_checklist_and_artifacts() {
        let evidence = accepted_phase8_evidence(false, true);
        let issues = validate_phase8_acceptance_governance(
            &evidence,
            |_| true,
            accepted_phase8_artifact_text,
        );

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn phase8_acceptance_claim_rejects_stale_deferred_markers() {
        let mut evidence = accepted_phase8_evidence(false, true);
        evidence.push_str(
            "production transport, native terminal, hosted export, raw-source vault, and operational GA remain deferred\n",
        );

        let issues = validate_phase8_acceptance_governance(
            &evidence,
            |_| true,
            accepted_phase8_artifact_text,
        );

        assert!(issues.iter().any(|issue| {
            issue.contains("claims acceptance while still containing stale deferred marker")
        }));
    }

    #[test]
    fn phase8_acceptance_claim_requires_final_ga_markers() {
        let evidence = accepted_phase8_evidence(false, true).replace(
            "Platform matrix: Linux, Windows, and macOS validated.",
            "Platform matrix: pending.",
        );

        let issues = validate_phase8_acceptance_governance(
            &evidence,
            |_| true,
            accepted_phase8_artifact_text,
        );

        assert!(issues.iter().any(|issue| {
            issue.contains("final GA marker `Platform matrix: Linux, Windows, and macOS validated.` is missing")
        }));
    }

    #[test]
    fn phase8_acceptance_claim_requires_matrix_artifact_contents() {
        let evidence = accepted_phase8_evidence(false, true);
        let issues = validate_phase8_acceptance_governance(
            &evidence,
            |_| true,
            |artifact| {
                if artifact == PHASE8_PLATFORM_MATRIX_ARTIFACT {
                    Some("Workflow: .github/workflows/ci.yml\nwindows-latest: pending".to_string())
                } else {
                    accepted_phase8_artifact_text(artifact)
                }
            },
        );

        assert!(issues.iter().any(|issue| {
            issue.contains("`platform-matrix-evidence.txt`")
                && issue.contains("ubuntu-latest: passed")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("`platform-matrix-evidence.txt`")
                && issue.contains("stale marker `pending`")
        }));
    }

    #[test]
    fn phase8_artifact_stale_marker_matching_rejects_pending_token_not_substrings() {
        assert!(contains_phase8_accepted_artifact_stale_marker(
            "windows-latest: pending.",
            "pending",
        ));
        assert!(contains_phase8_accepted_artifact_stale_marker(
            "Status: Pending signoff.",
            "pending",
        ));
        assert!(!contains_phase8_accepted_artifact_stale_marker(
            "Status: accepted depending on archived CI evidence.",
            "pending",
        ));
    }

    #[test]
    fn phase8_acceptance_claim_requires_release_signoff_artifact_contents() {
        let evidence = accepted_phase8_evidence(false, true);
        let issues = validate_phase8_acceptance_governance(
            &evidence,
            |_| true,
            |artifact| {
                if artifact == PHASE8_RELEASE_READINESS_ARTIFACT {
                    Some(
                        "Status: final GA signoff still pending platform matrix evidence"
                            .to_string(),
                    )
                } else {
                    accepted_phase8_artifact_text(artifact)
                }
            },
        );

        assert!(issues.iter().any(|issue| {
            issue.contains("`release-readiness-review.md`")
                && issue.contains("Security signoff: Complete.")
        }));
        assert!(issues.iter().any(|issue| {
            issue.contains("`release-readiness-review.md`")
                && issue.contains("final GA signoff still pending")
        }));
    }

    #[test]
    fn phase8_evidence_declares_accepted_ga_status() {
        let source = read_workspace_file(DEFAULT_PHASE8_EVIDENCE_PATH);
        let evidence_path = workspace_root().join(DEFAULT_PHASE8_EVIDENCE_PATH);
        let evidence_dir = evidence_path
            .parent()
            .expect("Phase 8 evidence path should have a parent directory");
        let issues = validate_phase8_acceptance_governance(
            &source,
            |artifact| evidence_dir.join(artifact).is_file(),
            |artifact| fs::read_to_string(evidence_dir.join(artifact)).ok(),
        );

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert!(source.contains(PHASE8_ACCEPTED_MARKER));
        for marker in PHASE8_ACCEPTED_REQUIRED_MARKERS {
            assert!(
                source.contains(marker),
                "Phase 8 evidence must contain accepted marker `{marker}`"
            );
        }
        for artifact in PHASE8_REQUIRED_ARTIFACTS {
            assert!(
                source.contains(artifact),
                "Phase 8 evidence must list required artifact `{artifact}`"
            );
        }
    }

    #[test]
    fn ui_shell_remains_projection_only() {
        let source = read_workspace_file("crates/legion-ui/src/ui.rs");
        let manifest = read_workspace_file("crates/legion-ui/Cargo.toml");

        assert!(source.contains("pub struct Shell"));
        assert!(source.contains("CommandDispatchIntent"));
        assert!(source.contains("without mutating editor or workspace state"));
        assert!(!source.contains("EditorSession"));
        assert!(!source.contains("WorkspaceActor"));
        assert!(!source.contains("EditorEngine"));
        assert!(!source.contains("SaveWorkflowService"));
        assert!(!manifest.contains("legion-editor"));
        assert!(!manifest.contains("legion-project"));
        assert!(!manifest.contains("legion-storage"));
        assert!(!manifest.contains("legion-app"));
    }

    #[test]
    fn save_active_buffer_remains_proposal_mediated() {
        let source = read_workspace_file("crates/legion-app/src/lib.rs");

        assert!(source.contains("struct SaveWorkflowService;"));
        assert!(source.contains("SaveWorkflowService::save_active_buffer("));
        assert!(source.contains("workspace.save_file_with_proposal(workspace_save)"));
        assert!(source.contains("AppSaveOutcome::Rejected"));
    }

    #[test]
    fn source_snapshots_are_not_persisted_by_default() {
        let source = read_workspace_file("crates/legion-storage/src/lib.rs");
        let session_record = source_block(&source, "pub struct SessionRecord");
        let persisted_state = source_block(&source, "struct PersistedState");

        assert!(!session_record.contains("SnapshotId"));
        assert!(!session_record.contains("text:"));
        assert!(!persisted_state.contains("SnapshotId"));
        assert!(!persisted_state.contains("text:"));
    }

    #[test]
    fn phase4_runtime_surfaces_remain_protocol_mediated() {
        for path in [
            "crates/legion-agent/src/lib.rs",
            "crates/legion-tracker/src/lib.rs",
            "crates/legion-memory/src/lib.rs",
        ] {
            assert_phase4_runtime_surface_preserves_boundaries(path);
        }
    }
}
