#![warn(missing_docs)]

//! Metadata-only VS Code extension compatibility analysis.
//!
//! This crate deliberately does not execute VSIX contents, spawn Node.js, read
//! extension files, or grant filesystem/process/network/editor authority. It
//! normalizes package metadata into `devil-protocol` DTOs so later product
//! gates can wire execution through explicit policy and proposal boundaries.

use std::collections::BTreeSet;

use devil_protocol::{
    CapabilityId, CausalityId, CorrelationId, EventSequence, PluginId, ProtocolDiagnosticSeverity,
    VsCodeActivationEvent, VsCodeCompatibilityDiagnostic, VsCodeCompatibilityStatus,
    VsCodeCompatibilityTier, VsCodeContributionDescriptor, VsCodeContributionKind,
    VsCodeExtensionHostRuntime, VsCodeExtensionHostSession, VsCodeExtensionId, VsCodeExtensionKind,
    VsCodeExtensionManifest,
};
use serde_json::Value;
use thiserror::Error;

/// Error returned when a VS Code extension manifest cannot be normalized.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VsCodeCompatError {
    /// Manifest metadata is invalid or incomplete.
    #[error("invalid VS Code extension manifest: {0}")]
    InvalidManifest(String),
    /// Protocol control identifiers are invalid.
    #[error("invalid protocol control identifiers")]
    InvalidControlIds,
}

/// Normalizes a VS Code `package.json` manifest into Devil compatibility DTOs.
pub fn manifest_from_package_json(
    plugin_id: PluginId,
    package_json: Value,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
) -> Result<VsCodeExtensionManifest, VsCodeCompatError> {
    validate_control_ids(correlation_id, causality_id, sequence)?;

    let publisher = required_string(&package_json, "publisher")?;
    let name = required_string(&package_json, "name")?;
    let version = required_string(&package_json, "version")?;
    let display_name =
        optional_string(&package_json, "displayName").unwrap_or_else(|| name.clone());
    let engine_vscode = package_json
        .get("engines")
        .and_then(|engines| engines.get("vscode"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let extension_kinds = extension_kinds(package_json.get("extensionKind"));
    let activation_events = activation_events(package_json.get("activationEvents"));
    let contributions = contributions(package_json.get("contributes"));

    let mut diagnostics = Vec::new();
    collect_activation_diagnostics(&activation_events, &mut diagnostics);
    collect_contribution_diagnostics(&contributions, &mut diagnostics);

    let required_tier = required_tier(&activation_events, &contributions);
    let status = aggregate_status(
        required_tier,
        &activation_events,
        &contributions,
        &diagnostics,
    );
    let requested_capabilities = requested_capabilities(&activation_events, &contributions);

    Ok(VsCodeExtensionManifest {
        extension_id: VsCodeExtensionId(format!("{publisher}.{name}")),
        plugin_id,
        publisher,
        name,
        display_name,
        version,
        engine_vscode,
        extension_kinds,
        activation_events,
        contributions,
        requested_capabilities,
        required_tier,
        status,
        diagnostics,
        correlation_id,
        causality_id,
        sequence,
        schema_version: 1,
    })
}

/// Builds a passive extension-host session descriptor for a normalized manifest.
pub fn extension_host_session_for_manifest(
    manifest: &VsCodeExtensionManifest,
) -> VsCodeExtensionHostSession {
    let runtime = match manifest.required_tier {
        VsCodeCompatibilityTier::Tier0Declarative
        | VsCodeCompatibilityTier::Tier1ProtocolAdapter => VsCodeExtensionHostRuntime::NoneRequired,
        VsCodeCompatibilityTier::Tier2ExtensionHostSidecar => {
            if manifest.extension_kinds.contains(&VsCodeExtensionKind::Web) {
                VsCodeExtensionHostRuntime::WebWorkerSidecar
            } else {
                VsCodeExtensionHostRuntime::NodeSidecar
            }
        }
        VsCodeCompatibilityTier::Tier3WebviewNotebookCustomEditor => {
            VsCodeExtensionHostRuntime::Deferred
        }
    };
    let process_label = match runtime {
        VsCodeExtensionHostRuntime::NoneRequired => "none-required",
        VsCodeExtensionHostRuntime::NodeSidecar => "node-extension-host-sidecar",
        VsCodeExtensionHostRuntime::WebWorkerSidecar => "web-worker-extension-host-sidecar",
        VsCodeExtensionHostRuntime::Deferred => "deferred-extension-host",
    };

    VsCodeExtensionHostSession {
        extension_id: manifest.extension_id.clone(),
        runtime,
        status: manifest.status,
        process_label: process_label.to_string(),
        requested_capabilities: manifest.requested_capabilities.clone(),
        correlation_id: manifest.correlation_id,
        causality_id: manifest.causality_id,
        sequence: manifest.sequence,
        schema_version: 1,
    }
}

/// Returns aggregate compatibility diagnostics for already-normalized manifests.
pub fn compatibility_diagnostics(
    manifests: &[VsCodeExtensionManifest],
) -> Vec<VsCodeCompatibilityDiagnostic> {
    manifests
        .iter()
        .flat_map(|manifest| manifest.diagnostics.iter().cloned())
        .collect()
}

fn validate_control_ids(
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    sequence: EventSequence,
) -> Result<(), VsCodeCompatError> {
    if correlation_id.0 == 0 || causality_id.0.is_nil() || sequence.0 == 0 {
        return Err(VsCodeCompatError::InvalidControlIds);
    }

    Ok(())
}

fn required_string(value: &Value, field: &str) -> Result<String, VsCodeCompatError> {
    optional_string(value, field)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| VsCodeCompatError::InvalidManifest(format!("missing `{field}`")))
}

fn optional_string(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(ToOwned::to_owned)
}

fn extension_kinds(value: Option<&Value>) -> Vec<VsCodeExtensionKind> {
    match value {
        Some(Value::String(kind)) => vec![extension_kind(kind)],
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(extension_kind)
            .collect(),
        _ => vec![VsCodeExtensionKind::Unknown],
    }
}

fn extension_kind(kind: &str) -> VsCodeExtensionKind {
    match kind {
        "ui" => VsCodeExtensionKind::Ui,
        "workspace" => VsCodeExtensionKind::Workspace,
        "web" => VsCodeExtensionKind::Web,
        _ => VsCodeExtensionKind::Unknown,
    }
}

fn activation_events(value: Option<&Value>) -> Vec<VsCodeActivationEvent> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(classify_activation_event)
            .collect(),
        _ => Vec::new(),
    }
}

fn classify_activation_event(raw: &str) -> VsCodeActivationEvent {
    let (tier, status) = if raw.starts_with("onLanguage:")
        || raw.starts_with("onCommand:")
        || raw.starts_with("onDebug")
        || raw.starts_with("workspaceContains:")
    {
        (
            VsCodeCompatibilityTier::Tier1ProtocolAdapter,
            VsCodeCompatibilityStatus::SupportedWithPolicy,
        )
    } else if raw == "onStartupFinished" {
        (
            VsCodeCompatibilityTier::Tier2ExtensionHostSidecar,
            VsCodeCompatibilityStatus::SupportedWithPolicy,
        )
    } else {
        (
            VsCodeCompatibilityTier::Tier2ExtensionHostSidecar,
            VsCodeCompatibilityStatus::Deferred,
        )
    };

    VsCodeActivationEvent {
        raw: raw.to_string(),
        tier,
        status,
    }
}

fn contributions(value: Option<&Value>) -> Vec<VsCodeContributionDescriptor> {
    let Some(Value::Object(map)) = value else {
        return Vec::new();
    };

    map.iter()
        .map(|(key, value)| {
            let (kind, tier, status, capability_label) = classify_contribution(key);
            VsCodeContributionDescriptor {
                kind,
                contribution_id: key.clone(),
                count: json_entry_count(value),
                tier,
                status,
                metadata_label: capability_label.to_string(),
            }
        })
        .collect()
}

fn classify_contribution(
    key: &str,
) -> (
    VsCodeContributionKind,
    VsCodeCompatibilityTier,
    VsCodeCompatibilityStatus,
    &'static str,
) {
    match key {
        "themes" => tier0(VsCodeContributionKind::Theme, "theme"),
        "iconThemes" => tier0(VsCodeContributionKind::IconTheme, "icon-theme"),
        "snippets" => tier0(VsCodeContributionKind::Snippet, "snippet"),
        "keybindings" => tier0(VsCodeContributionKind::Keybinding, "keybinding"),
        "languages" => tier0(VsCodeContributionKind::Language, "language"),
        "grammars" => tier0(VsCodeContributionKind::Grammar, "grammar"),
        "commands" => tier1(VsCodeContributionKind::Command, "command"),
        "configuration" => tier1(VsCodeContributionKind::Configuration, "configuration"),
        "menus" => tier1(VsCodeContributionKind::Menu, "menu"),
        "debuggers" => tier1(VsCodeContributionKind::Debugger, "debug-adapter"),
        "taskDefinitions" => tier1(VsCodeContributionKind::Task, "task"),
        "testing" | "tests" => tier1(VsCodeContributionKind::Test, "test"),
        "scm" | "sourceControl" => tier1(VsCodeContributionKind::Scm, "scm"),
        "views" | "viewsContainers" => (
            VsCodeContributionKind::View,
            VsCodeCompatibilityTier::Tier2ExtensionHostSidecar,
            VsCodeCompatibilityStatus::SupportedWithPolicy,
            "view",
        ),
        "webviews" => tier3(VsCodeContributionKind::Webview, "webview"),
        "notebooks" => tier3(VsCodeContributionKind::Notebook, "notebook"),
        "customEditors" => tier3(VsCodeContributionKind::CustomEditor, "custom-editor"),
        _ => (
            VsCodeContributionKind::Unknown,
            VsCodeCompatibilityTier::Tier2ExtensionHostSidecar,
            VsCodeCompatibilityStatus::Unsupported,
            "unknown",
        ),
    }
}

fn tier0(
    kind: VsCodeContributionKind,
    label: &'static str,
) -> (
    VsCodeContributionKind,
    VsCodeCompatibilityTier,
    VsCodeCompatibilityStatus,
    &'static str,
) {
    (
        kind,
        VsCodeCompatibilityTier::Tier0Declarative,
        VsCodeCompatibilityStatus::Supported,
        label,
    )
}

fn tier1(
    kind: VsCodeContributionKind,
    label: &'static str,
) -> (
    VsCodeContributionKind,
    VsCodeCompatibilityTier,
    VsCodeCompatibilityStatus,
    &'static str,
) {
    (
        kind,
        VsCodeCompatibilityTier::Tier1ProtocolAdapter,
        VsCodeCompatibilityStatus::SupportedWithPolicy,
        label,
    )
}

fn tier3(
    kind: VsCodeContributionKind,
    label: &'static str,
) -> (
    VsCodeContributionKind,
    VsCodeCompatibilityTier,
    VsCodeCompatibilityStatus,
    &'static str,
) {
    (
        kind,
        VsCodeCompatibilityTier::Tier3WebviewNotebookCustomEditor,
        VsCodeCompatibilityStatus::Deferred,
        label,
    )
}

fn json_entry_count(value: &Value) -> u32 {
    match value {
        Value::Array(values) => bounded_count(values.len()),
        Value::Object(map) => bounded_count(map.len()),
        Value::Null => 0,
        _ => 1,
    }
}

fn bounded_count(count: usize) -> u32 {
    count.min(u32::MAX as usize) as u32
}

fn collect_activation_diagnostics(
    activation_events: &[VsCodeActivationEvent],
    diagnostics: &mut Vec<VsCodeCompatibilityDiagnostic>,
) {
    for activation_event in activation_events {
        if activation_event.status == VsCodeCompatibilityStatus::Deferred {
            diagnostics.push(VsCodeCompatibilityDiagnostic {
                severity: ProtocolDiagnosticSeverity::Warning,
                code: "vscode.activation.deferred".to_string(),
                message: format!(
                    "activation event `{}` is deferred until extension-host policy is accepted",
                    activation_event.raw
                ),
                tier: Some(activation_event.tier),
                contribution_kind: None,
            });
        }
    }
}

fn collect_contribution_diagnostics(
    contributions: &[VsCodeContributionDescriptor],
    diagnostics: &mut Vec<VsCodeCompatibilityDiagnostic>,
) {
    for contribution in contributions {
        match contribution.status {
            VsCodeCompatibilityStatus::Deferred => {
                diagnostics.push(VsCodeCompatibilityDiagnostic {
                    severity: ProtocolDiagnosticSeverity::Warning,
                    code: "vscode.contribution.deferred".to_string(),
                    message: format!(
                        "contribution `{}` is deferred until Tier 3 policy is accepted",
                        contribution.contribution_id
                    ),
                    tier: Some(contribution.tier),
                    contribution_kind: Some(contribution.kind),
                });
            }
            VsCodeCompatibilityStatus::Unsupported => {
                diagnostics.push(VsCodeCompatibilityDiagnostic {
                    severity: ProtocolDiagnosticSeverity::Error,
                    code: "vscode.contribution.unsupported".to_string(),
                    message: format!(
                        "contribution `{}` is not covered by the compatibility matrix",
                        contribution.contribution_id
                    ),
                    tier: Some(contribution.tier),
                    contribution_kind: Some(contribution.kind),
                });
            }
            VsCodeCompatibilityStatus::Supported
            | VsCodeCompatibilityStatus::SupportedWithPolicy => {}
        }
    }
}

fn required_tier(
    activation_events: &[VsCodeActivationEvent],
    contributions: &[VsCodeContributionDescriptor],
) -> VsCodeCompatibilityTier {
    activation_events
        .iter()
        .map(|event| event.tier)
        .chain(contributions.iter().map(|contribution| contribution.tier))
        .max()
        .unwrap_or(VsCodeCompatibilityTier::Tier0Declarative)
}

fn aggregate_status(
    required_tier: VsCodeCompatibilityTier,
    activation_events: &[VsCodeActivationEvent],
    contributions: &[VsCodeContributionDescriptor],
    diagnostics: &[VsCodeCompatibilityDiagnostic],
) -> VsCodeCompatibilityStatus {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ProtocolDiagnosticSeverity::Error)
    {
        return VsCodeCompatibilityStatus::Unsupported;
    }

    if activation_events
        .iter()
        .any(|event| event.status == VsCodeCompatibilityStatus::Deferred)
        || contributions
            .iter()
            .any(|contribution| contribution.status == VsCodeCompatibilityStatus::Deferred)
    {
        return VsCodeCompatibilityStatus::Deferred;
    }

    if required_tier == VsCodeCompatibilityTier::Tier0Declarative {
        VsCodeCompatibilityStatus::Supported
    } else {
        VsCodeCompatibilityStatus::SupportedWithPolicy
    }
}

fn requested_capabilities(
    activation_events: &[VsCodeActivationEvent],
    contributions: &[VsCodeContributionDescriptor],
) -> Vec<CapabilityId> {
    let mut capabilities = BTreeSet::new();

    for activation_event in activation_events {
        if activation_event.tier >= VsCodeCompatibilityTier::Tier1ProtocolAdapter {
            capabilities.insert("vscode.extension.activate".to_string());
        }
    }

    for contribution in contributions {
        match contribution.kind {
            VsCodeContributionKind::Command | VsCodeContributionKind::Menu => {
                capabilities.insert("vscode.command.dispatch".to_string());
            }
            VsCodeContributionKind::Configuration => {
                capabilities.insert("vscode.configuration.read".to_string());
            }
            VsCodeContributionKind::Debugger => {
                capabilities.insert("debug.adapter.dispatch".to_string());
            }
            VsCodeContributionKind::Test => {
                capabilities.insert("test.runner.dispatch".to_string());
            }
            VsCodeContributionKind::Scm => {
                capabilities.insert("scm.provider.dispatch".to_string());
            }
            VsCodeContributionKind::View => {
                capabilities.insert("vscode.view.project".to_string());
            }
            VsCodeContributionKind::Webview
            | VsCodeContributionKind::Notebook
            | VsCodeContributionKind::CustomEditor => {
                capabilities.insert("vscode.webview.deferred".to_string());
            }
            VsCodeContributionKind::Task => {
                capabilities.insert("task.provider.dispatch".to_string());
            }
            VsCodeContributionKind::Theme
            | VsCodeContributionKind::IconTheme
            | VsCodeContributionKind::Snippet
            | VsCodeContributionKind::Keybinding
            | VsCodeContributionKind::Language
            | VsCodeContributionKind::Grammar
            | VsCodeContributionKind::Lsp
            | VsCodeContributionKind::Unknown => {}
        }
    }

    capabilities.into_iter().map(CapabilityId).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::RedactionHint;
    use serde_json::json;
    use uuid::Uuid;

    fn causality_id() -> CausalityId {
        CausalityId(Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap())
    }

    #[test]
    fn tier0_manifest_is_declarative_and_needs_no_host() {
        let manifest = manifest_from_package_json(
            PluginId(7),
            json!({
                "publisher": "devil",
                "name": "theme-fixture",
                "version": "1.0.0",
                "engines": { "vscode": "^1.90.0" },
                "contributes": {
                    "themes": [{ "label": "Devil Dark" }],
                    "snippets": [{ "language": "rust" }]
                }
            }),
            CorrelationId(1),
            causality_id(),
            EventSequence(1),
        )
        .expect("tier0 manifest normalizes");

        assert_eq!(manifest.status, VsCodeCompatibilityStatus::Supported);
        assert_eq!(
            manifest.required_tier,
            VsCodeCompatibilityTier::Tier0Declarative
        );
        assert!(manifest.requested_capabilities.is_empty());

        let session = extension_host_session_for_manifest(&manifest);
        assert_eq!(session.runtime, VsCodeExtensionHostRuntime::NoneRequired);
    }

    #[test]
    fn command_and_debug_manifest_is_policy_gated() {
        let manifest = manifest_from_package_json(
            PluginId(8),
            json!({
                "publisher": "devil",
                "name": "debug-fixture",
                "displayName": "Debug Fixture",
                "version": "1.0.0",
                "activationEvents": ["onCommand:devil.run", "onDebug"],
                "contributes": {
                    "commands": [{ "command": "devil.run", "title": "Run" }],
                    "debuggers": [{ "type": "lldb" }]
                }
            }),
            CorrelationId(2),
            causality_id(),
            EventSequence(2),
        )
        .expect("tier1 manifest normalizes");

        assert_eq!(
            manifest.status,
            VsCodeCompatibilityStatus::SupportedWithPolicy
        );
        assert!(
            manifest
                .requested_capabilities
                .contains(&CapabilityId("debug.adapter.dispatch".to_string()))
        );
        assert!(
            manifest
                .requested_capabilities
                .contains(&CapabilityId("vscode.command.dispatch".to_string()))
        );
    }

    #[test]
    fn webview_manifest_is_deferred_to_tier3() {
        let manifest = manifest_from_package_json(
            PluginId(9),
            json!({
                "publisher": "devil",
                "name": "webview-fixture",
                "version": "1.0.0",
                "contributes": {
                    "webviews": [{ "viewType": "devil.preview" }]
                }
            }),
            CorrelationId(3),
            causality_id(),
            EventSequence(3),
        )
        .expect("tier3 manifest normalizes");

        assert_eq!(manifest.status, VsCodeCompatibilityStatus::Deferred);
        assert_eq!(
            manifest.required_tier,
            VsCodeCompatibilityTier::Tier3WebviewNotebookCustomEditor
        );
        assert_eq!(
            manifest.diagnostics[0].severity,
            ProtocolDiagnosticSeverity::Warning
        );
    }

    #[test]
    fn invalid_manifest_and_control_ids_reject() {
        let missing = manifest_from_package_json(
            PluginId(10),
            json!({ "publisher": "devil", "version": "1.0.0" }),
            CorrelationId(4),
            causality_id(),
            EventSequence(4),
        );
        assert!(matches!(
            missing,
            Err(VsCodeCompatError::InvalidManifest(_))
        ));

        let bad_controls = manifest_from_package_json(
            PluginId(10),
            json!({ "publisher": "devil", "name": "bad", "version": "1.0.0" }),
            CorrelationId(0),
            causality_id(),
            EventSequence(4),
        );
        assert!(matches!(
            bad_controls,
            Err(VsCodeCompatError::InvalidControlIds)
        ));
    }

    #[test]
    fn compatibility_diagnostics_remain_metadata_only() {
        let manifest = manifest_from_package_json(
            PluginId(11),
            json!({
                "publisher": "devil",
                "name": "unknown-fixture",
                "version": "1.0.0",
                "contributes": {
                    "unknownSurface": { "some": "metadata" }
                }
            }),
            CorrelationId(5),
            causality_id(),
            EventSequence(5),
        )
        .expect("unsupported manifest still normalizes");

        let diagnostics = compatibility_diagnostics(&[manifest]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, ProtocolDiagnosticSeverity::Error);
        assert!(!diagnostics[0].message.contains("some"));

        let _redaction_marker = RedactionHint::MetadataOnly;
    }
}
