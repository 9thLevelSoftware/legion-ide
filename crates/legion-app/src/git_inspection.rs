//! Background Git inspection worker and app-thread drain seam.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread;

use legion_project::{
    GitInspectionError, GitSnapshotOptions, ProjectGitSnapshot, collect_git_snapshot,
};

/// A Git snapshot request issued by the app thread.
#[derive(Debug, Clone)]
pub struct GitInspectionRequest {
    /// Workspace root used for inspection.
    pub root: PathBuf,
    /// Active file used for blame projection.
    pub active_file: Option<PathBuf>,
    /// Snapshot bounds.
    pub options: GitSnapshotOptions,
    /// Monotonic app generation for stale-result rejection.
    pub generation: u64,
}

/// Result returned by the background Git worker.
#[derive(Debug)]
pub struct GitInspectionResult {
    /// Request generation that produced this result.
    pub generation: u64,
    /// Snapshot or failure returned by Git.
    pub result: Result<ProjectGitSnapshot, GitInspectionError>,
}

/// Single-worker Git inspector.
///
/// The app thread owns the latest request and never waits for the worker. A
/// second refresh while a request is running replaces the pending request; the
/// worker receives the replacement only after the current result is drained.
pub struct GitWorker {
    request_tx: SyncSender<GitInspectionRequest>,
    result_rx: Receiver<GitInspectionResult>,
    latest_request: Option<GitInspectionRequest>,
    in_flight: bool,
}

impl GitWorker {
    /// Spawn the worker thread and return its app-thread handle.
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::sync_channel::<GitInspectionRequest>(1);
        let (result_tx, result_rx) = mpsc::sync_channel::<GitInspectionResult>(4);
        thread::Builder::new()
            .name("legion-git-inspection".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let result = collect_git_snapshot(
                        &request.root,
                        request.active_file.as_deref(),
                        request.options,
                    );
                    if result_tx
                        .send(GitInspectionResult {
                            generation: request.generation,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("Git inspection worker must spawn");
        Self {
            request_tx,
            result_rx,
            latest_request: None,
            in_flight: false,
        }
    }

    /// Queue the newest request without blocking the caller.
    pub fn request(&mut self, request: GitInspectionRequest) {
        self.latest_request = Some(request);
        self.try_send_latest();
    }

    /// Drain all currently available results without blocking.
    pub fn drain(&mut self) -> Vec<GitInspectionResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            self.in_flight = false;
            results.push(result);
            self.try_send_latest();
        }
        results
    }

    /// Whether no request is queued or executing.
    pub fn is_idle(&self) -> bool {
        !self.in_flight && self.latest_request.is_none()
    }

    fn try_send_latest(&mut self) {
        if self.in_flight {
            return;
        }
        let Some(request) = self.latest_request.take() else {
            return;
        };
        match self.request_tx.try_send(request) {
            Ok(()) => self.in_flight = true,
            Err(TrySendError::Full(request)) => self.latest_request = Some(request),
            Err(TrySendError::Disconnected(_)) => self.in_flight = false,
        }
    }
}

impl Default for GitWorker {
    fn default() -> Self {
        Self::new()
    }
}
