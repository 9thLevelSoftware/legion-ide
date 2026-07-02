use legion_protocol::DelegatedTaskRuntimeActivationState;
use legion_sandbox::{SandboxBackend, SandboxScope};
use legion_ui::ShellProjectionSnapshot;

#[cfg(target_os = "linux")]
use legion_sandbox::landlock::LandlockProfile;
#[cfg(target_os = "macos")]
use legion_sandbox::seatbelt::SeatbeltProfile;
#[cfg(target_os = "windows")]
use legion_sandbox::windows::WindowsProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SandboxProfileSummary {
    backend_label: String,
    strength_label: String,
    caveats: Vec<String>,
}

pub(crate) fn rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::with_capacity(4);
    let activation = snapshot.delegated_task_projection.runtime_activation;
    rows.push(format!(
        "delegated runtime: {}",
        runtime_activation_label(activation)
    ));

    let summary = host_profile_summary();
    rows.push(format!(
        "sandbox backend: {} (strength={})",
        summary.backend_label, summary.strength_label
    ));

    rows.extend(
        summary
            .caveats
            .into_iter()
            .map(|caveat| format!("sandbox caveat: {caveat}")),
    );

    rows.push(match activation {
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
    });

    rows
}

fn host_profile_summary() -> SandboxProfileSummary {
    let scope = SandboxScope::workspace_only("/workspace/project");

    #[cfg(target_os = "macos")]
    {
        let profile = SeatbeltProfile::compile(scope);
        return SandboxProfileSummary {
            backend_label: sandbox_backend_label(&profile.profile.backend),
            strength_label: sandbox_strength_label(&profile.profile.backend).to_string(),
            caveats: profile
                .profile
                .notes
                .into_iter()
                .chain(profile.rules)
                .collect(),
        };
    }

    #[cfg(target_os = "linux")]
    {
        let profile = LandlockProfile::compile(scope);
        return SandboxProfileSummary {
            backend_label: sandbox_backend_label(&profile.profile.backend),
            strength_label: sandbox_strength_label(&profile.profile.backend).to_string(),
            caveats: profile
                .profile
                .notes
                .into_iter()
                .chain(profile.notes.into_iter())
                .collect(),
        };
    }

    #[cfg(target_os = "windows")]
    {
        let profile = WindowsProfile::compile(scope).expect("windows sandbox profile compiles");
        return SandboxProfileSummary {
            backend_label: sandbox_backend_label(&profile.profile.backend),
            strength_label: sandbox_strength_label(&profile.profile.backend).to_string(),
            caveats: profile
                .profile
                .notes
                .into_iter()
                .chain(profile.notes.into_iter())
                .collect(),
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
        SandboxBackend::DocumentedFallback { .. } => "weaker",
        _ => "strong",
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
