use legion_protocol::DelegatedTaskRuntimeActivationState;
use legion_sandbox::{SandboxBackend, SandboxScope};
use legion_ui::ShellProjectionSnapshot;

#[cfg(target_os = "linux")]
use legion_sandbox::landlock::LandlockProfile;
#[cfg(target_os = "macos")]
use legion_sandbox::seatbelt::SeatbeltProfile;
#[cfg(target_os = "windows")]
use legion_sandbox::windows::WindowsProfile;

/// How many sandbox rows the panel renders before collapsing the rest into an
/// "N more rows" line.
///
/// Shared with the render call site so the row order in [`rows`] and the number
/// of rows actually drawn cannot drift apart. They did: the panel drew five of
/// eleven rows, and the line saying Windows enforces nothing but process
/// lifetime was the ninth.
pub(crate) const PANEL_VISIBLE_ROW_LIMIT: usize = 5;

/// Windows enforces process lifetime and nothing else.
///
/// `legion-sandbox` spawns through a Job Object with `KILL_ON_JOB_CLOSE` and
/// reports `windows-no-filesystem-enforcement`; `docs/SECURITY.md` records the
/// same residual. The panel must say it where the reader can see it.
const WINDOWS_LIMITATION: &str =
    "Windows Job Object only: process lifetime enforced; filesystem and network are not";

const MACOS_LIMITATION: &str =
    "Seatbelt enforces filesystem and network scope for the sandboxed process";

const LINUX_LIMITATION: &str =
    "Landlock enforces filesystem writes; network deny-all only when bwrap is present";

const UNKNOWN_TARGET_LIMITATION: &str =
    "no sandbox backend on this target: nothing is enforced by the OS";

/// The limitation line for the host this build targets.
///
/// `cfg!` rather than `#[cfg]` so every branch is compiled on every target:
/// the four lines above are then checked by the same tests everywhere, instead
/// of three of them going unread until someone builds for that platform.
fn platform_limitation() -> &'static str {
    if cfg!(target_os = "windows") {
        WINDOWS_LIMITATION
    } else if cfg!(target_os = "macos") {
        MACOS_LIMITATION
    } else if cfg!(target_os = "linux") {
        LINUX_LIMITATION
    } else {
        UNKNOWN_TARGET_LIMITATION
    }
}

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
        /// One line naming what this platform does *not* contain.
        platform_limitation: String,
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
                    platform_limitation: summary.platform_limitation,
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
    /// What this platform does not contain, in one line.
    ///
    /// Separate from `caveats` because it is the one line that must never be
    /// truncated away: the panel renders a bounded number of rows, and the
    /// caveat list is long enough that the Windows "filesystem and network are
    /// not enforced" line sat below the cut. See `rows`.
    platform_limitation: String,
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
            platform_limitation,
            caveats,
            lease_held,
        } => {
            rows.push(format!(
                "sandbox backend: {} (strength={})",
                backend_label, strength_label
            ));
            // Third row, deliberately. The panel renders only the first few
            // rows and hides the rest behind an "N more rows" line, so a
            // limitation pushed down among the caveats is a limitation the
            // reader never sees. On Windows that hid the fact that the sandbox
            // enforces process lifetime and nothing else, while the visible
            // rows still said "profile compiled fail-closed".
            rows.push(format!("sandbox limits: {platform_limitation}"));
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
            platform_limitation: platform_limitation().to_string(),
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
            platform_limitation: platform_limitation().to_string(),
            caveats,
        };
    }

    #[cfg(target_os = "windows")]
    {
        let profile = WindowsProfile::compile(scope).expect("windows sandbox profile compiles");
        // `profile.notes` is dropped here on purpose. It says "filesystem scope
        // limited to workspace root" and "egress remains allowlist-based and
        // audited", which describe the scope the caller *requested*, not
        // anything the Windows spawn path enforces: `legion-sandbox` uses a Job
        // Object with KILL_ON_JOB_CLOSE and reports
        // `windows-no-filesystem-enforcement`, and `docs/SECURITY.md` says so
        // in as many words. Rendering those lines as sandbox caveats told the
        // reader the workspace boundary was contained when it is not.
        let mut caveats: Vec<String> = profile.profile.notes;
        caveats.push(
            "workspace-root scope and egress allowlist are requested policy, not enforced containment"
                .to_string(),
        );
        caveats.push(
            "product spawn: live SandboxEnforcementReport remains authoritative after each TerminalCommand"
                .to_string(),
        );
        return SandboxProfileSummary {
            backend_label: sandbox_backend_label(&profile.profile.backend),
            strength_label: sandbox_strength_label(&profile.profile.backend).to_string(),
            platform_limitation: platform_limitation().to_string(),
            caveats,
        };
    }

    #[allow(unreachable_code)]
    {
        SandboxProfileSummary {
            backend_label: "unknown".to_string(),
            strength_label: "unknown".to_string(),
            platform_limitation: platform_limitation().to_string(),
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

    /// Longest a row may be before the renderer elides its middle.
    ///
    /// Taken from the renderer rather than restated, so the budget these tests
    /// hold the lines to is the budget the panel actually applies.
    use crate::view::COMPACT_ROW_CHAR_BUDGET as PANEL_ROW_CHAR_BUDGET;

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
            platform_limitation: "test-limitation".to_string(),
            caveats: vec!["test-caveat-a".to_string()],
            lease_held: true,
        }
    }

    /// The limitation row must sit inside the slice of rows the panel draws.
    ///
    /// This is the guard for the defect this field exists to fix: the row
    /// naming what the platform does not contain used to be ninth of eleven in
    /// a panel that draws five, so on Windows the reader saw
    /// "profile compiled fail-closed" and never saw that filesystem and network
    /// are unrestricted.
    #[test]
    fn the_platform_limitation_row_is_never_truncated_away() {
        for activation in [
            DelegatedTaskRuntimeActivationState::SandboxAllocated,
            DelegatedTaskRuntimeActivationState::Executing,
            DelegatedTaskRuntimeActivationState::Verifying,
            DelegatedTaskRuntimeActivationState::WaitingForApproval,
        ] {
            let snapshot = snapshot_with_activation(activation);
            let state = SandboxPanelState::from_snapshot(&snapshot);
            let panel_rows = rows(&snapshot, state);
            let index = panel_rows
                .iter()
                .position(|row| row.starts_with("sandbox limits: "))
                .unwrap_or_else(|| {
                    panic!("activation={activation:?}: no `sandbox limits:` row in {panel_rows:?}")
                });
            assert!(
                index < PANEL_VISIBLE_ROW_LIMIT,
                "activation={activation:?}: the limitation row is at index {index}, below the {PANEL_VISIBLE_ROW_LIMIT} rows the panel draws, so it is hidden behind the \"more rows\" line"
            );
        }
    }

    /// A limitation whose middle is replaced by an ellipsis is stated badly.
    #[test]
    fn every_platform_limitation_fits_the_rendered_row_budget() {
        for limitation in [
            WINDOWS_LIMITATION,
            MACOS_LIMITATION,
            LINUX_LIMITATION,
            UNKNOWN_TARGET_LIMITATION,
        ] {
            let row = format!("sandbox limits: {limitation}");
            assert!(
                row.chars().count() <= PANEL_ROW_CHAR_BUDGET,
                "`{row}` is {} chars, over the {PANEL_ROW_CHAR_BUDGET}-char budget, so the renderer would elide its middle",
                row.chars().count()
            );
        }
    }

    /// The Windows line has to name the Job Object and deny the two things the
    /// Job Object does not do. Checking the words rather than the whole string
    /// leaves the wording free to improve without letting it go quiet.
    #[test]
    fn the_windows_limitation_names_what_is_not_enforced() {
        let lowered = WINDOWS_LIMITATION.to_lowercase();
        for term in ["job object", "filesystem", "network", "not"] {
            assert!(
                lowered.contains(term),
                "the Windows limitation must mention `{term}`: {WINDOWS_LIMITATION}"
            );
        }
    }

    /// The panel must not repeat scope wording that Windows does not enforce.
    ///
    /// `WindowsProfile::notes` describes the requested scope — "filesystem
    /// scope limited to workspace root", "egress remains allowlist-based and
    /// audited" — and the panel used to render both as sandbox caveats. On a
    /// Job-Object-only host both are false, and `docs/SECURITY.md` says so.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_rows_do_not_claim_filesystem_or_egress_containment() {
        let snapshot =
            snapshot_with_activation(DelegatedTaskRuntimeActivationState::SandboxAllocated);
        let state = SandboxPanelState::from_snapshot(&snapshot);
        let all = rows(&snapshot, state).join("\n");
        for claim in [
            "filesystem scope limited to workspace root",
            "egress remains allowlist-based and audited",
        ] {
            assert!(
                !all.contains(claim),
                "the Windows sandbox panel claims `{claim}` while the spawn path enforces process lifetime only, got: {all}"
            );
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
        let mut snapshot = snapshot_with_activation(DelegatedTaskRuntimeActivationState::Executing);
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
