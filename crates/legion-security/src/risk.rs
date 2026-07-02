//! Deterministic risk-classification helpers for proposal auto-approval.

/// Coarse risk level emitted by deterministic risk rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// Safe to consider for auto-approval.
    Low,
    /// Requires manual review; must not be auto-approved.
    Elevated,
    /// Must be denied outright.
    Deny,
}

/// Returns true when `path` names a dependency manifest or lockfile for a
/// Legion-supported workspace ecosystem.
///
/// Dependency/manifest edits change the trust surface of a project, so they must
/// not be silently classified as low-risk. The comparison is performed on the
/// lowercased file name so both `/` and `\` path separators are handled.
pub fn is_dependency_or_lockfile(path: &str) -> bool {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();

    matches!(
        name.as_str(),
        // Rust
        "cargo.toml"
            | "cargo.lock"
            // JavaScript / TypeScript
            | "package.json"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "deno.json"
            | "deno.jsonc"
            | "deno.lock"
            | "bun.lockb"
            // Go
            | "go.mod"
            | "go.sum"
            // Python
            | "pyproject.toml"
            | "requirements.txt"
            | "pipfile"
            | "pipfile.lock"
            | "poetry.lock"
            // Ruby
            | "gemfile"
            | "gemfile.lock"
            // PHP
            | "composer.json"
            | "composer.lock"
            // JVM
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "gradle.lockfile"
    )
}

/// Evaluates the path-scope risk for a proposal.
///
/// When `workspace_root` is absent, path containment cannot be evaluated, so the
/// proposal must be treated as non-low (deny for auto-approval purposes) rather
/// than emitting an informational allow finding. Missing scope metadata is never
/// low-risk evidence.
pub fn path_scope_risk(workspace_root: Option<&str>, target_paths: &[String]) -> RiskLevel {
    let Some(root) = workspace_root
        .map(str::trim)
        .filter(|root| !root.is_empty())
    else {
        // Scope could not be evaluated; refuse to classify as low-risk.
        return RiskLevel::Deny;
    };

    let normalized_root = root.replace('\\', "/");
    let escapes_scope = target_paths.iter().any(|path| {
        let normalized = path.replace('\\', "/");
        !normalized.starts_with(&normalized_root) || normalized.contains("..")
    });

    if escapes_scope {
        RiskLevel::Elevated
    } else {
        RiskLevel::Low
    }
}

use legion_protocol::ProposalRiskLabel;
use legion_protocol::risk::{
    RiskAssessment, RiskRuleEngine, RiskRuleFinding, RiskRuleId, RiskRuleInput,
};

/// Thresholds used by the deterministic risk-rule engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskRuleThresholds {
    /// Maximum number of files that can be touched while remaining low risk.
    pub max_touched_files: usize,
    /// Maximum deletion ratio percentage that can remain low risk.
    pub max_deletion_ratio_percent: usize,
}

impl Default for RiskRuleThresholds {
    fn default() -> Self {
        Self {
            max_touched_files: 8,
            max_deletion_ratio_percent: 50,
        }
    }
}

/// Deterministic, metadata-only risk-rule engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeterministicRiskRuleEngine {
    thresholds: RiskRuleThresholds,
}

impl DeterministicRiskRuleEngine {
    /// Creates an engine with explicit thresholds.
    pub fn new(thresholds: RiskRuleThresholds) -> Self {
        Self { thresholds }
    }

    /// Evaluates the normalized input and returns all deterministic findings.
    pub fn evaluate(&self, input: &RiskRuleInput) -> RiskAssessment {
        <Self as RiskRuleEngine>::evaluate(self, input)
    }

    fn finding_for_rule(&self, rule_id: RiskRuleId, input: &RiskRuleInput) -> RiskRuleFinding {
        match rule_id {
            RiskRuleId::PathScope => {
                match path_scope_risk(input.workspace_root.as_deref(), &input.touched_paths) {
                    RiskLevel::Low => RiskRuleFinding::allow(
                        rule_id,
                        ProposalRiskLabel::Low,
                        vec!["all touched paths are inside workspace scope".to_string()],
                    ),
                    RiskLevel::Elevated | RiskLevel::Deny => RiskRuleFinding::deny(
                        rule_id,
                        vec!["one or more touched paths escape workspace scope".to_string()],
                    ),
                }
            }
            RiskRuleId::FileCount => {
                if input.touched_file_count() > self.thresholds.max_touched_files {
                    RiskRuleFinding::deny(
                        rule_id,
                        vec![format!(
                            "{} touched files exceeds limit {}",
                            input.touched_file_count(),
                            self.thresholds.max_touched_files
                        )],
                    )
                } else {
                    RiskRuleFinding::allow(
                        rule_id,
                        ProposalRiskLabel::Low,
                        vec![format!("{} touched files", input.touched_file_count())],
                    )
                }
            }
            RiskRuleId::DeletionRatio => {
                let touched = input.touched_file_count().max(1);
                let deletion_ratio = (input.deleted_file_count * 100) / touched;
                if deletion_ratio > self.thresholds.max_deletion_ratio_percent {
                    RiskRuleFinding::deny(
                        rule_id,
                        vec![format!(
                            "deletion ratio {deletion_ratio}% exceeds limit {}%",
                            self.thresholds.max_deletion_ratio_percent
                        )],
                    )
                } else {
                    RiskRuleFinding::allow(
                        rule_id,
                        ProposalRiskLabel::Low,
                        vec![format!("deletion ratio {deletion_ratio}%")],
                    )
                }
            }
            RiskRuleId::DependencyOrLockfileTouch => {
                if input
                    .touched_paths
                    .iter()
                    .any(|path| is_dependency_or_lockfile(path))
                {
                    RiskRuleFinding::deny(
                        rule_id,
                        vec!["dependency manifest or lockfile touched".to_string()],
                    )
                } else {
                    RiskRuleFinding::allow(
                        rule_id,
                        ProposalRiskLabel::Low,
                        vec!["no dependency manifests or lockfiles touched".to_string()],
                    )
                }
            }
            RiskRuleId::Migration => {
                if input.touched_paths.iter().any(|path| {
                    let normalized = path.replace('\\', "/").to_ascii_lowercase();
                    normalized.contains("/migrations/") || normalized.contains("/migration/")
                }) {
                    RiskRuleFinding::deny(rule_id, vec!["migration file touched".to_string()])
                } else {
                    RiskRuleFinding::allow(
                        rule_id,
                        ProposalRiskLabel::Low,
                        vec!["no migration files touched".to_string()],
                    )
                }
            }
            RiskRuleId::SecretsProximity => {
                if input.touched_paths.iter().any(|path| {
                    let normalized = path.replace('\\', "/").to_ascii_lowercase();
                    normalized.contains("/secrets/")
                        || normalized.contains("/.secrets/")
                        || normalized.contains("secret")
                        || normalized.contains("api_key")
                        || normalized.contains("credentials")
                }) {
                    RiskRuleFinding::deny(
                        rule_id,
                        vec!["path is near secrets or credential material".to_string()],
                    )
                } else {
                    RiskRuleFinding::allow(
                        rule_id,
                        ProposalRiskLabel::Low,
                        vec!["no secrets-adjacent paths touched".to_string()],
                    )
                }
            }
            RiskRuleId::BinaryOrGeneratedFileChange => {
                if input.touched_paths.iter().any(|path| {
                    let normalized = path.replace('\\', "/").to_ascii_lowercase();
                    normalized.contains("/target/")
                        || normalized.contains("/generated/")
                        || normalized.ends_with(".exe")
                        || normalized.ends_with(".dll")
                        || normalized.ends_with(".dylib")
                        || normalized.ends_with(".so")
                        || normalized.ends_with(".png")
                        || normalized.ends_with(".jpg")
                        || normalized.ends_with(".jpeg")
                        || normalized.ends_with(".gif")
                        || normalized.ends_with(".pdf")
                }) {
                    RiskRuleFinding::deny(
                        rule_id,
                        vec!["binary or generated file touched".to_string()],
                    )
                } else {
                    RiskRuleFinding::allow(
                        rule_id,
                        ProposalRiskLabel::Low,
                        vec!["no binary or generated files touched".to_string()],
                    )
                }
            }
        }
    }
}

impl Default for DeterministicRiskRuleEngine {
    fn default() -> Self {
        Self::new(RiskRuleThresholds::default())
    }
}

impl RiskRuleEngine for DeterministicRiskRuleEngine {
    fn evaluate(&self, input: &RiskRuleInput) -> RiskAssessment {
        let findings = RiskRuleId::all()
            .iter()
            .map(|rule_id| self.finding_for_rule(*rule_id, input))
            .collect::<Vec<_>>();
        let aggregate_risk_label = if findings.iter().any(|finding| finding.outcome.is_deny()) {
            ProposalRiskLabel::High
        } else {
            ProposalRiskLabel::Low
        };
        RiskAssessment {
            findings,
            aggregate_risk_label,
        }
    }
}

/// Evaluates risk rules using the default deterministic thresholds.
pub fn evaluate_risk_rules(input: &RiskRuleInput) -> RiskAssessment {
    DeterministicRiskRuleEngine::default().evaluate(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_and_lockfiles_are_detected() {
        assert!(is_dependency_or_lockfile("Cargo.toml"));
        assert!(is_dependency_or_lockfile("crates/foo/Cargo.toml"));
        assert!(is_dependency_or_lockfile("frontend\\package.json"));
        assert!(is_dependency_or_lockfile("Cargo.lock"));
        assert!(is_dependency_or_lockfile("pnpm-lock.yaml"));
        assert!(!is_dependency_or_lockfile("src/main.rs"));
        assert!(!is_dependency_or_lockfile("README.md"));
    }

    #[test]
    fn missing_workspace_root_is_not_low_risk() {
        assert_eq!(
            path_scope_risk(None, &["src/main.rs".to_string()]),
            RiskLevel::Deny
        );
        assert_eq!(
            path_scope_risk(Some("   "), &["src/main.rs".to_string()]),
            RiskLevel::Deny
        );
    }

    #[test]
    fn contained_paths_are_low_risk() {
        assert_eq!(
            path_scope_risk(Some("/repo"), &["/repo/src/main.rs".to_string()]),
            RiskLevel::Low
        );
        assert_eq!(
            path_scope_risk(Some("/repo"), &["/repo/../etc/passwd".to_string()]),
            RiskLevel::Elevated
        );
        assert_eq!(
            path_scope_risk(Some("/repo"), &["/outside/file".to_string()]),
            RiskLevel::Elevated
        );
    }
}
