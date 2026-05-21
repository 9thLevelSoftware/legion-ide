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
const DEFAULT_PHASE3_EVIDENCE_PATH: &str = "plans/evidence/phase-3/predictive-semantic-fabric.md";
const PHASE3_STATUS_HEADING: &str = "## Acceptance status";
const PHASE3_FINAL_CHECKLIST_HEADING: &str = "## Final validation checklist";
const PHASE3_PARTIAL_RUNTIME_MARKER: &str = "Runtime surface status: Partial `devil-index` indexing behavior is active; acceptance evidence is incomplete.";
const PHASE3_NOT_ACCEPTED_MARKER: &str = "Phase 3 acceptance: Not accepted.";
const PHASE3_ACCEPTED_MARKER: &str = "Phase 3 acceptance: Accepted.";
const LSP_NOT_ACCEPTED_MARKER: &str = "LSP supervision acceptance: Not accepted.";
const LSP_ACCEPTED_MARKER: &str = "LSP supervision acceptance: Accepted.";
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

    let mut all = violations;
    all.extend(protocol_violations);
    all.extend(phase3_violations);

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

    fn assert_placeholder_docs_only(relative_path: &str) {
        let source = read_workspace_file(relative_path);
        let code_lines = source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter(|line| !line.starts_with("//!"))
            .filter(|line| !line.starts_with("#!["))
            .collect::<Vec<_>>();

        assert!(
            code_lines.is_empty(),
            "placeholder crate `{relative_path}` must remain docs-only, found code lines: {code_lines:?}"
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
    fn phase3_evidence_declares_partial_activation_not_acceptance() {
        let source = read_workspace_file(DEFAULT_PHASE3_EVIDENCE_PATH);
        let issues = validate_phase3_acceptance_governance(&source, |_| false);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
        assert!(source.contains("This document is not implementation evidence yet"));
        for artifact in PHASE3_REQUIRED_ARTIFACTS {
            assert!(
                source.contains(artifact),
                "Phase 3 scaffold must list required artifact `{artifact}`"
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
    fn placeholder_crates_remain_inert_until_activation_gates_land() {
        for path in [
            "crates/devil-agent/src/lib.rs",
            "crates/devil-tracker/src/lib.rs",
            "crates/devil-memory/src/lib.rs",
        ] {
            assert_placeholder_docs_only(path);
        }
    }
}
