use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::Path,
    process,
};

use cargo_metadata::{Metadata, MetadataCommand};
use clap::{Parser, Subcommand};

const DEFAULT_POLICY_PATH: &str = "plans/dependency-policy.md";
const DEFAULT_PROTOCOL_PATH: &str = "crates/devil-protocol/src/lib.rs";
const DEFAULT_UI_MANIFEST_PATH: &str = "crates/devil-ui/Cargo.toml";
const DEFAULT_PHASE3_EVIDENCE_PATH: &str = "plans/evidence/phase-3/predictive-semantic-fabric.md";
const DEFAULT_PHASE4_EVIDENCE_PATH: &str = "plans/evidence/phase-4/agentic-ai-architecture-map.md";
const DEFAULT_PHASE5_EVIDENCE_PATH: &str = "plans/evidence/phase-5/plugin-architecture-map.md";
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
const PHASE3_PARTIAL_RUNTIME_MARKER: &str = "Runtime surface status: Partial `devil-index` indexing behavior is active; acceptance evidence is incomplete.";
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
    "`devil-desktop` may depend on:",
    "`eframe`",
    "`egui`",
    "renderer dependencies",
    "adapter-only",
];
const DEVIL_UI_FORBIDDEN_RENDERER_DEPS: &[&str] = &[
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
    };

    process::exit(code);
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
    let violations = validate_dependency_policy(&packages, &policy);
    let renderer_violations = validate_renderer_dependency_gate(
        &policy_text,
        &package_dependency_names(&metadata, "devil-ui"),
    );

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
    all.extend(protocol_violations);
    all.extend(phase3_violations);
    all.extend(phase4_violations);
    all.extend(phase5_violations);
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

fn package_dependency_names(metadata: &Metadata, package_name: &str) -> HashSet<String> {
    metadata
        .packages
        .iter()
        .find(|package| package.name == package_name)
        .map(|package| {
            package
                .dependencies
                .iter()
                .map(|dependency| dependency.name.clone())
                .collect()
        })
        .unwrap_or_default()
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
    devil_ui_dependencies: &HashSet<String>,
) -> Vec<String> {
    let mut issues = Vec::new();

    for marker in RENDERER_BOUNDARY_POLICY_MARKERS {
        if !policy_text.contains(marker) {
            issues.push(format!(
                "`plans/dependency-policy.md` must document renderer boundary marker `{marker}`"
            ));
        }
    }

    let mut forbidden_declared = DEVIL_UI_FORBIDDEN_RENDERER_DEPS
        .iter()
        .filter(|dependency| devil_ui_dependencies.contains(**dependency))
        .copied()
        .collect::<Vec<_>>();
    forbidden_declared.sort();

    if !forbidden_declared.is_empty() {
        issues.push(format!(
            "`{DEFAULT_UI_MANIFEST_PATH}` must not declare renderer/windowing dependencies: {}",
            forbidden_declared.join(", ")
        ));
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
        .map(|symbol| format!("protocol contract symbol `{symbol}` missing from `crates/devil-protocol/src/lib.rs`"))
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
                if !matrix.contains(marker) {
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
                if !release.contains(marker) {
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
                            if dep.starts_with("devil-") {
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
    let devil_ui_dependencies = HashSet::from([
        "devil-protocol".to_string(),
        "thiserror".to_string(),
        "uuid".to_string(),
    ]);

    let issues = validate_renderer_dependency_gate(&policy, &devil_ui_dependencies);
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");

    let mut violating_dependencies = devil_ui_dependencies;
    violating_dependencies.insert("eframe".to_string());
    let issues = validate_renderer_dependency_gate(&policy, &violating_dependencies);
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains(DEFAULT_UI_MANIFEST_PATH) && issue.contains("eframe")),
        "renderer dependency violation should be reported, got: {issues:?}"
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
            source.contains("devil_protocol"),
            "Phase 4 runtime crate `{relative_path}` must use protocol DTOs as its boundary"
        );
        assert!(
            source.contains("metadata") || source.contains("Metadata"),
            "Phase 4 runtime crate `{relative_path}` must keep runtime records metadata-oriented"
        );
        assert!(
            !source.contains("devil_app") && !source.contains("devil_ui"),
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
                    "Run URL: https://github.example/devil-ide/actions/runs/1",
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
            "devil-ui".to_string(),
            HashSet::from(["devil-protocol".to_string()]),
        )]);

        let issues = validate_dependency_policy(&packages, &Policy::default());

        assert!(issues.iter().any(|issue| {
            issue.contains(
                "`devil-ui` lacks dependency policy coverage in `plans/dependency-policy.md`",
            )
        }));
    }

    #[test]
    fn policy_parses_required_dependencies_from_markdown() {
        let markdown = r#"
### 1. Directional Intent
- `devil-ui` may depend on:
  - `devil-protocol`
- `devil-ui` MUST directly depend on:
  - `devil-protocol`
- `devil-ui` MUST NOT depend on `devil-editor`.
- `devil-ui` MUST NOT depend on `devil-project`.

### 2. Shared Contracts Boundary
  - `WorkspaceId`
"#;

        let policy = Policy::from_markdown(markdown).expect("policy should parse");

        assert_eq!(
            policy.allowed_internal("devil-ui"),
            Some(&HashSet::from(["devil-protocol".to_string()]))
        );
        assert_eq!(
            policy.required_dependencies().get("devil-ui"),
            Some(&HashSet::from(["devil-protocol".to_string()]))
        );
        assert!(
            policy
                .forbidden_pairs()
                .contains(&("devil-ui".to_string(), "devil-editor".to_string()))
        );
        assert!(
            policy
                .forbidden_pairs()
                .contains(&("devil-ui".to_string(), "devil-project".to_string()))
        );
        assert!(policy.protocol_symbols().contains("WorkspaceId"));
    }

    #[test]
    fn renderer_dependency_gate_preserves_projection_boundary() {
        let policy = read_workspace_file(DEFAULT_POLICY_PATH);
        let devil_ui_dependencies = HashSet::from([
            "devil-protocol".to_string(),
            "thiserror".to_string(),
            "uuid".to_string(),
        ]);

        let issues = validate_renderer_dependency_gate(&policy, &devil_ui_dependencies);
        assert!(issues.is_empty(), "unexpected issues: {issues:?}");

        let mut violating_dependencies = devil_ui_dependencies;
        violating_dependencies.insert("eframe".to_string());
        let issues = validate_renderer_dependency_gate(&policy, &violating_dependencies);
        assert!(
            issues.iter().any(|issue| {
                issue.contains(DEFAULT_UI_MANIFEST_PATH) && issue.contains("eframe")
            }),
            "renderer dependency violation should be reported, got: {issues:?}"
        );
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
        let source = read_workspace_file("crates/devil-ui/src/ui.rs");
        let manifest = read_workspace_file("crates/devil-ui/Cargo.toml");

        assert!(source.contains("pub struct Shell"));
        assert!(source.contains("CommandDispatchIntent"));
        assert!(source.contains("without mutating editor or workspace state"));
        assert!(!source.contains("EditorSession"));
        assert!(!source.contains("WorkspaceActor"));
        assert!(!source.contains("EditorEngine"));
        assert!(!source.contains("SaveWorkflowService"));
        assert!(!manifest.contains("devil-editor"));
        assert!(!manifest.contains("devil-project"));
        assert!(!manifest.contains("devil-storage"));
        assert!(!manifest.contains("devil-app"));
    }

    #[test]
    fn save_active_buffer_remains_proposal_mediated() {
        let source = read_workspace_file("crates/devil-app/src/lib.rs");

        assert!(source.contains("struct SaveWorkflowService;"));
        assert!(source.contains("SaveWorkflowService::save_active_buffer("));
        assert!(source.contains("workspace.save_file_with_proposal(workspace_save)"));
        assert!(source.contains("AppSaveOutcome::Rejected"));
    }

    #[test]
    fn source_snapshots_are_not_persisted_by_default() {
        let source = read_workspace_file("crates/devil-storage/src/lib.rs");
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
            "crates/devil-agent/src/lib.rs",
            "crates/devil-tracker/src/lib.rs",
            "crates/devil-memory/src/lib.rs",
        ] {
            assert_phase4_runtime_surface_preserves_boundaries(path);
        }
    }
}
