use legion_protocol::DelegatedTaskRuntimeActivationState;
use legion_sandbox::{SandboxBackend, SandboxScope};
use legion_ui::ShellProjectionSnapshot;

#[cfg(target_os = "linux")]
use legion_sandbox::landlock::LandlockProfile;
#[cfg(target_os = "macos")]
use legion_sandbox::seatbelt::SeatbeltProfile;
#[cfg(target_os = "windows")]
use legion_sandbox::windows::WindowsProfile;

/// What the sandbox panel should display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SandboxPanelState {
    /// No sandbox allocated yet.
    NoSandbox,
    /// Sandbox allocated with enforcement data.
    Active {
        /// Human-readable label for the isolation mode (e.g. "git-worktree" or "directory-copy").
        isolation_mode_label: String,
        /// Backend used for OS-level enforcement.
        backend_label: String,
        /// Honest enforcement strength label.
        strength_label: String,
        /// Human-readable caveat descriptions for anything not enforced.
        caveats: Vec<String>,
        /// Whether an exclusive lease is held over the sandbox directory.
        lease_held: bool,
    },
}

impl SandboxPanelState {
    /// Derives panel state from a projection snapshot.
    ///
    /// Uses the runtime activation state to determine `NoSandbox` vs `Active`,
    /// and `host_profile_summary()` to populate the enforcement data for
    /// `Active` states. `isolation_mode_label` and `lease_held` will be replaced
    /// with richer data once the orchestrator state is piped through the snapshot.
    pub(crate) fn from_snapshot(snapshot: &ShellProjectionSnapshot) -> Self {
        let activation = snapshot.delegated_task_projection.runtime_activation;
        match activation {
            DelegatedTaskRuntimeActivationState::NotEncoded
            | DelegatedTaskRuntimeActivationState::Planned => SandboxPanelState::NoSandbox,
            _ => {
                let summary = host_profile_summary();
                let lease_held = !matches!(
                    activation,
                    DelegatedTaskRuntimeActivationState::Completed
                        | DelegatedTaskRuntimeActivationState::Cancelled
                        | DelegatedTaskRuntimeActivationState::Failed
                );
                SandboxPanelState::Active {
                    isolation_mode_label: "worktree-or-copy".to_string(),
                    backend_label: summary.backend_label,
                    strength_label: summary.strength_label,
                    caveats: summary.caveats,
                    lease_held,
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SandboxProfileSummary {
    backend_label: String,
    strength_label: String,
    caveats: Vec<String>,
}

pub(crate) fn rows(snapshot: &ShellProjectionSnapshot, state: SandboxPanelState) -> Vec<String> {
    let mut rows = Vec::with_capacity(6);
    let activation = snapshot.delegated_task_projection.runtime_activation;
    rows.push(format!(
        "delegated runtime: {}",
        runtime_activation_label(activation)
    ));

    match state {
        SandboxPanelState::NoSandbox => {
            rows.push("sandbox state: no sandbox/worktree allocated yet".to_string());
        }
        SandboxPanelState::Active {
            isolation_mode_label,
            backend_label,
            strength_label,
            caveats,
            lease_held,
        } => {
            rows.push(format!(
                "sandbox backend: {} (strength={})",
                backend_label, strength_label
            ));
            rows.push(format!("sandbox isolation: {}", isolation_mode_label));
            rows.push(format!(
                "sandbox lease: {}",
                if lease_held { "held" } else { "released" }
            ));
            rows.extend(
                caveats
                    .into_iter()
                    .map(|caveat| format!("sandbox caveat: {caveat}")),
            );
            // Surface live spawn enforcement lines if the delegated projection
            // recorded them (tool host appends "sandbox live enforcement: …").
            for disclaimer in &snapshot.delegated_task_projection.plan_only_disclaimers {
                if disclaimer.contains("sandbox live enforcement")
                    || disclaimer.starts_with("sandbox live enforcement")
                {
                    rows.push(format!("sandbox runtime: {disclaimer}"));
                }
            }
            rows.push(activation_state_row(activation));
        }
    }

    rows
}

fn activation_state_row(activation: DelegatedTaskRuntimeActivationState) -> String {
    match activation {
        DelegatedTaskRuntimeActivationState::NotEncoded
        | DelegatedTaskRuntimeActivationState::Planned => {
            "sandbox state: no sandbox/worktree allocated yet".to_string()
        }
        DelegatedTaskRuntimeActivationState::SandboxAllocated => {
            "sandbox state: sandbox allocated and isolated".to_string()
        }
        DelegatedTaskRuntimeActivationState::Executing => {
            "sandbox state: active execution inside sandbox".to_string()
        }
        DelegatedTaskRuntimeActivationState::Verifying => {
            "sandbox state: verification is running inside the isolated boundary".to_string()
        }
        DelegatedTaskRuntimeActivationState::WaitingForApproval => {
            "sandbox state: waiting for approval after sandbox allocation".to_string()
        }
        DelegatedTaskRuntimeActivationState::Blocked => {
            "sandbox state: blocked before sandbox reuse or allocation".to_string()
        }
        DelegatedTaskRuntimeActivationState::Completed => {
            "sandbox state: completed after isolated execution".to_string()
        }
        DelegatedTaskRuntimeActivationState::Cancelled => {
            "sandbox state: cancelled before completion".to_string()
        }
        DelegatedTaskRuntimeActivationState::Failed => {
            "sandbox state: failed after isolated execution".to_string()
        }
    }
}

fn host_profile_summary() -> SandboxProfileSummary {
    let scope = SandboxScope::workspace_only("(no active sandbox — descriptor only)");

    #[cfg(target_os = "macos")]
    {
        let profile = SeatbeltProfile::compile(scope);
        let mut caveats: Vec<String> = profile
            .profile
            .notes
            .into_iter()
            .chain(profile.rules)
            .collect();
        caveats.push(
            "product spawn: live SandboxEnforcementReport is authoritative after each TerminalCommand"
                .to_string(),
        );
        return SandboxProfileSummary {
            backend_label: sandbox_backend_label(&profile.profile.backend),
            strength_label: sandbox_strength_label(&profile.profile.backend).to_string(),
            caveats,
        };
    }

    #[cfg(target_os = "linux")]
    {
        let profile = LandlockProfile::compile(scope);
        let mut caveats: Vec<String> = profile
            .profile
            .notes
            .into_iter()
            .chain(profile.notes)
            .collect();
        // C1: deny-all network is enforced via bwrap --unshare-net when bwrap is
        // available. Selective egress allowlists remain unimplemented. The live
        // SandboxEnforcementReport from product spawn is authoritative.
        caveats.push(
            "FS write: Landlock. Network deny-all: bwrap --unshare-net when bwrap is available (empty egress); selective allowlist not implemented"
                .to_string(),
        );
        caveats.push(
            "product spawn: live SandboxEnforcementReport (backend/fs/network/caveats) is source of truth after each TerminalCommand"
                .to_string(),
        );
        return SandboxProfileSummary {
            backend_label: sandbox_backend_label(&profile.profile.backend),
            strength_label: sandbox_strength_label(&profile.profile.backend).to_string(),
            caveats,
        };
    }

    #[cfg(target_os = "windows")]
    {
        let profile = WindowsProfile::compile(scope).expect("windows sandbox profile compiles");
        let mut caveats: Vec<String> = profile
            .profile
            .notes
            .into_iter()
            .chain(profile.notes)
            .collect();
        caveats.push(
            "Windows sandbox enforces process lifetime (job kill-on-close); filesystem and network scope are not fully enforced (C2 residual)"
                .to_string(),
        );
        caveats.push(
            "product spawn: live SandboxEnforcementReport remains authoritative after each TerminalCommand"
                .to_string(),
        );
        return SandboxProfileSummary {
            backend_label: sandbox_backend_label(&profile.profile.backend),
            strength_label: sandbox_strength_label(&profile.profile.backend).to_string(),
            caveats,
        };
    }

    #[allow(unreachable_code)]
    {
        SandboxProfileSummary {
            backend_label: "unknown".to_string(),
            strength_label: "unknown".to_string(),
            caveats: vec!["sandbox backend unavailable on this target".to_string()],
        }
    }
}

fn sandbox_backend_label(backend: &SandboxBackend) -> String {
    match backend {
        SandboxBackend::Seatbelt => "Seatbelt".to_string(),
        SandboxBackend::BubblewrapLandlock => "BubblewrapLandlock".to_string(),
        SandboxBackend::RestrictedToken => "RestrictedToken".to_string(),
        SandboxBackend::AppContainer => "AppContainer".to_string(),
        SandboxBackend::DocumentedFallback { reason } => {
            format!("DocumentedFallback ({reason})")
        }
    }
}

fn sandbox_strength_label(backend: &SandboxBackend) -> &'static str {
    match backend {
        SandboxBackend::Seatbelt => "os-enforced",
        // Landlock FS-write always; network deny-all only when bwrap wraps spawn (C1/C3).
        SandboxBackend::BubblewrapLandlock => "os-enforced-fs-write; net-deny-all-if-bwrap",
        // Windows RestrictedToken/job path enforces process lifetime; FS/network not fully enforced.
        SandboxBackend::RestrictedToken => "process-lifetime-only",
        SandboxBackend::AppContainer => "os-enforced",
        SandboxBackend::DocumentedFallback { .. } => "fallback",
    }
}

fn runtime_activation_label(activation: DelegatedTaskRuntimeActivationState) -> &'static str {
    match activation {
        DelegatedTaskRuntimeActivationState::NotEncoded => "not encoded",
        DelegatedTaskRuntimeActivationState::Planned => "planned",
        DelegatedTaskRuntimeActivationState::SandboxAllocated => "sandbox allocated",
        DelegatedTaskRuntimeActivationState::Executing => "executing",
        DelegatedTaskRuntimeActivationState::Verifying => "verifying",
        DelegatedTaskRuntimeActivationState::WaitingForApproval => "waiting for approval",
        DelegatedTaskRuntimeActivationState::Blocked => "blocked",
        DelegatedTaskRuntimeActivationState::Completed => "completed",
        DelegatedTaskRuntimeActivationState::Cancelled => "cancelled",
        DelegatedTaskRuntimeActivationState::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::DelegatedTaskRuntimeActivationState;
    use legion_ui::Shell;

    fn snapshot_with_activation(
        activation: DelegatedTaskRuntimeActivationState,
    ) -> legion_ui::ShellProjectionSnapshot {
        let mut snapshot = Shell::empty("test").projection_snapshot();
        snapshot.delegated_task_projection.runtime_activation = activation;
        snapshot
    }

    fn active_state() -> SandboxPanelState {
        SandboxPanelState::Active {
            isolation_mode_label: "git-worktree".to_string(),
            backend_label: "TestBackend".to_string(),
            strength_label: "os-enforced".to_string(),
            caveats: vec!["test-caveat-a".to_string()],
            lease_held: true,
        }
    }

    /// Verify that `sandbox_strength_label` never returns "strong" for any
    /// `SandboxBackend` variant — "strong" was dishonest before PKT-SANDBOX
    /// enforcement landed. Labels must stay honest about partial enforcement.
    #[test]
    fn sandbox_strength_label_never_returns_strong() {
        let backends = [
            SandboxBackend::Seatbelt,
            SandboxBackend::BubblewrapLandlock,
            SandboxBackend::RestrictedToken,
            SandboxBackend::AppContainer,
            SandboxBackend::DocumentedFallback {
                reason: "test fallback".to_string(),
            },
        ];
        for backend in &backends {
            let label = sandbox_strength_label(backend);
            assert_ne!(
                label, "strong",
                "sandbox_strength_label returned 'strong' for {backend:?}"
            );
            assert_ne!(
                label, "descriptor-only",
                "sandbox_strength_label still returns 'descriptor-only' for {backend:?} — \
                 PKT-SANDBOX enforcement has landed, labels must be updated"
            );
        }
    }

    /// Verify honest labels per backend (Tier 0: partial-enforcement caveats).
    #[test]
    fn sandbox_strength_label_returns_honest_labels() {
        assert_eq!(
            sandbox_strength_label(&SandboxBackend::Seatbelt),
            "os-enforced"
        );
        assert_eq!(
            sandbox_strength_label(&SandboxBackend::BubblewrapLandlock),
            "os-enforced-fs-write; net-deny-all-if-bwrap"
        );
        assert_eq!(
            sandbox_strength_label(&SandboxBackend::RestrictedToken),
            "process-lifetime-only"
        );
        assert_eq!(
            sandbox_strength_label(&SandboxBackend::AppContainer),
            "os-enforced"
        );
        assert_eq!(
            sandbox_strength_label(&SandboxBackend::DocumentedFallback {
                reason: "test".to_string()
            }),
            "fallback"
        );
    }

    /// Live product-spawn enforcement lines on the projection surface as runtime rows.
    #[test]
    fn rows_surface_live_enforcement_disclaimer_from_projection() {
        let mut snapshot =
            snapshot_with_activation(DelegatedTaskRuntimeActivationState::Executing);
        snapshot
            .delegated_task_projection
            .plan_only_disclaimers
            .push(
                "sandbox live enforcement: backend=job-object-kill-on-close fs_write=false fs_read=false network=false caveats=windows-no-filesystem-enforcement"
                    .to_string(),
            );
        let panel_rows = rows(&snapshot, active_state());
        let all = panel_rows.join("\n");
        assert!(
            all.contains("sandbox runtime: sandbox live enforcement:"),
            "C3 product spawn: panel must surface live enforcement report, got: {all}"
        );
        assert!(
            all.contains("fs_write=false") || all.contains("backend="),
            "live enforcement row should include report fields, got: {all}"
        );
    }

    /// Panel rows for `NoSandbox` state show "no sandbox/worktree allocated yet".
    #[test]
    fn rows_nosandbox_state_shows_not_allocated() {
        let snapshot = snapshot_with_activation(DelegatedTaskRuntimeActivationState::NotEncoded);
        let rows = rows(&snapshot, SandboxPanelState::NoSandbox);
        let all = rows.join("\n");
        assert!(
            all.contains("no sandbox/worktree allocated yet"),
            "NoSandbox rows must contain 'no sandbox/worktree allocated yet', got: {all}"
        );
        // Must NOT show backend, isolation, or lease rows when no sandbox is allocated.
        assert!(
            !all.contains("sandbox backend:"),
            "NoSandbox rows must not contain 'sandbox backend:', got: {all}"
        );
        assert!(
            !all.contains("sandbox isolation:"),
            "NoSandbox rows must not contain 'sandbox isolation:', got: {all}"
        );
    }

    /// Panel rows for `Active` state show backend, strength, caveats, isolation mode,
    /// and lease status.
    #[test]
    fn rows_active_state_shows_all_enforcement_fields() {
        let snapshot =
            snapshot_with_activation(DelegatedTaskRuntimeActivationState::SandboxAllocated);
        let rows = rows(&snapshot, active_state());
        let all = rows.join("\n");
        assert!(
            all.contains("sandbox backend: TestBackend (strength=os-enforced)"),
            "Active rows must show backend and strength, got: {all}"
        );
        assert!(
            all.contains("sandbox isolation: git-worktree"),
            "Active rows must show isolation mode, got: {all}"
        );
        assert!(
            all.contains("sandbox lease: held"),
            "Active rows must show lease status, got: {all}"
        );
        assert!(
            all.contains("sandbox caveat: test-caveat-a"),
            "Active rows must show caveats, got: {all}"
        );
    }

    /// Verify that `rows()` output contains honest labels, not "descriptor-only" or "strong".
    #[test]
    fn rows_output_contains_honest_label_not_strong_or_descriptor_only() {
        let snapshot =
            snapshot_with_activation(DelegatedTaskRuntimeActivationState::SandboxAllocated);
        let state = SandboxPanelState::from_snapshot(&snapshot);
        let rows = rows(&snapshot, state);
        let all_output = rows.join("\n");
        assert!(
            all_output.contains("os-enforced")
                || all_output.contains("process-isolated")
                || all_output.contains("process-lifetime-only")
                || all_output.contains("fs-write")
                || all_output.contains("net-deny-all-if-bwrap")
                || all_output.contains("fallback"),
            "rows() output should contain an honest enforcement label, got: {all_output}",
        );
        assert!(
            !all_output.contains("strong"),
            "rows() output must not contain 'strong' — dishonest label, got: {all_output}",
        );
        assert!(
            !all_output.contains("descriptor-only"),
            "rows() output must not contain 'descriptor-only' after PKT-SANDBOX landed, got: {all_output}",
        );
    }
}
