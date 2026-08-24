//! Git remote operation dispatch for [`AppComposition`] (P2.F5.T2/T4).
//!
//! Moved out of `lib.rs` unchanged. `lib.rs` is the merge chokepoint for this
//! workspace and is already ~38k lines, so the dispatch half of the git remote
//! surface lives here while the pure policy and projection half stays in
//! [`crate::git_policy`].
//!
//! These are `impl AppComposition` continuations rather than free functions
//! because they read and write composition-owned state: the active workspace,
//! the security policy it enforces, and the remote-policy audit trail.

use crate::*;

impl AppComposition {
    /// Run a git operation that contacts a remote, after a policy decision.
    ///
    /// Push, fetch, and pull are the only git verbs that can leave the machine,
    /// so each one is evaluated against the workspace network policy and records
    /// its verdict in the projection before anything is executed (P2.F5.T4). A
    /// denial is *not* an error: the projection carries the reason, so the SCM
    /// surface can show it instead of raising an opaque failure.
    pub(crate) fn dispatch_git_remote_operation(
        &mut self,
        operation: GitRemoteOperation,
        remote: &str,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        let Some(root_path) = self.active_documents.workspace_root_path.as_deref() else {
            return Err(AppCompositionError::WorkspaceNotOpen);
        };
        let root_path = root_path.to_string();

        // A freshly opened workspace may not have a completed snapshot yet;
        // the worker resolves an empty branch label off the app thread.
        let branch = self.git_projection.branch_label.clone().unwrap_or_default();

        let remote_url = legion_project::git_remote_configured_url(Path::new(&root_path), remote);
        let trust = self
            .active_documents
            .active_workspace_trust
            .clone()
            .map_or(TrustState::Unknown, TrustState::from);
        let outcome = git_policy::evaluate(
            &self.workspace.security_policy(),
            trust,
            operation,
            remote,
            remote_url.as_deref(),
        );
        git_policy::record(&mut self.git_remote_policy_audit, outcome.audit);

        if !outcome.allowed {
            // Denials are app-thread decisions: expose the audit row without
            // starting a worker job or synchronously invoking git.exe.
            self.git_projection.remote_policy_audit = self.git_remote_policy_audit.clone();
            return Ok(AppCommandOutcome::GitUpdated(self.git_projection.clone()));
        }
        let projection = self.enqueue_git_remote(operation, remote.to_string(), branch)?;
        Ok(AppCommandOutcome::GitUpdated(projection))
    }

    /// Record or withdraw user consent to reach a host for git remote operations.
    ///
    /// This is the write half of the network gate. It exists so a denial is a
    /// decision the user can act on rather than a wall: an explicit grant is
    /// what turns "policy refuses" into "policy permits, and here is the record
    /// of who permitted it". Consent is refused outside a trusted workspace, so
    /// an untrusted repository cannot grant itself egress.
    pub(crate) fn dispatch_git_remote_consent(
        &mut self,
        host: &str,
        grant: bool,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        if self.active_documents.workspace_root_path.is_none() {
            return Err(AppCompositionError::WorkspaceNotOpen);
        }
        let host = host.trim().to_ascii_lowercase();
        if host.is_empty() {
            return Err(git_protocol_error(
                "git_consent_host_missing",
                "a host is required to grant or revoke git remote consent",
            ));
        }
        if self.active_documents.active_workspace_trust != Some(WorkspaceTrustState::Trusted) {
            return Err(AppCompositionError::WorkspaceNotTrusted(
                "git remote consent denied: workspace is untrusted".to_string(),
            ));
        }

        let changed = if grant {
            self.workspace.consent_git_remote_host(&host)
        } else {
            self.workspace.revoke_git_remote_host(&host)
        };
        git_policy::record(
            &mut self.git_remote_policy_audit,
            git_policy::consent_row(&host, grant, changed),
        );
        Ok(AppCommandOutcome::GitUpdated(self.refresh_git_projection()))
    }
}
