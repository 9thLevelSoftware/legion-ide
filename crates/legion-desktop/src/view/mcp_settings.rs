//! Native HTTP MCP settings. The renderer collects decisions; app owns effects.

use crate::bridge::{DesktopAction, SensitiveString};
use crate::theme;
use legion_app::named_mcp_peer::{NamedMcpPeerConfig, NamedMcpPeerTransport};
use legion_protocol::{McpServerId, named_mcp_peer::*};

/// Explicit revision-bound lifecycle decision for the selected peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSettingsOperation {
    /// Grant only the displayed HTTP route; does not connect or permit tools.
    GrantTransport,
    /// Negotiate and ping through the existing app-owned HTTP client.
    Connect,
    /// Ping an already connected client.
    ProbeHealth,
    /// Revoke the local runtime and transport grant, retaining the stored token.
    RevokeGrant,
    /// Revoke locally and delete the current route-bound SecretStore token.
    RevokeCredential,
}

/// Adapter-local HTTP metadata form; never contains a bearer credential.
#[derive(Clone, PartialEq, Eq)]
pub struct McpHttpPeerForm {
    /// Stable identity; edits retain the original identity.
    pub peer_id: String,
    /// Display label.
    pub display_label: String,
    /// Exact endpoint; redacted in action diagnostics before validation.
    pub endpoint: SensitiveString,
    /// Explicit bearer authentication rather than provider fallback.
    pub bearer: bool,
    /// Whitespace-separated declared credential scopes.
    pub scopes: String,
    /// Displayed revision, or None for a new identity.
    pub expected_revision: Option<u64>,
}

impl std::fmt::Debug for McpHttpPeerForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("McpHttpPeerForm(<redacted metadata>)")
    }
}

impl Default for McpHttpPeerForm {
    fn default() -> Self {
        Self {
            peer_id: String::new(),
            display_label: String::new(),
            endpoint: SensitiveString(String::new()),
            bearer: false,
            scopes: String::new(),
            expected_revision: None,
        }
    }
}

impl McpHttpPeerForm {
    pub(crate) fn configuration(&self) -> NamedMcpPeerConfig {
        let mut config = NamedMcpPeerConfig::http_client(
            McpServerId(self.peer_id.clone()),
            &self.display_label,
            &self.endpoint.0,
        );
        if self.bearer {
            config.metadata.authentication = McpPeerAuthentication::Bearer;
            config.metadata.credential_scopes =
                self.scopes.split_whitespace().map(str::to_owned).collect();
        }
        config
    }
}

/// App-owned metadata plus session-local desktop selection, without credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSettingsPeerProjection {
    /// Current authoritative peer revision and lifecycle.
    pub snapshot: McpPeerSnapshot,
    /// Validated HTTP address for settings inspection only.
    pub endpoint: Option<String>,
    /// Explicit selection in this desktop session; never restored automatically.
    pub selected: bool,
    /// An app-owned network request is pending or draining after local revoke.
    pub busy: bool,
}

pub(crate) fn peer_projections(
    app: &legion_app::AppComposition,
    selected: Option<&McpServerId>,
) -> Vec<McpSettingsPeerProjection> {
    app.named_mcp_peer_configurations()
        .into_iter()
        .map(|config| {
            let snapshot = app
                .inspect_named_mcp_peer(&config.metadata.peer_id)
                .expect("enumerated peer");
            let endpoint = match config.transport {
                NamedMcpPeerTransport::Http { endpoint } => Some(endpoint),
                NamedMcpPeerTransport::Stdio { .. } => None,
            };
            McpSettingsPeerProjection {
                selected: selected == Some(&snapshot.metadata.peer_id),
                busy: app.named_mcp_peer_operation_pending(&snapshot.metadata.peer_id),
                snapshot,
                endpoint,
            }
        })
        .collect()
}

#[derive(Debug, Default)]
pub(crate) struct McpSettingsDraft {
    metadata: Option<McpHttpPeerForm>,
    submitted: bool,
    credential: Option<(McpServerId, u64, SensitiveString)>,
}

impl McpSettingsDraft {
    pub(crate) fn clear_sensitive(&mut self) {
        self.credential = None;
    }
}

pub(crate) fn render(
    ui: &mut egui::Ui,
    peers: &[McpSettingsPeerProjection],
    manual: bool,
    draft: &mut McpSettingsDraft,
    actions: &mut Vec<DesktopAction>,
) {
    if draft.submitted
        && draft.metadata.as_ref().is_some_and(|form| {
            let expected_revision = form.expected_revision.unwrap_or(0).checked_add(1);
            peers.iter().any(|peer| {
                Some(peer.snapshot.revision) == expected_revision
                    && peer.snapshot.metadata == form.configuration().metadata
                    && peer.endpoint.as_deref() == Some(form.endpoint.0.as_str())
            })
        })
    {
        draft.metadata = None;
        draft.submitted = false;
    }
    ui.label(theme::muted(
        "HTTP client · protocol 2025-11-25 · metadata-only privacy",
    ));
    ui.label(theme::muted(
        "Client stdio and server role are unsupported in these settings.",
    ));
    ui.label(theme::muted("Saving and selecting make no connection. Manual denies MCP network activity. No inference provider is required."));
    if super::soft_button(ui, "Add HTTP peer").clicked() {
        draft.clear_sensitive();
        draft.metadata = Some(McpHttpPeerForm::default());
        draft.submitted = false;
    }
    let mut saved = false;
    if let Some(form) = &mut draft.metadata {
        ui.add_enabled_ui(form.expected_revision.is_none(), |ui| {
            field(ui, "Peer ID", &mut form.peer_id, 128)
        });
        field(ui, "Peer label", &mut form.display_label, 128);
        field(ui, "HTTP endpoint", &mut form.endpoint.0, 4096);
        ui.checkbox(&mut form.bearer, "Use peer-specific bearer authentication");
        if form.bearer {
            field(ui, "Credential scopes", &mut form.scopes, 2048);
        }
        if super::soft_button(ui, "Save HTTP peer").clicked() {
            actions.push(DesktopAction::ConfigureNamedMcpHttpPeer { form: form.clone() });
            draft.submitted = true;
        }
        if super::soft_button(ui, "Cancel peer form").clicked() {
            saved = true;
        }
    }
    if saved {
        draft.metadata = None;
        draft.submitted = false;
    }
    if draft.metadata.is_some() {
        draft.clear_sensitive();
        return;
    }
    for peer in peers {
        ui.push_id(&peer.snapshot.metadata.peer_id.0, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::body_strong(&peer.snapshot.metadata.display_label));
                if peer.selected {
                    ui.label(theme::muted("Selected"));
                } else if ui
                    .add_enabled(
                        peer.endpoint.is_some()
                            && peer.snapshot.metadata.role == McpPeerRole::Client,
                        egui::Button::new("Select peer"),
                    )
                    .clicked()
                {
                    draft.clear_sensitive();
                    actions.push(DesktopAction::SelectNamedMcpPeer {
                        peer_id: peer.snapshot.metadata.peer_id.clone(),
                    });
                }
            });
        });
        ui.label(theme::muted(format!(
            "{} · Local health: {:?}",
            peer.snapshot.metadata.peer_id.0, peer.snapshot.health
        )));
    }
    let Some(selected) = peers.iter().find(|p| p.selected) else {
        draft.clear_sensitive();
        ui.label(theme::muted("No peer selected"));
        return;
    };
    ui.separator();
    if super::soft_button(ui, "Edit selected HTTP peer").clicked() {
        draft.submitted = false;
        draft.clear_sensitive();
        draft.metadata = Some(McpHttpPeerForm {
            peer_id: selected.snapshot.metadata.peer_id.0.clone(),
            display_label: selected.snapshot.metadata.display_label.clone(),
            endpoint: SensitiveString(selected.endpoint.clone().unwrap_or_default()),
            bearer: selected.snapshot.metadata.authentication == McpPeerAuthentication::Bearer,
            scopes: selected.snapshot.metadata.credential_scopes.join(" "),
            expected_revision: Some(selected.snapshot.revision),
        });
        return;
    }
    ui.label(theme::label(
        selected
            .endpoint
            .as_deref()
            .unwrap_or("Unsupported transport"),
    ));
    ui.label(theme::muted(format!(
        "Authentication: {:?} · declared scopes: {}",
        selected.snapshot.metadata.authentication,
        selected.snapshot.metadata.credential_scopes.join(" ")
    )));
    ui.label(theme::muted(format!(
        "Network grant: {} · revision {}",
        if selected.snapshot.transport_granted {
            "granted"
        } else {
            "not granted"
        },
        selected.snapshot.revision
    )));
    if manual {
        ui.label(theme::muted("Manual blocks connection. Choose a non-Manual mode explicitly; no inference provider is required."));
    }
    if selected.busy {
        ui.label(theme::muted(
            "MCP operation pending; revoked requests may still be draining.",
        ));
    }
    ui.label(theme::muted("A network grant does not authorize tool execution. Revoke is local; remote token invalidation is not implied."));
    let action = |operation| DesktopAction::ManageNamedMcpPeer {
        peer_id: selected.snapshot.metadata.peer_id.clone(),
        revision: selected.snapshot.revision,
        operation,
    };
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                !selected.busy,
                egui::Button::new("Grant selected HTTP route"),
            )
            .clicked()
        {
            actions.push(action(McpSettingsOperation::GrantTransport));
        }
        if ui
            .add_enabled(
                !selected.busy && !manual && selected.snapshot.transport_granted,
                egui::Button::new("Connect selected peer"),
            )
            .clicked()
        {
            actions.push(action(McpSettingsOperation::Connect));
        }
        if ui
            .add_enabled(
                !selected.busy
                    && !manual
                    && selected.snapshot.health == McpPeerHealth::Ready
                    && selected.snapshot.transport_granted,
                egui::Button::new("Probe selected peer health"),
            )
            .clicked()
        {
            actions.push(action(McpSettingsOperation::ProbeHealth));
        }
        if super::soft_button(ui, "Revoke local MCP grant").clicked() {
            actions.push(action(McpSettingsOperation::RevokeGrant));
        }
    });
    ui.label(theme::muted(format!(
        "Credential deletion: {:?}",
        selected.snapshot.credential_state
    )));
    if selected.snapshot.metadata.authentication != McpPeerAuthentication::Bearer {
        draft.clear_sensitive();
        return;
    }
    if draft.credential.as_ref().is_none_or(|(id, revision, _)| {
        id != &selected.snapshot.metadata.peer_id || *revision != selected.snapshot.revision
    }) {
        draft.credential = Some((
            selected.snapshot.metadata.peer_id.clone(),
            selected.snapshot.revision,
            SensitiveString(String::new()),
        ));
    }
    let (_, _, credential) = draft.credential.as_mut().expect("displayed bearer route");
    ui.horizontal(|ui| {
        let labelled = ui.label("MCP bearer token");
        let mut field = egui::TextEdit::singleline(&mut credential.0)
            .password(true)
            .char_limit(8192)
            .desired_width(320.0)
            .min_size(egui::vec2(32.0, 32.0))
            .id_salt((
                "mcp-settings-key",
                &selected.snapshot.metadata.peer_id.0,
                selected.snapshot.revision,
            ))
            .show(ui);
        field.state.clear_undoer();
        field.state.store(ui.ctx(), field.response.id);
        egui::Response::clone(&field.response).labelled_by(labelled.id);
    });
    ui.label(theme::muted("Stored in the operating system keyring for this exact endpoint and scopes. Replacing a token revokes the local grant; grant again explicitly."));
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                !credential.trim().is_empty(),
                egui::Button::new("Replace MCP token"),
            )
            .clicked()
        {
            actions.push(DesktopAction::ReplaceNamedMcpPeerCredential {
                peer_id: selected.snapshot.metadata.peer_id.clone(),
                revision: selected.snapshot.revision,
                credential: std::mem::replace(credential, SensitiveString(String::new())),
            });
        }
        if super::soft_button(ui, "Revoke MCP token and grant").clicked() {
            *credential = SensitiveString(String::new());
            actions.push(action(McpSettingsOperation::RevokeCredential));
        }
    });
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, limit: usize) {
    ui.horizontal(|ui| {
        let labelled = ui.label(label);
        ui.add(
            egui::TextEdit::singleline(value)
                .char_limit(limit)
                .desired_width(350.0)
                .id_salt(("mcp-settings-metadata", label)),
        )
        .labelled_by(labelled.id);
    });
}
