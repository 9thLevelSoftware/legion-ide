//! Background Git inspection and mutation worker.

use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, SyncSender, TrySendError},
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
pub enum GitWorkRequest {
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
pub enum GitWorkResult {
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

pub struct GitWorker {
    request_tx: SyncSender<GitWorkRequest>,
    result_rx: Receiver<GitWorkResult>,
    in_flight: bool,
}

impl GitWorker {
    pub fn new() -> Self {
        Self::new_with_runner(Arc::new(|_, root, active_file, options| {
            collect_git_snapshot(root, active_file, options)
        }))
    }

    pub fn new_with_runner(runner: GitInspectionRunner) -> Self {
        let (request_tx, request_rx) = mpsc::sync_channel::<GitWorkRequest>(1);
        let (result_tx, result_rx) = mpsc::sync_channel::<GitWorkResult>(4);
        thread::Builder::new()
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
            })
            .expect("Git inspection worker must spawn");
        Self {
            request_tx,
            result_rx,
            in_flight: false,
        }
    }

    pub fn try_send(&mut self, request: GitWorkRequest) -> bool {
        if self.in_flight {
            return false;
        }
        match self.request_tx.try_send(request) {
            Ok(()) => {
                self.in_flight = true;
                true
            }
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false,
        }
    }

    pub fn drain(&mut self) -> Vec<GitWorkResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            self.in_flight = false;
            results.push(result);
        }
        results
    }

    pub fn is_idle(&self) -> bool {
        !self.in_flight
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

impl Default for GitWorker {
    fn default() -> Self {
        Self::new()
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
        self.git_latest_generation = self.git_latest_generation.saturating_add(1);
        self.git_projection.refresh_state = GitRefreshState::Refreshing;
        self.git_projection.stale = true;
        if !self.git_in_flight && self.pending_mutation.is_none() {
            let request = GitWorkRequest::Snapshot {
                generation: self.git_latest_generation,
                root: PathBuf::from(root_path),
                active_file,
                options: GitSnapshotOptions::default(),
            };
            self.git_in_flight = self.git_worker.try_send(request);
        }
        self.drain_git_inspection();
        self.sync_git_projection_overlay();
        self.git_projection.clone()
    }

    pub(crate) fn enqueue_git_mutation(
        &mut self,
        operation: GitMutateOp,
    ) -> Result<GitProjection, AppCompositionError> {
        if self.pending_mutation.is_some() {
            return Err(git_protocol_error(
                "git_mutation_pending",
                "another Git mutation is already waiting for the worker",
            ));
        }
        let active_file = self
            .active_documents
            .active_file_path
            .as_deref()
            .map(PathBuf::from);
        self.git_latest_generation = self.git_latest_generation.saturating_add(1);
        self.git_projection.refresh_state = GitRefreshState::Refreshing;
        self.git_projection.stale = true;
        let request = GitWorkRequest::Mutate {
            generation: self.git_latest_generation,
            operation,
            active_file,
            options: GitSnapshotOptions::default(),
        };
        if self.git_in_flight {
            self.pending_mutation = Some(request);
        } else {
            self.git_in_flight = self.git_worker.try_send(request);
        }
        self.sync_git_projection_overlay();
        Ok(self.git_projection.clone())
    }

    pub(crate) fn enqueue_git_remote(
        &mut self,
        operation: GitRemoteOperation,
        remote: String,
        branch: String,
    ) -> Result<GitProjection, AppCompositionError> {
        if self.pending_mutation.is_some() {
            return Err(git_protocol_error(
                "git_mutation_pending",
                "another Git mutation is already waiting for the worker",
            ));
        }
        let Some(root_path) = self.active_documents.workspace_root_path.as_deref() else {
            return Err(AppCompositionError::WorkspaceNotOpen);
        };
        let active_file = self
            .active_documents
            .active_file_path
            .as_deref()
            .map(PathBuf::from);
        self.git_latest_generation = self.git_latest_generation.saturating_add(1);
        self.git_projection.refresh_state = GitRefreshState::Refreshing;
        self.git_projection.stale = true;
        let request = GitWorkRequest::Remote {
            generation: self.git_latest_generation,
            root: PathBuf::from(root_path),
            operation,
            remote,
            branch,
            active_file,
            options: GitSnapshotOptions::default(),
        };
        if self.git_in_flight {
            self.pending_mutation = Some(request);
        } else {
            self.git_in_flight = self.git_worker.try_send(request);
        }
        self.sync_git_projection_overlay();
        Ok(self.git_projection.clone())
    }

    /// Apply completed Git worker results without blocking.
    pub fn drain_git_inspection(&mut self) -> bool {
        let mut applied = false;
        let mut received = false;
        for result in self.git_worker.drain() {
            received = true;
            self.git_in_flight = false;
            let (generation, snapshot, diagnostic) = match result {
                GitWorkResult::SnapshotReady {
                    generation,
                    snapshot,
                }
                | GitWorkResult::MutateReady {
                    generation,
                    snapshot,
                } => (generation, Some(snapshot), None),
                GitWorkResult::Failed {
                    generation,
                    diagnostic,
                } => (generation, None, Some(diagnostic)),
            };
            if generation != self.git_latest_generation {
                continue;
            }
            self.git_applied_generation = generation;
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
        if received && !self.git_in_flight {
            if let Some(pending) = self.pending_mutation.take() {
                if self.git_worker.try_send(pending.clone()) {
                    self.git_in_flight = true;
                } else {
                    self.pending_mutation = Some(pending);
                }
            } else if self.git_applied_generation < self.git_latest_generation {
                let root_path = self.active_documents.workspace_root_path.clone();
                if let Some(root_path) = root_path {
                    let request = GitWorkRequest::Snapshot {
                        generation: self.git_latest_generation,
                        root: PathBuf::from(root_path),
                        active_file: self
                            .active_documents
                            .active_file_path
                            .as_deref()
                            .map(PathBuf::from),
                        options: GitSnapshotOptions::default(),
                    };
                    self.git_in_flight = self.git_worker.try_send(request);
                }
            }
        }
        self.sync_git_projection_overlay();
        applied
    }

    /// Drain Git worker results until no accepted job remains.
    pub fn drain_git_until_idle(&mut self) -> GitProjection {
        while !self.git_worker.is_idle() {
            self.drain_git_inspection();
            if !self.git_worker.is_idle() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        self.drain_git_inspection();
        self.git_projection.clone()
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
