//! Manifest-level permission review for extension installs.
//!
//! P7.F2.T2. The unit of review here is one *capability*, never one extension.
//! The stop condition for this task is explicit: permissions must not collapse
//! into a single "trust this extension" toggle. That is enforced structurally
//! rather than by convention — [`ExtensionPermissionReview`] stores one
//! [`ExtensionPermissionDecision`] per requested capability, and
//! [`ExtensionPermissionReview::approval`] refuses to produce an approval while
//! any single capability is undecided or denied. There is no API on this type
//! that grants more than one capability at a time.

use std::collections::HashSet;

use legion_protocol::{CapabilityId, PluginContribution, PluginManifest};
use thiserror::Error;

/// How much authority a requested capability confers.
///
/// Used to order and badge the review rows so an elevated request cannot be
/// visually buried under a list of benign ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExtensionPermissionRisk {
    /// Metadata-only or presentation-only authority (grammars, themes).
    Standard,
    /// Capability that reaches beyond presentation (commands, providers, LSP).
    Elevated,
}

impl ExtensionPermissionRisk {
    /// Stable lowercase label for projections and audit rows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Elevated => "elevated",
        }
    }
}

/// One reviewable permission row: exactly one requested capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionPermissionRow {
    /// 1-based position in the review list.
    pub ordinal: u32,
    /// The capability this row grants or withholds.
    pub capability: CapabilityId,
    /// Short human-readable title for the capability.
    pub title: String,
    /// Why the extension asks for it, derived from its declared contributions.
    pub reason: String,
    /// Contribution labels that depend on this capability.
    pub contributions: Vec<String>,
    /// Authority classification for this row.
    pub risk: ExtensionPermissionRisk,
}

impl ExtensionPermissionRow {
    /// Render the row as a single stable audit/projection line.
    pub fn render(&self) -> String {
        format!(
            "permission review {}: capability={} risk={} reason={}",
            self.ordinal,
            self.capability.0,
            self.risk.label(),
            self.reason
        )
    }
}

/// Build structured permission-review rows for a plugin manifest install prompt.
///
/// One row per requested capability, in declaration order. This never merges,
/// deduplicates across capabilities, or summarizes: `rows.len()` is always
/// `manifest.requested_capabilities.len()`.
pub fn plugin_manifest_permission_review_rows(
    manifest: &PluginManifest,
) -> Vec<ExtensionPermissionRow> {
    // One row per *distinct* capability, not per entry.
    //
    // A manifest controls its own `requested_capabilities` and nothing rejects
    // a repeat, so the naive one-row-per-entry mapping produced two rows for
    // the same capability -- and `index_of` resolves a capability to the FIRST
    // matching row. The consequences were not cosmetic: the second row could
    // never be decided (`decide` only ever reaches the first), so the review
    // showed a permanently-undecided row while `approval()` succeeded anyway;
    // and a user who denied the duplicate row via `decide_at` had that denial
    // silently ignored. A manifest could therefore ship a capability twice and
    // be granted it by a review the user never completed.
    let mut seen: HashSet<&CapabilityId> =
        HashSet::with_capacity(manifest.requested_capabilities.len());
    let mut rows = Vec::with_capacity(manifest.requested_capabilities.len());
    for capability in &manifest.requested_capabilities {
        if !seen.insert(capability) {
            continue;
        }
        let contributions = contributions_for_capability(manifest, capability);
        let reason = contributions
            .first()
            .cloned()
            .unwrap_or_else(|| format!("requested capability {}", capability.0));
        rows.push(ExtensionPermissionRow {
            ordinal: u32::try_from(rows.len() + 1).unwrap_or(u32::MAX),
            capability: capability.clone(),
            title: capability_title(capability),
            reason,
            contributions,
            risk: capability_risk(capability),
        });
    }
    rows
}

/// Render every review row as a text line, one line per capability.
pub fn plugin_manifest_permission_review_lines(manifest: &PluginManifest) -> Vec<String> {
    plugin_manifest_permission_review_rows(manifest)
        .iter()
        .map(ExtensionPermissionRow::render)
        .collect()
}

fn capability_title(capability: &CapabilityId) -> String {
    match capability.0.as_str() {
        "plugin.command" => "Run editor commands".to_string(),
        "plugin.grammar.tree_sitter" => "Provide a syntax grammar".to_string(),
        "plugin.theme" => "Provide a color theme".to_string(),
        "plugin.formatter" => "Format documents".to_string(),
        "plugin.language.provider" => "Provide language intelligence".to_string(),
        "plugin.lsp.registration" => "Register a language server".to_string(),
        "plugin.workspace.scanner" => "Scan the workspace".to_string(),
        "plugin.ai.context" => "Contribute AI context".to_string(),
        other => format!("Use capability {other}"),
    }
}

fn capability_risk(capability: &CapabilityId) -> ExtensionPermissionRisk {
    // Deny-by-default posture applies to classification too: anything not on
    // the presentation-only list is treated as elevated.
    match capability.0.as_str() {
        "plugin.grammar.tree_sitter" | "plugin.theme" => ExtensionPermissionRisk::Standard,
        _ => ExtensionPermissionRisk::Elevated,
    }
}

fn contributions_for_capability(
    manifest: &PluginManifest,
    capability: &CapabilityId,
) -> Vec<String> {
    manifest
        .contributions
        .iter()
        .filter_map(|contribution| match contribution {
            PluginContribution::Command(command) if &command.required_capability == capability => {
                Some(format!("command {}", command.command_id))
            }
            PluginContribution::TreeSitterGrammar(grammar)
                if &grammar.required_capability == capability =>
            {
                Some(format!("tree-sitter grammar {}", grammar.grammar_name))
            }
            PluginContribution::Formatter(formatter)
                if formatter.command_id == capability.0 || capability.0 == "plugin.formatter" =>
            {
                Some(format!("formatter {}", formatter.command_id))
            }
            PluginContribution::LanguageProvider(provider)
                if capability.0 == "plugin.language.provider" =>
            {
                Some(format!("language provider {}", provider.provider_kind))
            }
            PluginContribution::LspRegistration(lsp)
                if capability.0 == "plugin.lsp.registration" =>
            {
                Some(format!("lsp registration {}", lsp.server_label))
            }
            PluginContribution::WorkspaceScanner(scanner)
                if capability.0 == "plugin.workspace.scanner" =>
            {
                Some(format!("workspace scanner {}", scanner.label))
            }
            PluginContribution::PassiveAiContextProvider(provider)
                if capability.0 == "plugin.ai.context" =>
            {
                Some(format!("passive ai context provider {}", provider.key))
            }
            _ => None,
        })
        .collect()
}

/// A per-capability decision recorded by the user during install review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionPermissionDecision {
    /// The user has not yet decided this row. Never treated as a grant.
    Undecided,
    /// The user granted this one capability.
    Granted,
    /// The user denied this one capability.
    Denied,
}

/// Why a permission review could not be turned into an install approval.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExtensionPermissionReviewError {
    /// The review was built for a different manifest than the one presented.
    #[error("permission review was built for manifest `{expected}`, not `{actual}`")]
    ManifestMismatch {
        /// Manifest the review was built for.
        expected: String,
        /// Manifest the caller tried to approve.
        actual: String,
    },
    /// The manifest requested a capability that has no review row.
    #[error("capability `{capability}` has no permission review row")]
    UnreviewedCapability {
        /// The unreviewed capability id.
        capability: String,
    },
    /// At least one row is still undecided.
    #[error("capability `{capability}` is still undecided in the permission review")]
    Undecided {
        /// The undecided capability id.
        capability: String,
    },
    /// At least one row was explicitly denied.
    #[error("capability `{capability}` was denied in the permission review")]
    Denied {
        /// The denied capability id.
        capability: String,
    },
}

/// An itemised install-time permission review bound to one manifest.
///
/// Construction always starts fully undecided: no capability is granted until a
/// caller decides that specific row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionPermissionReview {
    manifest_id: String,
    rows: Vec<ExtensionPermissionRow>,
    decisions: Vec<ExtensionPermissionDecision>,
}

impl ExtensionPermissionReview {
    /// Build an all-undecided review for a manifest.
    pub fn for_manifest(manifest: &PluginManifest) -> Self {
        let rows = plugin_manifest_permission_review_rows(manifest);
        let decisions = vec![ExtensionPermissionDecision::Undecided; rows.len()];
        Self {
            manifest_id: manifest.manifest_id.clone(),
            rows,
            decisions,
        }
    }

    /// Manifest id this review is bound to.
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    /// The itemised rows, one per requested capability.
    pub fn rows(&self) -> &[ExtensionPermissionRow] {
        &self.rows
    }

    /// Current decision for a row by index.
    pub fn decision_at(&self, index: usize) -> Option<ExtensionPermissionDecision> {
        self.decisions.get(index).copied()
    }

    /// Current decision for a capability.
    pub fn decision_for(&self, capability: &CapabilityId) -> Option<ExtensionPermissionDecision> {
        self.index_of(capability)
            .and_then(|index| self.decisions.get(index).copied())
    }

    /// Record a decision for exactly one capability.
    ///
    /// There is deliberately no bulk variant: a caller wanting to approve an
    /// extension must visit every row.
    pub fn decide(
        &mut self,
        capability: &CapabilityId,
        decision: ExtensionPermissionDecision,
    ) -> bool {
        match self.index_of(capability) {
            Some(index) => {
                self.decisions[index] = decision;
                true
            }
            None => false,
        }
    }

    /// Record a decision for exactly one row by index.
    pub fn decide_at(&mut self, index: usize, decision: ExtensionPermissionDecision) -> bool {
        match self.decisions.get_mut(index) {
            Some(slot) => {
                *slot = decision;
                true
            }
            None => false,
        }
    }

    /// Capabilities still awaiting a decision.
    pub fn undecided(&self) -> Vec<CapabilityId> {
        self.collect_with(ExtensionPermissionDecision::Undecided)
    }

    /// Capabilities the user explicitly denied.
    pub fn denied(&self) -> Vec<CapabilityId> {
        self.collect_with(ExtensionPermissionDecision::Denied)
    }

    /// Capabilities the user explicitly granted.
    pub fn granted(&self) -> Vec<CapabilityId> {
        self.collect_with(ExtensionPermissionDecision::Granted)
    }

    /// Whether every row has been decided one way or the other.
    pub fn is_complete(&self) -> bool {
        self.undecided().is_empty()
    }

    /// Turn a fully-granted review into an install approval.
    ///
    /// Fails closed on a manifest mismatch, an unreviewed capability, any
    /// undecided row, and any denied row.
    pub fn approval(
        &self,
        manifest: &PluginManifest,
    ) -> Result<ExtensionInstallApproval, ExtensionPermissionReviewError> {
        if manifest.manifest_id != self.manifest_id {
            return Err(ExtensionPermissionReviewError::ManifestMismatch {
                expected: self.manifest_id.clone(),
                actual: manifest.manifest_id.clone(),
            });
        }

        let mut granted = Vec::with_capacity(manifest.requested_capabilities.len());
        for capability in &manifest.requested_capabilities {
            let Some(index) = self.index_of(capability) else {
                return Err(ExtensionPermissionReviewError::UnreviewedCapability {
                    capability: capability.0.clone(),
                });
            };
            match self.decisions[index] {
                ExtensionPermissionDecision::Undecided => {
                    return Err(ExtensionPermissionReviewError::Undecided {
                        capability: capability.0.clone(),
                    });
                }
                ExtensionPermissionDecision::Denied => {
                    return Err(ExtensionPermissionReviewError::Denied {
                        capability: capability.0.clone(),
                    });
                }
                // Deduplicated to match the rows: a repeated capability is one
                // decision, so it is one grant.
                ExtensionPermissionDecision::Granted => {
                    if !granted.contains(capability) {
                        granted.push(capability.clone());
                    }
                }
            }
        }

        Ok(ExtensionInstallApproval {
            manifest_id: self.manifest_id.clone(),
            granted,
        })
    }

    fn index_of(&self, capability: &CapabilityId) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| &row.capability == capability)
    }

    fn collect_with(&self, wanted: ExtensionPermissionDecision) -> Vec<CapabilityId> {
        self.rows
            .iter()
            .zip(self.decisions.iter())
            .filter(|(_, decision)| **decision == wanted)
            .map(|(row, _)| row.capability.clone())
            .collect()
    }
}

/// Proof that every requested capability was individually granted.
///
/// Constructible only through [`ExtensionPermissionReview::approval`], so an
/// install path that demands one of these cannot be reached without an itemised
/// review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionInstallApproval {
    manifest_id: String,
    granted: Vec<CapabilityId>,
}

impl ExtensionInstallApproval {
    /// Manifest this approval was issued for.
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    /// Capabilities individually granted by the user.
    pub fn granted(&self) -> &[CapabilityId] {
        &self.granted
    }
}

#[cfg(test)]
mod tests {
    use legion_protocol::{
        CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor,
        PluginContribution, PluginId, PluginManifest, PluginQuotaDeclaration, PluginStateNamespace,
        PluginTreeSitterGrammarContribution, PluginTrustDecision, PluginTrustMetadata,
        PluginTrustSource,
    };

    use super::{
        ExtensionPermissionDecision, ExtensionPermissionReview, ExtensionPermissionReviewError,
        ExtensionPermissionRisk, plugin_manifest_permission_review_lines,
        plugin_manifest_permission_review_rows,
    };

    fn manifest() -> PluginManifest {
        let plugin_id = PluginId(7);
        PluginManifest {
            plugin_id,
            name: "phase8.desktop".to_string(),
            version: "0.1.0".to_string(),
            schema_version: 1,
            min_abi_version: 1,
            max_abi_version: 1,
            module_hash: "sha256:phase8:7".to_string(),
            manifest_id: "manifest:phase8:7".to_string(),
            trust: PluginTrustMetadata {
                source: PluginTrustSource::ExplicitLocalAllow,
                decision: PluginTrustDecision::ExplicitlyAllowed,
                reason: "desktop plugin management test allow".to_string(),
            },
            signature: None,
            activation_events: vec![PluginActivationEvent::OnCommand {
                command: "phase8.run".to_string(),
            }],
            contributions: vec![
                PluginContribution::Command(PluginCommandDescriptor {
                    command_id: "phase8.run".to_string(),
                    title: "Phase 8 Run".to_string(),
                    required_capability: CapabilityId("plugin.command".to_string()),
                }),
                PluginContribution::TreeSitterGrammar(PluginTreeSitterGrammarContribution {
                    language_id: LanguageId("rust-plugin".to_string()),
                    grammar_name: "rust-plugin-grammar".to_string(),
                    artifact_uri: "file:///tmp/rust-plugin-grammar.wasm".to_string(),
                    artifact_hash: "sha256:rust-plugin-grammar".to_string(),
                    required_capability: CapabilityId("plugin.grammar.tree_sitter".to_string()),
                }),
            ],
            requested_capabilities: vec![
                CapabilityId("plugin.command".to_string()),
                CapabilityId("plugin.grammar.tree_sitter".to_string()),
            ],
            storage_namespace: PluginStateNamespace {
                plugin_id,
                namespace: "state".to_string(),
            },
            quotas: PluginQuotaDeclaration {
                max_fuel: 1000,
                max_wall_time_ms: 50,
                max_memory_pages: 8,
                max_storage_bytes: 4096,
                max_host_calls: 4,
                max_events: 4,
                max_output_bytes: 512,
            },
        }
    }

    #[test]
    fn plugin_manifest_permission_review_rows_are_structured() {
        let rows = plugin_manifest_permission_review_rows(&manifest());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].capability.0, "plugin.command");
        assert_eq!(rows[0].reason, "command phase8.run");
        assert_eq!(rows[0].risk, ExtensionPermissionRisk::Elevated);
        assert_eq!(rows[1].capability.0, "plugin.grammar.tree_sitter");
        assert_eq!(rows[1].reason, "tree-sitter grammar rust-plugin-grammar");
        assert_eq!(rows[1].risk, ExtensionPermissionRisk::Standard);

        let lines = plugin_manifest_permission_review_lines(&manifest());
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("permission review 1"));
        assert!(lines[0].contains("capability=plugin.command"));
        assert!(lines[0].contains("risk=elevated"));
        assert!(lines[1].contains("capability=plugin.grammar.tree_sitter"));
    }

    /// A manifest that lists the same capability twice gets one row, one
    /// decision, and one grant.
    ///
    /// Before the dedup this was a hole with three separate symptoms, all from
    /// `index_of` resolving to the first matching row: the duplicate row could
    /// not be decided, `is_complete()` therefore stayed false while `approval()`
    /// succeeded, and denying the duplicate row had no effect on the outcome.
    #[test]
    fn a_capability_requested_twice_is_reviewed_once_and_granted_once() {
        let mut manifest = manifest();
        let repeated = manifest.requested_capabilities[0].clone();
        manifest.requested_capabilities.push(repeated.clone());

        let rows = plugin_manifest_permission_review_rows(&manifest);
        assert_eq!(
            rows.len(),
            2,
            "three entries naming two capabilities must produce two rows: {rows:?}"
        );
        assert_eq!(
            rows.iter().map(|row| row.ordinal).collect::<Vec<_>>(),
            vec![1, 2],
            "ordinals number the rows shown, so they must stay dense"
        );

        let mut review = ExtensionPermissionReview::for_manifest(&manifest);
        for row in review.rows().to_vec() {
            assert!(review.decide(&row.capability, ExtensionPermissionDecision::Granted));
        }
        assert!(
            review.is_complete(),
            "every row is decidable, so deciding each one completes the review"
        );

        let approval = review
            .approval(&manifest)
            .expect("a fully granted review approves");
        assert_eq!(
            approval
                .granted()
                .iter()
                .filter(|c| **c == repeated)
                .count(),
            1,
            "the repeated capability is granted once, not once per mention"
        );
    }

    /// Every row the review renders can be denied, and any denial refuses.
    ///
    /// Stated per-row on purpose. The first draft of this test denied by
    /// capability and passed with the dedup disabled, because `decide` resolves
    /// to the first matching row and that row was reachable either way -- it
    /// documented the outcome without guarding it. The defect only shows when a
    /// row is decided the way a UI decides it: by the index it was drawn at.
    /// With duplicate rows, denying the second one changed nothing and the
    /// install went through.
    #[test]
    fn denying_any_single_row_by_its_index_refuses_the_install() {
        let mut manifest = manifest();
        let repeated = manifest.requested_capabilities[0].clone();
        manifest.requested_capabilities.push(repeated);

        let row_count = ExtensionPermissionReview::for_manifest(&manifest)
            .rows()
            .len();
        for denied_index in 0..row_count {
            let mut review = ExtensionPermissionReview::for_manifest(&manifest);
            for index in 0..row_count {
                assert!(review.decide_at(index, ExtensionPermissionDecision::Granted));
            }
            assert!(review.decide_at(denied_index, ExtensionPermissionDecision::Denied));

            let error = review.approval(&manifest).expect_err(
                "denying row {denied_index} must refuse the install, whatever else was granted",
            );
            assert!(
                matches!(error, ExtensionPermissionReviewError::Denied { .. }),
                "row {denied_index}: expected a denial refusal, got {error:?}"
            );
        }
    }

    /// P7.F2.T2 stop condition, asserted directly: the review must expose one
    /// row per requested capability, not one collapsed extension-level toggle.
    #[test]
    fn permission_review_is_itemised_not_a_single_trust_toggle() {
        let manifest = manifest();
        let review = ExtensionPermissionReview::for_manifest(&manifest);

        assert_eq!(review.rows().len(), manifest.requested_capabilities.len());
        assert!(
            review.rows().len() > 1,
            "fixture must exercise more than one capability"
        );

        // Distinct capabilities, distinct titles, distinct reasons.
        assert_ne!(review.rows()[0].capability, review.rows()[1].capability);
        assert_ne!(review.rows()[0].title, review.rows()[1].title);
        assert_ne!(review.rows()[0].reason, review.rows()[1].reason);

        // Deciding one row must not decide any other row.
        let mut review = review;
        assert!(review.decide(
            &CapabilityId("plugin.command".to_string()),
            ExtensionPermissionDecision::Granted,
        ));
        assert_eq!(
            review.decision_for(&CapabilityId("plugin.command".to_string())),
            Some(ExtensionPermissionDecision::Granted)
        );
        assert_eq!(
            review.decision_for(&CapabilityId("plugin.grammar.tree_sitter".to_string())),
            Some(ExtensionPermissionDecision::Undecided),
            "granting one capability must not grant the others"
        );
        assert!(!review.is_complete());
    }

    #[test]
    fn permission_review_starts_undecided_and_refuses_approval() {
        let manifest = manifest();
        let review = ExtensionPermissionReview::for_manifest(&manifest);
        assert_eq!(review.granted(), Vec::new());
        assert_eq!(review.undecided().len(), 2);

        let error = review
            .approval(&manifest)
            .expect_err("an undecided review must not approve an install");
        assert_eq!(
            error,
            ExtensionPermissionReviewError::Undecided {
                capability: "plugin.command".to_string()
            }
        );
    }

    #[test]
    fn permission_review_refuses_approval_when_one_capability_is_denied() {
        let manifest = manifest();
        let mut review = ExtensionPermissionReview::for_manifest(&manifest);
        review.decide(
            &CapabilityId("plugin.command".to_string()),
            ExtensionPermissionDecision::Granted,
        );
        review.decide(
            &CapabilityId("plugin.grammar.tree_sitter".to_string()),
            ExtensionPermissionDecision::Denied,
        );
        assert!(review.is_complete());

        let error = review
            .approval(&manifest)
            .expect_err("a partially denied review must not approve an install");
        assert_eq!(
            error,
            ExtensionPermissionReviewError::Denied {
                capability: "plugin.grammar.tree_sitter".to_string()
            }
        );
    }

    #[test]
    fn permission_review_approves_only_when_every_row_is_granted() {
        let manifest = manifest();
        let mut review = ExtensionPermissionReview::for_manifest(&manifest);
        for row_index in 0..review.rows().len() {
            review.decide_at(row_index, ExtensionPermissionDecision::Granted);
        }
        let approval = review
            .approval(&manifest)
            .expect("fully granted review approves");
        assert_eq!(approval.manifest_id(), "manifest:phase8:7");
        assert_eq!(approval.granted().len(), 2);
    }

    #[test]
    fn permission_review_cannot_be_reused_for_another_manifest() {
        let manifest = manifest();
        let mut review = ExtensionPermissionReview::for_manifest(&manifest);
        for row_index in 0..review.rows().len() {
            review.decide_at(row_index, ExtensionPermissionDecision::Granted);
        }

        let mut other = manifest.clone();
        other.manifest_id = "manifest:someone-else".to_string();
        let error = review
            .approval(&other)
            .expect_err("a review must not approve a different manifest");
        assert_eq!(
            error,
            ExtensionPermissionReviewError::ManifestMismatch {
                expected: "manifest:phase8:7".to_string(),
                actual: "manifest:someone-else".to_string(),
            }
        );
    }

    #[test]
    fn permission_review_rejects_a_capability_with_no_row() {
        let manifest = manifest();
        let mut review = ExtensionPermissionReview::for_manifest(&manifest);
        for row_index in 0..review.rows().len() {
            review.decide_at(row_index, ExtensionPermissionDecision::Granted);
        }

        // A manifest that grew a capability after the prompt was rendered.
        let mut smuggled = manifest.clone();
        smuggled
            .requested_capabilities
            .push(CapabilityId("plugin.workspace.scanner".to_string()));
        let error = review
            .approval(&smuggled)
            .expect_err("a capability with no review row must not be approved");
        assert_eq!(
            error,
            ExtensionPermissionReviewError::UnreviewedCapability {
                capability: "plugin.workspace.scanner".to_string()
            }
        );
    }
}
