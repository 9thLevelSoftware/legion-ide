//! Background Git inspection and mutation worker.

use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, SyncSender, TrySendError},
};
use std::thread;

use legion_project::{
    GitHunkStage, GitInspectionError, GitSnapshotOptions, ProjectGitHunk, ProjectGitSnapshot,
    collect_git_snapshot, commit_git_changes, stage_git_hunk, stage_git_path, unstage_git_hunk,
    unstage_git_path,
};
use legion_security::GitRemoteOperation;

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
