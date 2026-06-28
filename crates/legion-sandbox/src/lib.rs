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
    fn allow(platform: SandboxPlatform, backend: SandboxBackend, action: SandboxAction, reason: impl Into<String>) -> Self {
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

    fn deny(platform: SandboxPlatform, backend: SandboxBackend, action: SandboxAction, reason: impl Into<String>) -> Self {
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxScope {
    /// Workspace root that is writable.
    pub workspace_root: PathBuf,
    /// Allowed egress destinations.
    pub allowed_egress: BTreeSet<String>,
}

impl SandboxScope {
    /// Creates a new scope that only allows workspace-local writes.
    pub fn workspace_only(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            allowed_egress: BTreeSet::new(),
        }
    }

    /// Adds an allowed egress destination.
    pub fn with_egress(mut self, target: impl Into<String>) -> Self {
        self.allowed_egress.insert(target.into());
        self
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
    pub fn activate(platform: SandboxPlatform, backend: SandboxBackend, scope: SandboxScope) -> Self {
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

    #[test]
    fn raw_egress_without_permission_fails_closed_and_audits() {
        let scope = SandboxScope::workspace_only("/workspace/project");
        let mut sandbox = ActivatedSandbox::activate(
            SandboxPlatform::MacOS,
            SandboxBackend::Seatbelt,
            scope,
        );

        let decision = sandbox.authorize_egress("https://example.com");

        assert!(!decision.allowed);
        assert!(matches!(decision.audit.action, SandboxAction::Egress { .. }));
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
