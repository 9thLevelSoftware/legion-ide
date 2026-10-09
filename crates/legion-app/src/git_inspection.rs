//! App-owned Git scheduling and its background execution adapter.

use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
};
use std::thread;

use legion_project::{
    GitDiffStrategy, GitHunkStage, GitInspectionError, GitSnapshotOptions, ProjectGitHunk,
    ProjectGitSnapshot, collect_git_snapshot, commit_git_changes, stage_git_hunk, stage_git_path,
    unstage_git_hunk, unstage_git_path,
};
use legion_protocol::TimestampMillis;
use legion_security::GitRemoteOperation;
use legion_ui::{
    GitBlameLineProjection, GitCommitProjection, GitConflictProjection, GitDiffStrategyProjection,
    GitFileProjection, GitHunkProjection, GitHunkStageProjection, GitProjection, GitRefreshState,
    GitWorktreeKindProjection, GitWorktreeProjection,
};

use crate::{AppComposition, AppCompositionError, git_protocol_error};

/// Injected snapshot runner used by deterministic worker tests.
pub type GitInspectionRunner = Arc<
    dyn Fn(
            u64,
            &Path,
            Option<&Path>,
            GitSnapshotOptions,
        ) -> Result<ProjectGitSnapshot, GitInspectionError>
        + Send
        + Sync,
>;

#[derive(Debug, Clone)]
pub enum GitMutateOp {
    Path {
        root: PathBuf,
        path: String,
        stage: bool,
    },
    Hunk {
        root: PathBuf,
        hunk: ProjectGitHunk,
        stage: bool,
    },
    Commit {
        root: PathBuf,
        message: String,
    },
}

impl GitMutateOp {
    fn root(&self) -> &Path {
        match self {
            Self::Path { root, .. } | Self::Hunk { root, .. } | Self::Commit { root, .. } => root,
        }
    }

    fn run(&self) -> Result<(), GitInspectionError> {
        match self {
            Self::Path { root, path, stage } => {
                let repo_root = legion_project::git_repository_root(root)?;
                if *stage {
                    stage_git_path(repo_root, path)
                } else {
                    unstage_git_path(repo_root, path)
                }
            }
            Self::Hunk { root, hunk, stage } => match (*stage, hunk.stage) {
                (true, GitHunkStage::Unstaged) => stage_git_hunk(root, hunk),
                (false, GitHunkStage::Staged) => unstage_git_hunk(root, hunk),
                _ => Err(GitInspectionError::InvalidInput(
                    "git hunk stage changed before the mutation ran".to_string(),
                )),
            },
            Self::Commit { root, message } => commit_git_changes(root, message).map(|_| ()),
        }
    }
}

#[derive(Debug, Clone)]
enum GitWorkRequest {
    Snapshot {
        generation: u64,
        root: PathBuf,
        active_file: Option<PathBuf>,
        options: GitSnapshotOptions,
    },
    Mutate {
        generation: u64,
        operation: GitMutateOp,
        active_file: Option<PathBuf>,
        options: GitSnapshotOptions,
    },
    Remote {
        generation: u64,
        root: PathBuf,
        operation: GitRemoteOperation,
        remote: String,
        branch: String,
        active_file: Option<PathBuf>,
        options: GitSnapshotOptions,
    },
}

#[derive(Debug)]
enum GitWorkResult {
    SnapshotReady {
        generation: u64,
        snapshot: ProjectGitSnapshot,
    },
    MutateReady {
        generation: u64,
        snapshot: ProjectGitSnapshot,
    },
    Failed {
        generation: u64,
        diagnostic: String,
    },
}

struct GitWorker {
    request_tx: SyncSender<GitWorkRequest>,
    result_rx: Receiver<GitWorkResult>,
}

impl GitWorker {
    fn new() -> Self {
        Self::new_with_runner(Arc::new(|_, root, active_file, options| {
            collect_git_snapshot(root, active_file, options)
        }))
    }

    fn new_with_runner(runner: GitInspectionRunner) -> Self {
        let (request_tx, request_rx) = mpsc::sync_channel::<GitWorkRequest>(1);
        let (result_tx, result_rx) = mpsc::sync_channel::<GitWorkResult>(4);
        // Failed thread creation drops the channel endpoints. The workflow reports
        // Unavailable just as it does for an unexpectedly disconnected worker.
        let _ = thread::Builder::new()
            .name("legion-git-inspection".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let generation = request_generation(&request);
                    let result = match request {
                        GitWorkRequest::Snapshot {
                            generation,
                            root,
                            active_file,
                            options,
                        } => runner(generation, &root, active_file.as_deref(), options).map(
                            |snapshot| GitWorkResult::SnapshotReady {
                                generation,
                                snapshot,
                            },
                        ),
                        GitWorkRequest::Mutate {
                            generation,
                            operation,
                            active_file,
                            options,
                        } => operation
                            .run()
                            .and_then(|()| {
                                runner(
                                    generation,
                                    operation.root(),
                                    active_file.as_deref(),
                                    options,
                                )
                            })
                            .map(|snapshot| GitWorkResult::MutateReady {
                                generation,
                                snapshot,
                            }),
                        GitWorkRequest::Remote {
                            generation,
                            root,
                            operation,
                            remote,
                            branch,
                            active_file,
                            options,
                        } => run_remote(operation, &root, &remote, &branch)
                            .and_then(|()| {
                                runner(generation, &root, active_file.as_deref(), options)
                            })
                            .map(|snapshot| GitWorkResult::MutateReady {
                                generation,
                                snapshot,
                            }),
                    }
                    .unwrap_or_else(|error| GitWorkResult::Failed {
                        generation,
                        diagnostic: error.to_string(),
                    });
                    if result_tx.send(result).is_err() {
                        break;
                    }
                }
            });
        Self {
            request_tx,
            result_rx,
        }
    }

    fn try_recv(&self) -> Result<GitWorkResult, TryRecvError> {
        self.result_rx.try_recv()
    }
}

fn request_generation(request: &GitWorkRequest) -> u64 {
    match request {
        GitWorkRequest::Snapshot { generation, .. }
        | GitWorkRequest::Mutate { generation, .. }
        | GitWorkRequest::Remote { generation, .. } => *generation,
    }
}

fn run_remote(
    operation: GitRemoteOperation,
    root: &Path,
    remote: &str,
    branch: &str,
) -> Result<(), GitInspectionError> {
    let resolved_branch = if branch.is_empty() {
        legion_project::git_current_branch(root)?
    } else {
        branch.to_string()
    };
    match operation {
        GitRemoteOperation::Push => {
            legion_project::push_git_remote(root, remote, &resolved_branch).map(|_| ())
        }
        GitRemoteOperation::Fetch => legion_project::fetch_git_remote(root, remote).map(|_| ()),
        GitRemoteOperation::Pull => {
            legion_project::pull_git_remote(root, remote, &resolved_branch).map(|_| ())
        }
    }
}

impl Default for GitWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Eq)]
enum GitScheduleResult {
    /// Owned by the workflow, either dispatched or retained for dispatch.
    Accepted,
    /// A mutation is already queued. The caller's operation was not accepted.
    Busy,
    /// The worker is gone. No automatic retry is safe for mutations.
    Unavailable,
}

enum GitMutation {
    Local(GitMutateOp),
    Remote {
        root: PathBuf,
        operation: GitRemoteOperation,
        remote: String,
        branch: String,
    },
}

impl GitMutation {
    fn root(&self) -> &Path {
        match self {
            Self::Local(operation) => operation.root(),
            Self::Remote { root, .. } => root,
        }
    }
}

struct GitInFlight {
    generation: u64,
    mutation: bool,
}

/// Scheduling authority for app-approved Git work. The channel adapter owns no
/// scheduling state; policy and projection overlays remain in AppComposition.
pub(crate) struct GitWorkflow {
    worker: GitWorker,
    in_flight: Option<GitInFlight>,
    requested_generation: u64,
    applied_generation: u64,
    pending_mutation: Option<GitWorkRequest>,
    latest_refresh: Option<GitWorkRequest>,
    latest_workspace_root: Option<PathBuf>,
    unavailable: bool,
    terminal_diagnostic: Option<String>,
}

impl GitWorkflow {
    pub(crate) fn new() -> Self {
        Self::with_worker(GitWorker::new())
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub(crate) fn new_with_runner(runner: GitInspectionRunner) -> Self {
        Self::with_worker(GitWorker::new_with_runner(runner))
    }

    fn with_worker(worker: GitWorker) -> Self {
        Self {
            worker,
            in_flight: None,
            requested_generation: 0,
            applied_generation: 0,
            pending_mutation: None,
            latest_refresh: None,
            latest_workspace_root: None,
            unavailable: false,
            terminal_diagnostic: None,
        }
    }

    fn refresh(&mut self, root: PathBuf, active_file: Option<PathBuf>) -> GitScheduleResult {
        if self.unavailable {
            return GitScheduleResult::Unavailable;
        }
        self.requested_generation = self.requested_generation.saturating_add(1);
        self.latest_workspace_root = Some(root.clone());
        self.latest_refresh = Some(GitWorkRequest::Snapshot {
            generation: self.requested_generation,
            root,
            active_file,
            options: GitSnapshotOptions::default(),
        });
        self.dispatch_next();
        self.schedule_result()
    }

    fn enqueue(
        &mut self,
        mutation: GitMutation,
        active_file: Option<PathBuf>,
    ) -> GitScheduleResult {
        if self.unavailable {
            return GitScheduleResult::Unavailable;
        }
        if self.pending_mutation.is_some() {
            return GitScheduleResult::Busy;
        }
        self.requested_generation = self.requested_generation.saturating_add(1);
        self.latest_workspace_root = Some(mutation.root().to_path_buf());
        let generation = self.requested_generation;
        let options = GitSnapshotOptions::default();
        self.pending_mutation = Some(match mutation {
            GitMutation::Local(operation) => GitWorkRequest::Mutate {
                generation,
                operation,
                active_file,
                options,
            },
            GitMutation::Remote {
                root,
                operation,
                remote,
                branch,
            } => GitWorkRequest::Remote {
                generation,
                root,
                operation,
                remote,
                branch,
                active_file,
                options,
            },
        });
        // The mutation includes a snapshot, so older refreshes are redundant.
        // Refreshes requested after this mutation remain queued behind it.
        self.latest_refresh = None;
        self.dispatch_next();
        self.schedule_result()
    }

    fn schedule_result(&self) -> GitScheduleResult {
        if self.unavailable {
            GitScheduleResult::Unavailable
        } else {
            GitScheduleResult::Accepted
        }
    }

    fn dispatch_next(&mut self) {
        if self.unavailable || self.in_flight.is_some() {
            return;
        }
        let Some(request) = self
            .pending_mutation
            .take()
            .or_else(|| self.latest_refresh.take())
        else {
            return;
        };
        let generation = request_generation(&request);
        let mutation = !matches!(&request, GitWorkRequest::Snapshot { .. });
        match self.worker.request_tx.try_send(request) {
            Ok(()) => {
                self.in_flight = Some(GitInFlight {
                    generation,
                    mutation,
                })
            }
            Err(TrySendError::Full(request)) => self.restore_request(request),
            Err(TrySendError::Disconnected(request)) => {
                self.restore_request(request);
                self.disconnect();
            }
        }
    }

    fn restore_request(&mut self, request: GitWorkRequest) {
        match &request {
            GitWorkRequest::Snapshot { .. } => self.latest_refresh = Some(request),
            _ => self.pending_mutation = Some(request),
        }
    }

    fn disconnect(&mut self) {
        if self.unavailable {
            return;
        }
        self.unavailable = true;
        let mut diagnostic = "git.worker_unavailable: Git worker disconnected".to_string();
        if let Some(active) = self.in_flight.take() {
            diagnostic.push_str(&format!(
                "; generation {} did not return a result",
                active.generation
            ));
            if active.mutation {
                diagnostic
                    .push_str(" (mutation may have completed; inspect repository before retrying)");
            }
        }
        if let Some(pending) = self.pending_mutation.take() {
            diagnostic.push_str(&format!(
                "; queued mutation generation {} was not executed",
                request_generation(&pending)
            ));
        }
        self.latest_refresh = None;
        self.terminal_diagnostic = Some(diagnostic);
    }

    /// Nonblocking: only matching completions release the single in-flight slot.
    /// Obsolete snapshots are hidden, but failed accepted mutations remain visible.
    fn poll(
        &mut self,
        workspace_root: Option<&Path>,
        active_file: Option<&Path>,
    ) -> Vec<GitWorkResult> {
        // Workspace opening is owned by app composition. Observe its current
        // context before releasing any result, including switches with no explicit
        // RefreshGit intent. Never retarget an already accepted mutation.
        if self.requested_generation > 0
            && !self.unavailable
            && let Some(root) = workspace_root
            && self.latest_workspace_root.as_deref() != Some(root)
        {
            self.refresh(root.to_path_buf(), active_file.map(Path::to_path_buf));
        }
        let mut results = Vec::new();
        if !self.unavailable {
            loop {
                match self.worker.try_recv() {
                    Ok(result) => {
                        let generation = match &result {
                            GitWorkResult::SnapshotReady { generation, .. }
                            | GitWorkResult::MutateReady { generation, .. }
                            | GitWorkResult::Failed { generation, .. } => *generation,
                        };
                        if !self
                            .in_flight
                            .as_ref()
                            .is_some_and(|active| active.generation == generation)
                        {
                            continue;
                        }
                        let active = self.in_flight.take().expect("matched in-flight generation");
                        if generation == self.requested_generation
                            && generation > self.applied_generation
                        {
                            self.applied_generation = generation;
                            results.push(result);
                        } else if active.mutation && matches!(&result, GitWorkResult::Failed { .. })
                        {
                            results.push(result);
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.disconnect();
                        break;
                    }
                }
            }
            self.dispatch_next();
        }
        if let Some(diagnostic) = self.terminal_diagnostic.take() {
            self.applied_generation = self.requested_generation;
            results.push(GitWorkResult::Failed {
                generation: self.requested_generation,
                diagnostic,
            });
        }
        results
    }

    fn is_idle(&self) -> bool {
        self.unavailable
            || (self.in_flight.is_none()
                && self.pending_mutation.is_none()
                && self.latest_refresh.is_none())
    }
}

fn git_projection_from_project(snapshot: ProjectGitSnapshot) -> GitProjection {
    GitProjection {
        root_label: Some(snapshot.root.0),
        hunks_truncated: snapshot.hunks_truncated,
        merge_awaiting_commit: snapshot.merge_awaiting_commit,
        branch_label: snapshot.branch_label,
        head_short: snapshot.head_short,
        remote_url: snapshot.remote_url,
        remote_default_branch: snapshot.remote_default_branch,
        changed_files: snapshot
            .changed_files
            .into_iter()
            .map(|file| GitFileProjection {
                path: file.path,
                status: file.status,
                inserted_lines: file.inserted_lines,
                deleted_lines: file.deleted_lines,
                unstaged_hunk_count: file.unstaged_hunk_count,
                staged_hunk_count: file.staged_hunk_count,
                stageable: file.stageable,
                diff_strategy: git_diff_strategy_projection(file.diff_strategy),
                fallback_reason: file.fallback_reason,
                conflict: file.conflict,
            })
            .collect(),
        hunks: snapshot
            .hunks
            .into_iter()
            .map(|hunk| GitHunkProjection {
                hunk_id: hunk.hunk_id,
                path: hunk.path,
                stage: git_hunk_stage_projection(hunk.stage),
                header: hunk.header,
                old_start: hunk.old_start,
                old_lines: hunk.old_lines,
                new_start: hunk.new_start,
                new_lines: hunk.new_lines,
                added_lines: hunk.added_lines,
                deleted_lines: hunk.deleted_lines,
                submodule_dirty_only: hunk.submodule_dirty_only,
                context: hunk.context,
            })
            .collect(),
        blame_lines: snapshot
            .blame_lines
            .into_iter()
            .map(|line| GitBlameLineProjection {
                path: line.path,
                line_number: line.line_number,
                commit_short: line.commit_short,
                author: line.author,
                summary: line.summary,
                line_preview: line.line_preview,
            })
            .collect(),
        commits: snapshot
            .commits
            .into_iter()
            .map(|commit| GitCommitProjection {
                hash: commit.hash,
                short_hash: commit.short_hash,
                author: commit.author,
                date: commit.date,
                summary: commit.summary,
                parent_count: commit.parent_count,
                refs: commit.refs,
            })
            .collect(),
        conflicts: snapshot
            .conflicts
            .into_iter()
            .map(|conflict| GitConflictProjection {
                path: conflict.path,
                marker_count: conflict.marker_count,
                actions: conflict.actions,
            })
            .collect(),
        worktrees: snapshot
            .worktrees
            .into_iter()
            .map(|worktree| GitWorktreeProjection {
                path: worktree.path,
                branch_label: worktree.branch_label,
                head_short: worktree.head_short,
                kind: match worktree.kind {
                    legion_project::ProjectGitWorktreeKind::Agent => {
                        GitWorktreeKindProjection::Agent
                    }
                    legion_project::ProjectGitWorktreeKind::Manual => {
                        GitWorktreeKindProjection::Manual
                    }
                },
                prunable: worktree.prunable,
            })
            .collect(),
        diagnostics: snapshot.diagnostics,
        generated_at: snapshot.generated_at,
        schema_version: snapshot.schema_version,
        // Navigation state and local history entries are injected at the app layer after build.
        focused_hunk_id: None,
        commit_validation_warnings: Vec::new(),
        commit_validation_errors: Vec::new(),
        local_history_entries: Vec::new(),
        remote_policy_audit: Vec::new(),
        refresh_state: GitRefreshState::Idle,
        stale: false,
    }
}

fn git_diff_strategy_projection(strategy: GitDiffStrategy) -> GitDiffStrategyProjection {
    match strategy {
        GitDiffStrategy::Syntactic => GitDiffStrategyProjection::Syntactic,
        GitDiffStrategy::LineFallback => GitDiffStrategyProjection::LineFallback,
    }
}

fn git_hunk_stage_projection(stage: GitHunkStage) -> GitHunkStageProjection {
    match stage {
        GitHunkStage::Unstaged => GitHunkStageProjection::Unstaged,
        GitHunkStage::Staged => GitHunkStageProjection::Staged,
    }
}

impl AppComposition {
    /// Refresh app-owned git projection data for the active workspace.
    pub fn refresh_git_projection(&mut self) -> GitProjection {
        let Some(root_path) = self.active_documents.workspace_root_path.as_deref() else {
            self.git_hunk_cache.clear();
            self.git_projection = GitProjection {
                diagnostics: vec!["git.workspace_not_open".to_string()],
                generated_at: TimestampMillis::now(),
                worktrees: Vec::new(),
                refresh_state: GitRefreshState::Idle,
                stale: false,
                ..GitProjection::idle()
            };
            return self.git_projection.clone();
        };
        let active_file = self
            .active_documents
            .active_file_path
            .as_deref()
            .map(PathBuf::from);
        let scheduled = self
            .git_workflow
            .refresh(PathBuf::from(root_path), active_file);
        if scheduled == GitScheduleResult::Accepted {
            self.git_projection.refresh_state = GitRefreshState::Refreshing;
            self.git_projection.stale = true;
        }
        self.drain_git_inspection();
        if scheduled == GitScheduleResult::Unavailable {
            self.mark_git_worker_unavailable();
        }
        self.sync_git_projection_overlay();
        self.git_projection.clone()
    }

    pub(crate) fn enqueue_git_mutation(
        &mut self,
        operation: GitMutateOp,
    ) -> Result<GitProjection, AppCompositionError> {
        self.enqueue_git_operation(GitMutation::Local(operation))
    }

    pub(crate) fn enqueue_git_remote(
        &mut self,
        operation: GitRemoteOperation,
        remote: String,
        branch: String,
    ) -> Result<GitProjection, AppCompositionError> {
        let Some(root_path) = self.active_documents.workspace_root_path.as_deref() else {
            return Err(AppCompositionError::WorkspaceNotOpen);
        };
        self.enqueue_git_operation(GitMutation::Remote {
            root: PathBuf::from(root_path),
            operation,
            remote,
            branch,
        })
    }

    fn enqueue_git_operation(
        &mut self,
        mutation: GitMutation,
    ) -> Result<GitProjection, AppCompositionError> {
        let active_file = self
            .active_documents
            .active_file_path
            .as_deref()
            .map(PathBuf::from);
        match self.git_workflow.enqueue(mutation, active_file) {
            GitScheduleResult::Accepted => {}
            GitScheduleResult::Busy => {
                return Err(git_protocol_error(
                    "git_mutation_pending",
                    "another Git mutation is already waiting for the worker",
                ));
            }
            GitScheduleResult::Unavailable => {
                self.drain_git_inspection();
                self.mark_git_worker_unavailable();
                return Err(git_protocol_error(
                    "git_worker_unavailable",
                    "Git worker is unavailable; the operation was not accepted",
                ));
            }
        }
        self.git_projection.refresh_state = GitRefreshState::Refreshing;
        self.git_projection.stale = true;
        self.sync_git_projection_overlay();
        Ok(self.git_projection.clone())
    }

    /// Apply completed Git worker results without blocking.
    pub fn drain_git_inspection(&mut self) -> bool {
        let mut applied = false;
        for result in self.git_workflow.poll(
            self.active_documents
                .workspace_root_path
                .as_deref()
                .map(Path::new),
            self.active_documents
                .active_file_path
                .as_deref()
                .map(Path::new),
        ) {
            let (snapshot, diagnostic) = match result {
                GitWorkResult::SnapshotReady { snapshot, .. }
                | GitWorkResult::MutateReady { snapshot, .. } => (Some(snapshot), None),
                GitWorkResult::Failed { diagnostic, .. } => (None, Some(diagnostic)),
            };
            applied = true;
            if let Some(snapshot) = snapshot {
                self.git_hunk_cache = snapshot
                    .hunks
                    .iter()
                    .map(|hunk| (hunk.hunk_id.clone(), hunk.clone()))
                    .collect();
                self.git_projection = git_projection_from_project(snapshot);
                self.git_projection.refresh_state = GitRefreshState::Idle;
                self.git_projection.stale = false;
            } else if let Some(message) = diagnostic {
                self.git_hunk_cache.clear();
                let state = if message.to_ascii_lowercase().contains("authentication")
                    || message.to_ascii_lowercase().contains("terminal prompts")
                {
                    GitRefreshState::AuthRequired
                } else if message.to_ascii_lowercase().contains("timed out") {
                    GitRefreshState::TimedOut
                } else {
                    GitRefreshState::Failed
                };
                self.git_projection.refresh_state = state;
                self.git_projection.stale = false;
                self.git_projection
                    .diagnostics
                    .push(format!("git.refresh_failed: {message}"));
            }
        }
        if !self.git_workflow.is_idle() {
            self.git_projection.refresh_state = GitRefreshState::Refreshing;
            self.git_projection.stale = true;
        }
        self.sync_git_projection_overlay();
        applied
    }

    /// Drain Git worker results until no accepted job remains.
    pub fn drain_git_until_idle(&mut self) -> GitProjection {
        // Observe a possible workspace switch even when the previous context is idle.
        self.drain_git_inspection();
        while !self.git_workflow.is_idle() {
            self.drain_git_inspection();
            if !self.git_workflow.is_idle() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        self.drain_git_inspection();
        self.git_projection.clone()
    }

    fn mark_git_worker_unavailable(&mut self) {
        self.git_projection.refresh_state = GitRefreshState::Failed;
        self.git_projection.stale = false;
        self.git_hunk_cache.clear();
        if !self
            .git_projection
            .diagnostics
            .iter()
            .any(|message| message.contains("git.worker_unavailable"))
        {
            self.git_projection
                .diagnostics
                .push("git.worker_unavailable: Git worker is unavailable".to_string());
        }
    }

    fn sync_git_projection_overlay(&mut self) {
        self.git_projection.focused_hunk_id = self.focused_git_hunk_id.clone();
        self.git_projection.remote_policy_audit = self.git_remote_policy_audit.clone();
        self.git_projection
            .diagnostics
            .retain(|d| !d.starts_with("local_history.write_degraded:"));
        if let Some(ref err) = self.local_history_last_write_error {
            self.git_projection
                .diagnostics
                .push(format!("local_history.write_degraded: {err}"));
        }
    }

    /// Navigate to the next or previous hunk in the diff review surface.
    ///
    /// `forward` — true = next, false = prev.
    /// `by_file` — true = jump to first hunk of next/prev file, false = adjacent hunk.
    pub(crate) fn navigate_git_hunk(&mut self, forward: bool, by_file: bool) -> GitProjection {
        let hunks = &self.git_projection.hunks;
        if hunks.is_empty() {
            return self.git_projection.clone();
        }

        let current_idx = self
            .focused_git_hunk_id
            .as_deref()
            .and_then(|id| hunks.iter().position(|h| h.hunk_id == id));

        let new_id = if by_file {
            // Jump to the first hunk of the next/prev file.
            let current_path = current_idx
                .and_then(|i| hunks.get(i))
                .map(|h| h.path.as_str());
            if forward {
                // Find the first hunk whose path differs and comes after current.
                let start = current_idx.map(|i| i + 1).unwrap_or(0);
                hunks[start..]
                    .iter()
                    .find(|h| current_path.is_none_or(|p| h.path != p))
                    .map(|h| h.hunk_id.clone())
                    .or_else(|| hunks.first().map(|h| h.hunk_id.clone()))
            } else {
                // Find the last hunk whose path differs and comes before current.
                let end = current_idx.unwrap_or(hunks.len());
                hunks[..end]
                    .iter()
                    .rev()
                    .find(|h| current_path.is_none_or(|p| h.path != p))
                    .and_then(|h| {
                        // Jump to the *first* hunk of that file.
                        let target_path = h.path.clone();
                        hunks.iter().find(|hh| hh.path == target_path)
                    })
                    .map(|h| h.hunk_id.clone())
                    .or_else(|| hunks.last().map(|h| h.hunk_id.clone()))
            }
        } else if forward {
            let next_idx = current_idx.map(|i| (i + 1) % hunks.len()).unwrap_or(0);
            hunks.get(next_idx).map(|h| h.hunk_id.clone())
        } else {
            let prev_idx = current_idx
                .map(|i| if i == 0 { hunks.len() - 1 } else { i - 1 })
                .unwrap_or_else(|| hunks.len() - 1);
            hunks.get(prev_idx).map(|h| h.hunk_id.clone())
        };

        self.focused_git_hunk_id = new_id;
        self.git_projection.focused_hunk_id = self.focused_git_hunk_id.clone();
        self.git_projection.clone()
    }
}

#[cfg(test)]
mod scheduling_tests {
    use super::*;

    // Exercise the scheduling interface with real bounded channels, without
    // filesystem side effects or timing-dependent background threads.
    fn workflow_channels() -> (
        GitWorkflow,
        Receiver<GitWorkRequest>,
        SyncSender<GitWorkResult>,
    ) {
        let (request_tx, request_rx) = mpsc::sync_channel(1);
        let (result_tx, result_rx) = mpsc::sync_channel(4);
        (
            GitWorkflow::with_worker(GitWorker {
                request_tx,
                result_rx,
            }),
            request_rx,
            result_tx,
        )
    }

    fn local_mutation() -> GitMutation {
        GitMutation::Local(GitMutateOp::Path {
            root: PathBuf::from("repo"),
            path: "file.rs".to_string(),
            stage: true,
        })
    }

    fn complete(result_tx: &SyncSender<GitWorkResult>, generation: u64) {
        result_tx
            .send(GitWorkResult::Failed {
                generation,
                diagnostic: format!("generation {generation} failed"),
            })
            .expect("send completion");
    }

    fn poll(workflow: &mut GitWorkflow) -> Vec<GitWorkResult> {
        let root = workflow.latest_workspace_root.clone();
        workflow.poll(root.as_deref(), None)
    }

    #[test]
    fn pending_mutation_precedes_latest_refresh_and_busy_preserves_order() {
        // Local and remote mutations share exactly the same queue guarantees.
        for remote in [false, true] {
            let (mut workflow, requests, results) = workflow_channels();
            assert_eq!(
                workflow.refresh("repo".into(), None),
                GitScheduleResult::Accepted
            );
            assert_eq!(
                request_generation(&requests.try_recv().expect("first refresh")),
                1
            );
            assert_eq!(
                workflow.refresh("obsolete".into(), None),
                GitScheduleResult::Accepted
            );
            let mutation = if remote {
                GitMutation::Remote {
                    root: "repo".into(),
                    operation: GitRemoteOperation::Fetch,
                    remote: "origin".into(),
                    branch: String::new(),
                }
            } else {
                local_mutation()
            };
            assert_eq!(
                workflow.enqueue(mutation, None),
                GitScheduleResult::Accepted
            );
            assert_eq!(
                workflow.enqueue(local_mutation(), None),
                GitScheduleResult::Busy
            );
            assert_eq!(
                workflow.refresh("newest".into(), Some("active.rs".into())),
                GitScheduleResult::Accepted
            );
            assert_eq!(
                workflow.refresh("latest".into(), Some("latest.rs".into())),
                GitScheduleResult::Accepted
            );
            assert!(matches!(requests.try_recv(), Err(TryRecvError::Empty)));

            complete(&results, 1);
            assert!(
                poll(&mut workflow).is_empty(),
                "obsolete refresh is not published"
            );
            let mutation = requests.try_recv().expect("queued mutation");
            assert_eq!(
                request_generation(&mutation),
                3,
                "Busy must not consume a generation"
            );
            if remote {
                assert!(
                    matches!(mutation, GitWorkRequest::Remote { remote, .. } if remote == "origin")
                );
            } else {
                assert!(
                    matches!(mutation, GitWorkRequest::Mutate { operation: GitMutateOp::Path { path, stage: true, .. }, .. } if path == "file.rs")
                );
            }

            complete(&results, 3);
            assert!(
                matches!(
                    poll(&mut workflow).as_slice(),
                    [GitWorkResult::Failed { generation: 3, .. }]
                ),
                "a failed accepted mutation must remain visible despite a newer refresh"
            );
            assert!(
                matches!(requests.try_recv().expect("latest refresh"), GitWorkRequest::Snapshot { generation: 5, root, active_file: Some(active_file), .. } if root == Path::new("latest") && active_file == Path::new("latest.rs"))
            );
            complete(&results, 5);
            assert!(matches!(
                poll(&mut workflow).as_slice(),
                [GitWorkResult::Failed { generation: 5, .. }]
            ));
            assert!(workflow.is_idle());
            assert!(poll(&mut workflow).is_empty());
            assert!(matches!(requests.try_recv(), Err(TryRecvError::Empty)));
        }
    }

    #[test]
    fn newer_mutation_snapshot_absorbs_older_queued_refresh() {
        let (mut workflow, requests, results) = workflow_channels();
        assert_eq!(
            workflow.refresh("repo".into(), None),
            GitScheduleResult::Accepted
        );
        requests.try_recv().expect("initial refresh");
        assert_eq!(
            workflow.refresh("repo".into(), None),
            GitScheduleResult::Accepted
        );
        assert_eq!(
            workflow.enqueue(local_mutation(), None),
            GitScheduleResult::Accepted
        );
        complete(&results, 1);
        assert!(poll(&mut workflow).is_empty());
        assert!(matches!(
            requests.try_recv().expect("mutation"),
            GitWorkRequest::Mutate { generation: 3, .. }
        ));
        complete(&results, 3);
        assert!(matches!(
            poll(&mut workflow).as_slice(),
            [GitWorkResult::Failed { generation: 3, .. }]
        ));
        assert!(workflow.is_idle());
        assert!(matches!(requests.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn backpressure_retains_accepted_mutation_before_latest_refresh() {
        let (mut workflow, requests, results) = workflow_channels();
        // Occupy the adapter slot to force Full independently of scheduling state.
        workflow
            .worker
            .request_tx
            .try_send(GitWorkRequest::Snapshot {
                generation: 99,
                root: "fixture".into(),
                active_file: None,
                options: GitSnapshotOptions::default(),
            })
            .expect("fill channel");
        assert_eq!(
            workflow.enqueue(local_mutation(), None),
            GitScheduleResult::Accepted
        );
        assert_eq!(
            workflow.enqueue(local_mutation(), None),
            GitScheduleResult::Busy
        );
        assert_eq!(
            workflow.refresh("repo".into(), None),
            GitScheduleResult::Accepted
        );
        assert!(
            !workflow.is_idle(),
            "queued work is not idle even before dispatch"
        );
        assert!(poll(&mut workflow).is_empty());
        assert_eq!(
            request_generation(&requests.try_recv().expect("fixture")),
            99
        );
        assert!(poll(&mut workflow).is_empty());
        assert!(matches!(
            requests.try_recv().expect("retained mutation"),
            GitWorkRequest::Mutate { generation: 1, .. }
        ));
        complete(&results, 1);
        assert_eq!(poll(&mut workflow).len(), 1);
        assert!(matches!(
            requests.try_recv().expect("refresh after mutation"),
            GitWorkRequest::Snapshot { generation: 2, .. }
        ));
        complete(&results, 2);
        assert_eq!(poll(&mut workflow).len(), 1);
        assert!(workflow.is_idle());
    }

    #[test]
    fn unmatched_result_cannot_release_in_flight_authority() {
        let (mut workflow, requests, results) = workflow_channels();
        workflow.refresh("repo".into(), None);
        requests.try_recv().expect("initial refresh");
        workflow.enqueue(local_mutation(), None);
        complete(&results, 2);
        assert!(poll(&mut workflow).is_empty());
        assert!(matches!(requests.try_recv(), Err(TryRecvError::Empty)));
        complete(&results, 1);
        assert!(poll(&mut workflow).is_empty());
        assert!(matches!(
            requests
                .try_recv()
                .expect("mutation after actual completion"),
            GitWorkRequest::Mutate { generation: 2, .. }
        ));
        complete(&results, 2);
        assert_eq!(poll(&mut workflow).len(), 1);
        assert!(workflow.is_idle());
    }

    #[test]
    fn disconnection_settles_in_flight_and_queued_mutations_once() {
        let (mut workflow, requests, results) = workflow_channels();
        assert_eq!(
            workflow.enqueue(local_mutation(), None),
            GitScheduleResult::Accepted
        );
        requests.try_recv().expect("in-flight mutation");
        assert_eq!(
            workflow.enqueue(local_mutation(), None),
            GitScheduleResult::Accepted
        );
        assert_eq!(
            workflow.refresh("repo".into(), None),
            GitScheduleResult::Accepted
        );
        drop(requests);
        drop(results);
        let failures = poll(&mut workflow);
        assert!(
            matches!(failures.as_slice(), [GitWorkResult::Failed { generation: 3, diagnostic }] if diagnostic.contains("mutation may have completed") && diagnostic.contains("queued mutation generation 2 was not executed"))
        );
        assert!(workflow.is_idle());
        assert_eq!(
            workflow.refresh("repo".into(), None),
            GitScheduleResult::Unavailable
        );
        assert_eq!(
            workflow.enqueue(local_mutation(), None),
            GitScheduleResult::Unavailable
        );
        assert!(
            poll(&mut workflow).is_empty(),
            "terminal failure must not repeat or reschedule"
        );
    }

    #[test]
    fn disconnected_send_reports_unavailable_without_accepting_mutation() {
        let (mut workflow, requests, _results) = workflow_channels();
        drop(requests);
        assert_eq!(
            workflow.enqueue(local_mutation(), None),
            GitScheduleResult::Unavailable
        );
        assert!(
            matches!(poll(&mut workflow).as_slice(), [GitWorkResult::Failed { diagnostic, .. }] if diagnostic.contains("was not executed"))
        );
        assert!(workflow.is_idle());
        assert!(poll(&mut workflow).is_empty());
    }

    #[test]
    fn workspace_switch_keeps_mutation_root_and_refreshes_latest_context() {
        let (mut workflow, requests, results) = workflow_channels();
        workflow.refresh("repo".into(), Some("repo/old.rs".into()));
        requests.try_recv().expect("old-root inspection");
        workflow.enqueue(local_mutation(), Some("repo/old.rs".into()));
        complete(&results, 1);
        assert!(
            workflow
                .poll(Some(Path::new("other")), Some(Path::new("other/new.rs")))
                .is_empty()
        );
        assert!(
            matches!(requests.try_recv().expect("original mutation"), GitWorkRequest::Mutate { generation: 2, operation: GitMutateOp::Path { root, .. }, active_file: Some(active_file), .. } if root == Path::new("repo") && active_file == Path::new("repo/old.rs"))
        );
        complete(&results, 2);
        assert_eq!(
            workflow
                .poll(Some(Path::new("other")), Some(Path::new("other/new.rs")))
                .len(),
            1
        );
        assert!(
            matches!(requests.try_recv().expect("new-root refresh"), GitWorkRequest::Snapshot { generation: 3, root, active_file: Some(active_file), .. } if root == Path::new("other") && active_file == Path::new("other/new.rs"))
        );
        complete(&results, 3);
        assert_eq!(poll(&mut workflow).len(), 1);
        assert!(workflow.is_idle());
    }
}
