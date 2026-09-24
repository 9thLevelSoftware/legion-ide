//! OS sandbox profiles and fail-closed audit decisions for worker execution.

#![warn(missing_docs)]

use std::{
    collections::BTreeSet,
    fmt,
    path::{Component, Path, PathBuf},
};

pub mod landlock;
pub mod network;
pub mod seatbelt;
pub mod spawn;
/// Long-lived sandboxed stdio spawn (DAP / interactive protocols).
pub mod spawn_stdio;
pub mod windows;

/// Platform-specific sandbox backend selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxBackend {
    /// macOS Seatbelt enforcement.
    Seatbelt,
    /// Linux bubblewrap + Landlock enforcement.
    BubblewrapLandlock,
    /// Windows restricted token enforcement.
    RestrictedToken,
    /// Windows AppContainer enforcement.
    AppContainer,
    /// Explicit, documented fallback with weaker guarantees.
    DocumentedFallback {
        /// Why the stronger backend was unavailable.
        reason: String,
    },
}

/// High-level platform selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPlatform {
    /// macOS.
    MacOS,
    /// Linux.
    Linux,
    /// Windows.
    Windows,
    /// Any other host platform.
    Other,
}

/// Enforcement action being audited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxAction {
    /// A filesystem write attempt.
    Write {
        /// Target path being written.
        path: PathBuf,
    },
    /// A filesystem read attempt.
    Read {
        /// Target path being read.
        path: PathBuf,
    },
    /// A raw egress attempt.
    Egress {
        /// Target hostname or URL.
        target: String,
    },
    /// Sandbox activation.
    Activate,
}

/// Audit record emitted for every sandbox decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxAuditEvent {
    /// Platform the decision was made for.
    pub platform: SandboxPlatform,
    /// Backend used or attempted.
    pub backend: SandboxBackend,
    /// Action that was evaluated.
    pub action: SandboxAction,
    /// Whether the action was allowed.
    pub allowed: bool,
    /// Human-readable reason for the decision.
    pub reason: String,
}

/// Fail-closed decision with attached audit event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxDecision {
    /// Whether the action is allowed.
    pub allowed: bool,
    /// Audit event for the decision.
    pub audit: SandboxAuditEvent,
}

impl SandboxDecision {
    fn allow(
        platform: SandboxPlatform,
        backend: SandboxBackend,
        action: SandboxAction,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            allowed: true,
            audit: SandboxAuditEvent {
                platform,
                backend,
                action,
                allowed: true,
                reason: reason.into(),
            },
        }
    }

    fn deny(
        platform: SandboxPlatform,
        backend: SandboxBackend,
        action: SandboxAction,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            allowed: false,
            audit: SandboxAuditEvent {
                platform,
                backend,
                action,
                allowed: false,
                reason: reason.into(),
            },
        }
    }
}

/// Sandbox scope used by all backends.
///
/// Reads are scoped as deliberately as writes. `workspace_root` is the only
/// readable location unless a caller adds more with [`SandboxScope::with_readable_root`],
/// and any prefix added to `denied_read_paths` is refused even when it falls
/// inside a readable root (deny wins over allow).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxScope {
    /// Workspace root that is writable.
    pub workspace_root: PathBuf,
    /// Allowed egress destinations.
    pub allowed_egress: BTreeSet<String>,
    /// Roots readable in addition to `workspace_root`. Empty by default: a
    /// scope that names no extra roots confines reads to the workspace root.
    pub readable_roots: BTreeSet<PathBuf>,
    /// Path prefixes that are refused for reads even when they resolve inside
    /// a readable root. Evaluated before the allow list, so a denied prefix
    /// cannot be re-opened by widening the readable roots.
    pub denied_read_paths: BTreeSet<PathBuf>,
}

impl SandboxScope {
    /// Creates a new scope that only allows workspace-local writes and reads.
    pub fn workspace_only(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            allowed_egress: BTreeSet::new(),
            readable_roots: BTreeSet::new(),
            denied_read_paths: BTreeSet::new(),
        }
    }

    /// Adds an allowed egress destination.
    pub fn with_egress(mut self, target: impl Into<String>) -> Self {
        self.allowed_egress.insert(target.into());
        self
    }

    /// Adds a root that may be read in addition to the workspace root.
    ///
    /// Writes are unaffected: widening the read surface never widens the write
    /// surface, because [`ActivatedSandbox::authorize_write`] only ever consults
    /// `workspace_root`.
    pub fn with_readable_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.readable_roots.insert(root.into());
        self
    }

    /// Refuses reads at or beneath `path`, even inside a readable root.
    pub fn deny_read_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.denied_read_paths.insert(path.into());
        self
    }
}

/// Whether the OS backend itself enforces the read boundary, or whether the
/// boundary only exists at the Legion decision layer.
///
/// This distinction is the read-side analogue of the "no silent fallback to
/// 'no sandbox'" rule: a caller that hands raw filesystem access to a worker
/// must not assume [`ActivatedSandbox::authorize_read`] constrains that worker,
/// because the decision layer is only consulted for reads that are *routed
/// through it*. A worker holding a real file descriptor bypasses it entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxReadEnforcement {
    /// The OS backend refuses out-of-scope reads by itself.
    OsEnforced,
    /// Only the Legion decision layer refuses out-of-scope reads. A worker with
    /// direct filesystem access is NOT contained by this.
    BrokerOnly {
        /// Why the backend does not enforce the read boundary.
        caveat: String,
    },
}

/// Reports, honestly, whether `backend` enforces read scope at the OS level.
///
/// Every backend currently returns [`SandboxReadEnforcement::BrokerOnly`], and
/// each returns its own reason rather than a shared placeholder, so that the
/// day a backend gains real read confinement only its arm changes:
///
/// * Seatbelt: the generated SBPL profile grants `(allow file-read* (subpath "/"))`.
/// * bubblewrap + Landlock: the ruleset handles write access rights only.
/// * Restricted token / AppContainer: the spawn path reports
///   `filesystem_read_enforced: false`.
///
/// Callers must not paper over a `BrokerOnly` answer. It is the reason a worker
/// that reads the filesystem directly cannot be admitted to a scoped run.
pub fn os_read_enforcement(backend: &SandboxBackend) -> SandboxReadEnforcement {
    let caveat = match backend {
        SandboxBackend::Seatbelt => "seatbelt-profile-allows-file-read-subpath-root",
        SandboxBackend::BubblewrapLandlock => "landlock-ruleset-handles-write-access-only",
        SandboxBackend::RestrictedToken => "windows-restricted-token-does-not-scope-reads",
        SandboxBackend::AppContainer => "windows-appcontainer-read-scoping-not-implemented",
        SandboxBackend::DocumentedFallback { reason } => {
            return SandboxReadEnforcement::BrokerOnly {
                caveat: format!("documented-fallback-does-not-scope-reads: {reason}"),
            };
        }
    };
    SandboxReadEnforcement::BrokerOnly {
        caveat: caveat.to_string(),
    }
}

/// Activated sandbox session with an audit log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivatedSandbox {
    platform: SandboxPlatform,
    backend: SandboxBackend,
    scope: SandboxScope,
    audit_log: Vec<SandboxAuditEvent>,
}

impl ActivatedSandbox {
    /// Activates a sandbox for the chosen platform and backend.
    pub fn activate(
        platform: SandboxPlatform,
        backend: SandboxBackend,
        scope: SandboxScope,
    ) -> Self {
        let mut sandbox = Self {
            platform,
            backend,
            scope,
            audit_log: Vec::new(),
        };
        sandbox.audit_log.push(SandboxAuditEvent {
            platform,
            backend: sandbox.backend.clone(),
            action: SandboxAction::Activate,
            allowed: true,
            reason: "sandbox activated".to_string(),
        });
        sandbox
    }

    /// Returns the configured backend.
    pub fn backend(&self) -> &SandboxBackend {
        &self.backend
    }

    /// Returns the audited decisions so far.
    pub fn audit_log(&self) -> &[SandboxAuditEvent] {
        &self.audit_log
    }

    /// Evaluates a write attempt and fails closed outside the workspace scope.
    pub fn authorize_write(&mut self, path: impl AsRef<Path>) -> SandboxDecision {
        let path = path.as_ref();
        let action = SandboxAction::Write {
            path: path.to_path_buf(),
        };
        if path_is_within_scope(path, &self.scope.workspace_root) {
            let decision = SandboxDecision::allow(
                self.platform,
                self.backend.clone(),
                action,
                "write stays inside workspace scope",
            );
            self.audit_log.push(decision.audit.clone());
            decision
        } else {
            let decision = SandboxDecision::deny(
                self.platform,
                self.backend.clone(),
                action,
                "write denied outside workspace scope",
            );
            self.audit_log.push(decision.audit.clone());
            decision
        }
    }

    /// Evaluates a read attempt and fails closed outside the readable scope.
    ///
    /// Order is load-bearing: the denied-prefix check runs BEFORE the readable
    /// roots check, so a path that a caller explicitly denied cannot be
    /// re-admitted by also listing an enclosing readable root. Both the
    /// candidate and every boundary are resolved through the same
    /// symlink-following resolution used by [`Self::authorize_write`], so an
    /// in-scope symlink pointing outside is measured at its real destination
    /// rather than its lexical spelling.
    pub fn authorize_read(&mut self, path: impl AsRef<Path>) -> SandboxDecision {
        let path = path.as_ref();
        let action = SandboxAction::Read {
            path: path.to_path_buf(),
        };

        if self
            .scope
            .denied_read_paths
            .iter()
            .any(|denied| path_is_within_scope(path, denied))
        {
            let decision = SandboxDecision::deny(
                self.platform,
                self.backend.clone(),
                action,
                "read denied by an explicit denied-read prefix",
            );
            self.audit_log.push(decision.audit.clone());
            return decision;
        }

        let readable = std::iter::once(&self.scope.workspace_root)
            .chain(self.scope.readable_roots.iter())
            .any(|root| path_is_within_scope(path, root));

        let decision = if readable {
            SandboxDecision::allow(
                self.platform,
                self.backend.clone(),
                action,
                "read stays inside readable scope",
            )
        } else {
            SandboxDecision::deny(
                self.platform,
                self.backend.clone(),
                action,
                "read denied outside readable scope",
            )
        };
        self.audit_log.push(decision.audit.clone());
        decision
    }

    /// Reports whether the OS backend enforces the read boundary by itself.
    pub fn os_read_enforcement(&self) -> SandboxReadEnforcement {
        os_read_enforcement(&self.backend)
    }

    /// Evaluates a raw egress attempt and fails closed unless it is explicitly allowed.
    pub fn authorize_egress(&mut self, target: impl Into<String>) -> SandboxDecision {
        network::authorize_egress(
            self.platform,
            self.backend.clone(),
            &self.scope,
            &mut self.audit_log,
            target,
        )
    }
}

/// A structured sandbox profile produced by the platform modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxProfile {
    /// Backend used by the profile.
    pub backend: SandboxBackend,
    /// Human-readable profile notes.
    pub notes: Vec<String>,
    /// Operational scope.
    pub scope: SandboxScope,
}

impl SandboxProfile {
    /// Creates a profile shell.
    pub fn new(backend: SandboxBackend, scope: SandboxScope) -> Self {
        Self {
            backend,
            notes: Vec::new(),
            scope,
        }
    }

    /// Adds a descriptive note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Sandbox errors are explicit and never translate to an implicit no-sandbox mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    /// The requested backend is unavailable on this host.
    UnsupportedBackend {
        /// Backend that could not be activated.
        backend: SandboxBackend,
    },
    /// The host requested a weaker documented fallback.
    DocumentedFallbackRequired {
        /// Reason the weaker documented fallback is required.
        reason: String,
    },
    /// The platform sandbox mechanism is unavailable.
    PlatformUnavailable {
        /// Which platform was attempted.
        platform: String,
        /// Why it's unavailable.
        reason: String,
    },
    /// The sandboxed process failed to spawn.
    SpawnFailed {
        /// Why the spawn failed.
        reason: String,
    },
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedBackend { backend } => {
                write!(f, "unsupported sandbox backend: {backend:?}")
            }
            Self::DocumentedFallbackRequired { reason } => {
                write!(f, "documented fallback required: {reason}")
            }
            Self::PlatformUnavailable { platform, reason } => {
                write!(f, "platform sandbox unavailable on {platform}: {reason}")
            }
            Self::SpawnFailed { reason } => {
                write!(f, "sandboxed spawn failed: {reason}")
            }
        }
    }
}

impl std::error::Error for SandboxError {}

/// Returns true when the path resolves to a location inside the scope.
///
/// Lexical normalization alone is not a safe write boundary: a symlink inside
/// the workspace can point outside it, so a purely textual prefix check can be
/// bypassed. To fail closed, both the candidate and the scope are resolved with
/// filesystem-aware canonicalization (which follows symlinks) before the prefix
/// comparison. Because the candidate is frequently a not-yet-created file, we
/// canonicalize the longest existing ancestor and re-append the remaining,
/// not-yet-created components lexically. When nothing along a path exists (for
/// example synthetic paths in unit tests), we fall back to lexical
/// normalization.
fn path_is_within_scope(candidate: &Path, scope: &Path) -> bool {
    let scope = resolve_for_scope_check(scope);
    if scope.components().count() == 0 {
        return false;
    }

    let candidate = resolve_for_scope_check(candidate);
    candidate.starts_with(&scope)
}

/// Resolve a path for boundary checking by canonicalizing its longest existing
/// ancestor (following symlinks) and re-appending the trailing components that
/// do not yet exist. Falls back to lexical normalization when no ancestor can
/// be canonicalized.
fn resolve_for_scope_check(path: &Path) -> PathBuf {
    let normalized = normalize_path(path);
    let mut existing = normalized.as_path();
    let mut trailing: Vec<&std::ffi::OsStr> = Vec::new();

    loop {
        if let Ok(canonical) = std::fs::canonicalize(existing) {
            let mut resolved = canonical;
            for component in trailing.iter().rev() {
                resolved.push(component);
            }
            return normalize_path(&resolved);
        }
        match (existing.file_name(), existing.parent()) {
            (Some(name), Some(parent)) => {
                trailing.push(name);
                existing = parent;
            }
            _ => return normalized,
        }
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_outside_scope_fails_closed_and_audits() {
        let scope = SandboxScope::workspace_only("/workspace/project");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::Linux,
            SandboxBackend::BubblewrapLandlock,
            scope,
        );

        let decision = sandbox.authorize_write("/etc/passwd");

        assert!(!decision.allowed);
        assert!(matches!(decision.audit.action, SandboxAction::Write { .. }));
        assert!(decision.audit.reason.contains("outside workspace scope"));
        assert_eq!(sandbox.audit_log().len(), 2);
    }

    /// The stop condition for a governed external-agent run is a READ escape,
    /// so this is the first thing that must fail closed.
    #[test]
    fn read_outside_scope_fails_closed_and_audits() {
        let scope = SandboxScope::workspace_only("/workspace/project");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::Linux,
            SandboxBackend::BubblewrapLandlock,
            scope,
        );

        let decision = sandbox.authorize_read("/etc/shadow");

        assert!(!decision.allowed);
        assert!(matches!(decision.audit.action, SandboxAction::Read { .. }));
        assert!(decision.audit.reason.contains("outside readable scope"));
        assert_eq!(
            sandbox.audit_log().len(),
            2,
            "a denied read must leave an audit row, not just a return value"
        );
    }

    /// A sibling directory whose name merely starts with the scope's name is
    /// outside the scope. A `String::starts_with` boundary check would let
    /// `/workspace/project-secrets` through.
    #[test]
    fn read_of_a_name_prefixed_sibling_directory_is_refused() {
        let scope = SandboxScope::workspace_only("/workspace/project");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::Linux,
            SandboxBackend::BubblewrapLandlock,
            scope,
        );

        let decision = sandbox.authorize_read("/workspace/project-secrets/key.pem");

        assert!(
            !decision.allowed,
            "a name-prefixed sibling is not inside the scope"
        );
    }

    /// `..` in a read request must be measured after normalization, not
    /// accepted because the spelling starts with the scope root.
    #[test]
    fn read_traversal_out_of_scope_is_refused() {
        let scope = SandboxScope::workspace_only("/workspace/project");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::Linux,
            SandboxBackend::BubblewrapLandlock,
            scope,
        );

        let decision = sandbox.authorize_read("/workspace/project/../secrets/key.pem");

        assert!(!decision.allowed);
    }

    /// Deny beats allow: listing a readable root that encloses a denied prefix
    /// must not re-open the denied prefix.
    #[test]
    fn denied_read_prefix_wins_over_an_enclosing_readable_root() {
        let scope = SandboxScope::workspace_only("/workspace/project")
            .with_readable_root("/workspace/project/vendor")
            .deny_read_path("/workspace/project/vendor/.credentials");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::Linux,
            SandboxBackend::BubblewrapLandlock,
            scope,
        );

        let decision = sandbox.authorize_read("/workspace/project/vendor/.credentials/token");

        assert!(!decision.allowed);
        assert!(decision.audit.reason.contains("denied-read prefix"));
    }

    #[test]
    fn read_inside_scope_is_allowed_and_audited() {
        let scope = SandboxScope::workspace_only("/workspace/project");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::Linux,
            SandboxBackend::BubblewrapLandlock,
            scope,
        );

        let decision = sandbox.authorize_read("/workspace/project/src/lib.rs");

        assert!(decision.allowed);
        assert_eq!(sandbox.audit_log().len(), 2);
    }

    /// Widening the read surface must not widen the write surface.
    #[test]
    fn an_extra_readable_root_does_not_become_writable() {
        let scope =
            SandboxScope::workspace_only("/workspace/project").with_readable_root("/opt/toolchain");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::Linux,
            SandboxBackend::BubblewrapLandlock,
            scope,
        );

        assert!(sandbox.authorize_read("/opt/toolchain/bin/rustc").allowed);
        assert!(
            !sandbox.authorize_write("/opt/toolchain/bin/rustc").allowed,
            "a readable root must stay read-only"
        );
    }

    /// No backend confines reads at the OS level today. This asserts the honest
    /// answer per backend so that a future backend that *does* confine reads
    /// has to change this test deliberately rather than inherit a stale claim.
    #[test]
    fn no_backend_claims_os_level_read_enforcement() {
        for backend in [
            SandboxBackend::Seatbelt,
            SandboxBackend::BubblewrapLandlock,
            SandboxBackend::RestrictedToken,
            SandboxBackend::AppContainer,
            SandboxBackend::DocumentedFallback {
                reason: "no supported backend".to_string(),
            },
        ] {
            match os_read_enforcement(&backend) {
                SandboxReadEnforcement::BrokerOnly { caveat } => {
                    assert!(
                        !caveat.trim().is_empty(),
                        "{backend:?} must name why reads are unconfined"
                    );
                }
                SandboxReadEnforcement::OsEnforced => panic!(
                    "{backend:?} claims OS-level read enforcement that the spawn path does not implement"
                ),
            }
        }
    }

    #[test]
    fn raw_egress_without_permission_fails_closed_and_audits() {
        let scope = SandboxScope::workspace_only("/workspace/project");
        let mut sandbox =
            ActivatedSandbox::activate(SandboxPlatform::MacOS, SandboxBackend::Seatbelt, scope);

        let decision = sandbox.authorize_egress("https://example.com");

        assert!(!decision.allowed);
        assert!(matches!(
            decision.audit.action,
            SandboxAction::Egress { .. }
        ));
        assert!(decision.audit.reason.contains("raw egress denied"));
    }

    #[test]
    fn allowed_egress_is_only_granted_when_explicitly_listed() {
        let scope = SandboxScope::workspace_only("/workspace/project").with_egress("localhost");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::Windows,
            SandboxBackend::AppContainer,
            scope,
        );

        let decision = sandbox.authorize_egress("localhost");

        assert!(decision.allowed);
        assert!(decision.audit.allowed);
        assert_eq!(sandbox.audit_log().len(), 2);
    }
}
