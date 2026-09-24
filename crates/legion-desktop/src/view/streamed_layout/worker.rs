//! Bounded atlas-independent streamed-layout scanner.
//!
//! This module is intentionally not wired into the desktop frame path yet. It
//! owns scan computation only; atlas replay remains in the parent cache.

use super::*;
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const MAX_ADMITTED_JOBS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StreamedWorkerSubmitError {
    AdmissionFull,
    MailboxFull,
    Stopped,
}

pub(super) struct StreamedWorkerJob {
    pub(super) slot: u64,
    pub(super) generation: u64,
    pub(super) key: CacheKey,
    pub(super) source: Arc<dyn DesktopLineSource + Send + Sync>,
    pub(super) metric_snapshot: egui::epaint::text::MetricFontSnapshot,
    pub(super) visible_rows: Range<usize>,
    pub(super) retain_last_tail: bool,
    pub(super) navigation_byte_target: Option<u64>,
}

pub(super) struct StreamedWorkerResult {
    pub(super) slot: u64,
    pub(super) generation: u64,
    pub(super) key: CacheKey,
    pub(super) scan: StreamedScanState,
    pub(super) eof: bool,
    pub(super) error: Option<String>,
}

struct WorkerTask {
    job: StreamedWorkerJob,
    cancelled: Arc<AtomicBool>,
}

enum WorkerCommand {
    Submit(Box<WorkerTask>),
    Cancel { slot: u64, generation: u64 },
    Shutdown,
}

struct WorkerState {
    job: StreamedWorkerJob,
    cancelled: Arc<AtomicBool>,
    engine: egui::epaint::text::MetricFontEngine,
    scan: StreamedScanState,
}

struct RegistrySlot {
    generation: u64,
    cancelled: Arc<AtomicBool>,
    result: Option<StreamedWorkerResult>,
}

struct WorkerRegistry {
    slots: HashMap<u64, RegistrySlot>,
}

pub(super) struct StreamedWorkerHandle {
    slot: u64,
    generation: u64,
    cancelled: Arc<AtomicBool>,
    command_tx: SyncSender<WorkerCommand>,
    registry: Arc<Mutex<WorkerRegistry>>,
}

impl StreamedWorkerHandle {
    pub(super) fn slot(&self) -> u64 {
        self.slot
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Ok(mut registry) = self.registry.lock() {
            registry.slots.remove(&self.slot);
        }
        let _ = self.command_tx.try_send(WorkerCommand::Cancel {
            slot: self.slot,
            generation: self.generation,
        });
    }
}

impl Drop for StreamedWorkerHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub(super) struct StreamedWorkerScheduler {
    command_tx: SyncSender<WorkerCommand>,
    registry: Arc<Mutex<WorkerRegistry>>,
    next_slot: AtomicU64,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl fmt::Debug for StreamedWorkerHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamedWorkerHandle")
            .field("slot", &self.slot)
            .field("generation", &self.generation)
            .finish()
    }
}

impl fmt::Debug for StreamedWorkerScheduler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamedWorkerScheduler")
            .field("stopped", &self.stop.load(Ordering::Acquire))
            .finish()
    }
}

impl StreamedWorkerScheduler {
    pub(super) fn new() -> Self {
        let (command_tx, command_rx) = mpsc::sync_channel(MAX_ADMITTED_JOBS);
        let registry = Arc::new(Mutex::new(WorkerRegistry {
            slots: HashMap::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_registry = Arc::clone(&registry);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("legion-streamed-layout".to_owned())
            .spawn(move || worker_loop(command_rx, worker_registry, worker_stop))
            .expect("streamed layout worker thread must start");
        Self {
            command_tx,
            registry,
            next_slot: AtomicU64::new(1),
            stop,
            worker: Some(worker),
        }
    }

    #[cfg(test)]
    pub(super) fn stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop)
    }

    pub(super) fn submit(
        &self,
        mut job: StreamedWorkerJob,
    ) -> Result<StreamedWorkerHandle, StreamedWorkerSubmitError> {
        if self.stop.load(Ordering::Acquire) {
            return Err(StreamedWorkerSubmitError::Stopped);
        }
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| StreamedWorkerSubmitError::Stopped)?;
        if registry.slots.len() >= MAX_ADMITTED_JOBS {
            return Err(StreamedWorkerSubmitError::AdmissionFull);
        }
        let slot = (0..=MAX_ADMITTED_JOBS).find_map(|_| {
            let candidate = self
                .next_slot
                .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |current| {
                    if current == 0 {
                        None
                    } else {
                        Some(current.checked_add(1).unwrap_or(0))
                    }
                })
                .ok()?;
            (candidate != 0 && !registry.slots.contains_key(&candidate)).then_some(candidate)
        });
        let Some(slot) = slot else {
            return Err(StreamedWorkerSubmitError::AdmissionFull);
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        job.slot = slot;
        let generation = job.generation;
        registry.slots.insert(
            slot,
            RegistrySlot {
                generation,
                cancelled: Arc::clone(&cancelled),
                result: None,
            },
        );
        drop(registry);
        if self
            .command_tx
            .try_send(WorkerCommand::Submit(Box::new(WorkerTask {
                job,
                cancelled: Arc::clone(&cancelled),
            })))
            .is_err()
        {
            if let Ok(mut registry) = self.registry.lock() {
                registry.slots.remove(&slot);
            }
            return Err(StreamedWorkerSubmitError::MailboxFull);
        }
        Ok(StreamedWorkerHandle {
            slot,
            generation,
            cancelled,
            command_tx: self.command_tx.clone(),
            registry: Arc::clone(&self.registry),
        })
    }

    pub(super) fn drain_results(&self) -> Vec<StreamedWorkerResult> {
        let mut registry = match self.registry.lock() {
            Ok(registry) => registry,
            Err(_) => return Vec::new(),
        };
        let mut results = Vec::new();
        let terminal_slots = registry
            .slots
            .iter_mut()
            .filter_map(|(slot, entry)| entry.result.take().map(|result| (*slot, result)))
            .collect::<Vec<_>>();
        for (slot, result) in terminal_slots {
            if registry.slots.get(&slot).is_some_and(|entry| {
                entry.generation == result.generation && !entry.cancelled.load(Ordering::Acquire)
            }) {
                results.push(result);
            }
        }
        results
    }
}

impl Drop for StreamedWorkerScheduler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(registry) = self.registry.lock() {
            for entry in registry.slots.values() {
                entry.cancelled.store(true, Ordering::Release);
            }
        }
        let _ = self.command_tx.try_send(WorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn make_worker_state(task: WorkerTask) -> WorkerState {
    let scan = StreamedScanState {
        phase: Some(ScanPhase::Metrics),
        metric_next_byte: task.job.key.identity.line_start_byte,
        row_next_byte: task.job.key.identity.line_start_byte,
        visible_rows: Some(task.job.visible_rows.clone()),
        navigation_byte_target: task.job.navigation_byte_target,
        ..StreamedScanState::default()
    };
    let engine = task.job.metric_snapshot.create_engine();
    WorkerState {
        job: task.job,
        cancelled: task.cancelled,
        engine,
        scan,
    }
}

fn prune_cancelled(states: &mut HashMap<u64, WorkerState>, order: &mut VecDeque<u64>) {
    states.retain(|_, state| !state.cancelled.load(Ordering::Acquire));
    order.retain(|slot| states.contains_key(slot));
}

fn worker_loop(
    command_rx: Receiver<WorkerCommand>,
    registry: Arc<Mutex<WorkerRegistry>>,
    stop: Arc<AtomicBool>,
) {
    let mut states = HashMap::<u64, WorkerState>::new();
    let mut order = VecDeque::<u64>::new();
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        for _ in 0..8 {
            match command_rx.try_recv() {
                Ok(WorkerCommand::Submit(task)) => {
                    prune_cancelled(&mut states, &mut order);
                    let state = make_worker_state(*task);
                    let slot = state.job.slot;
                    order.push_back(slot);
                    states.insert(slot, state);
                }
                Ok(WorkerCommand::Cancel { slot, generation }) => {
                    if states
                        .get(&slot)
                        .is_some_and(|state| state.job.generation == generation)
                    {
                        states.remove(&slot);
                        order.retain(|candidate| *candidate != slot);
                    }
                }
                Ok(WorkerCommand::Shutdown) => return,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        if states.is_empty() {
            if stop.load(Ordering::Acquire) {
                return;
            }
            match command_rx.recv_timeout(Duration::from_millis(2)) {
                Ok(WorkerCommand::Submit(task)) => {
                    prune_cancelled(&mut states, &mut order);
                    let state = make_worker_state(*task);
                    let slot = state.job.slot;
                    order.push_back(slot);
                    states.insert(slot, state);
                }
                Ok(WorkerCommand::Cancel { .. }) => {}
                Ok(WorkerCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
            }
        }
        let Some(slot) = order.pop_front() else {
            continue;
        };
        let Some(mut state) = states.remove(&slot) else {
            continue;
        };
        if state.cancelled.load(Ordering::Acquire) {
            continue;
        }
        let error = advance_one_quantum(&mut state);
        let failed = error.is_some();
        if !state.cancelled.load(Ordering::Acquire) {
            publish_result(&registry, &state, error);
        }
        let keep_advancing = !state.cancelled.load(Ordering::Acquire)
            && !failed
            && state.scan.phase != Some(ScanPhase::Ready);
        if keep_advancing {
            order.push_back(slot);
            states.insert(slot, state);
        }
    }
}

fn advance_one_quantum(state: &mut WorkerState) -> Option<String> {
    if state.job.source.identity(state.job.key.identity.line) != state.job.key.identity {
        return Some("worker source identity changed".to_owned());
    }
    if state.job.source.read_lease_id() != state.job.key.read_lease_id {
        return Some("worker source read lease changed".to_owned());
    }
    if state.job.metric_snapshot.pixels_per_point().to_bits() != state.job.key.pixels_per_point_bits
    {
        return Some("worker font scale changed".to_owned());
    }
    let mut source_budget = SOURCE_CHUNK_BYTES;
    let mut row_budget = MAX_FRAME_ROWS;
    let mut glyph_budget = MAX_FRAME_GLYPHS;
    let identity = state.job.key.identity;
    let result = match state.scan.phase {
        None | Some(ScanPhase::Metrics) => state.scan.scan_metrics(
            &mut state.engine,
            state.job.source.as_ref(),
            identity,
            &state.job.key.format,
            &mut source_budget,
            &mut glyph_budget,
        ),
        Some(ScanPhase::Rows) => state.scan.scan_rows(
            &mut state.engine,
            state.job.source.as_ref(),
            identity,
            &state.job.key.format,
            f32::from_bits(state.job.key.wrap_width_bits),
            state.job.key.break_anywhere,
            &mut source_budget,
            &mut row_budget,
            &state.job.visible_rows,
            state.job.retain_last_tail,
        ),
        Some(ScanPhase::Ready) => Ok(()),
    };
    result
        .err()
        .map(|error| error.to_string().chars().take(256).collect())
}

fn publish_result(registry: &Mutex<WorkerRegistry>, state: &WorkerState, error: Option<String>) {
    let result = StreamedWorkerResult {
        slot: state.job.slot,
        generation: state.job.generation,
        key: state.job.key.clone(),
        scan: state.scan.clone(),
        eof: state.scan.phase == Some(ScanPhase::Ready),
        error,
    };
    if let Ok(mut registry) = registry.lock()
        && let Some(entry) = registry.slots.get_mut(&result.slot)
        && entry.generation == result.generation
        && !entry.cancelled.load(Ordering::Acquire)
    {
        entry.result = Some(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, RawInput};
    use std::time::Instant;

    /// Ceiling for every condition wait in this module.
    ///
    /// These tests observe a real worker thread, so anything they need from it
    /// must be waited for as a condition rather than assumed to have happened
    /// after a fixed sleep: a loaded shared CI runner can leave that thread
    /// unscheduled for orders of magnitude longer than any constant a developer
    /// host would pick. The budget is deliberately far larger than the work
    /// involved -- a passing run leaves it almost entirely unused -- and every
    /// expiry below is a hard failure that names what was still true.
    const WORKER_WAIT_BUDGET: Duration = Duration::from_secs(30);

    /// Polls `condition` until it holds or [`WORKER_WAIT_BUDGET`] expires.
    ///
    /// Returns whether the condition held; the caller must assert on that, so a
    /// timeout can never be mistaken for success.
    #[must_use]
    fn wait_for(mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + WORKER_WAIT_BUDGET;
        loop {
            if condition() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Submits `make_job()`, retrying only while the bounded command mailbox is
    /// still full, and panics if it is still full after [`WORKER_WAIT_BUDGET`].
    ///
    /// [`StreamedWorkerScheduler::submit`] enforces two independent bounds: the
    /// registry admission count against `MAX_ADMITTED_JOBS`, refused as
    /// [`StreamedWorkerSubmitError::AdmissionFull`], and the separate
    /// `MAX_ADMITTED_JOBS`-deep command channel, refused as
    /// [`StreamedWorkerSubmitError::MailboxFull`].
    /// [`StreamedWorkerHandle::cancel`] frees the first immediately but not the
    /// second: its cancel command is a best-effort `try_send`, and the submit
    /// commands already queued stay in the channel until the worker thread
    /// consumes them. A caller can therefore hold no admitted jobs at all and
    /// still be refused with `MailboxFull`. That is intended backpressure -- the
    /// production caller in the parent module drops both refusals alike and
    /// retries on a later frame -- so a test that needs a submission to land
    /// waits for the mailbox to drain instead of assuming a fixed sleep drained
    /// it. Every other refusal is a real failure and panics immediately.
    fn submit_when_mailbox_accepts(
        scheduler: &StreamedWorkerScheduler,
        mut make_job: impl FnMut() -> StreamedWorkerJob,
        context: &str,
    ) -> StreamedWorkerHandle {
        let deadline = Instant::now() + WORKER_WAIT_BUDGET;
        let mut refusals = 0_u64;
        loop {
            match scheduler.submit(make_job()) {
                Ok(handle) => return handle,
                Err(StreamedWorkerSubmitError::MailboxFull) if Instant::now() < deadline => {
                    refusals += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!(
                    "{context}: scheduler refused submission with {error:?} after {refusals} \
                     mailbox refusals within {:?}",
                    WORKER_WAIT_BUDGET
                ),
            }
        }
    }

    struct WorkerSource {
        identity: DesktopSourceIdentity,
        text: String,
        revoked: Arc<AtomicBool>,
    }

    impl DesktopLineSource for WorkerSource {
        fn identity(&self, _line: usize) -> DesktopSourceIdentity {
            self.identity
        }

        fn read_chunk(
            &self,
            _line: usize,
            start_byte: u64,
            max_bytes: usize,
        ) -> Result<DesktopLineChunk, String> {
            if self.revoked.load(Ordering::Acquire) {
                return Err("revoked test source".to_owned());
            }
            let origin = usize::try_from(self.identity.line_start_byte)
                .map_err(|_| "origin overflow".to_owned())?;
            let start = usize::try_from(start_byte)
                .ok()
                .and_then(|start| start.checked_sub(origin))
                .ok_or_else(|| "overflow".to_owned())?;
            if start > self.text.len() {
                return Err("outside source".to_owned());
            }
            let end = (start + max_bytes.min(self.text.len() - start)).min(self.text.len());
            Ok(DesktopLineChunk {
                start_byte,
                end_byte: origin as u64 + end as u64,
                is_final: end == self.text.len(),
                text: self.text[start..end].to_owned(),
            })
        }
    }

    struct BlockingWorkerSource {
        identity: DesktopSourceIdentity,
        entered: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
    }

    impl DesktopLineSource for BlockingWorkerSource {
        fn identity(&self, _line: usize) -> DesktopSourceIdentity {
            self.identity
        }

        fn read_chunk(
            &self,
            _line: usize,
            start_byte: u64,
            _max_bytes: usize,
        ) -> Result<DesktopLineChunk, String> {
            self.entered.store(true, Ordering::Release);
            while !self.release.load(Ordering::Acquire) && !self.stop.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            if self.stop.load(Ordering::Acquire) && !self.release.load(Ordering::Acquire) {
                return Err("scheduler stopped".to_owned());
            }
            Ok(DesktopLineChunk {
                start_byte,
                end_byte: self.identity.logical_end_byte,
                is_final: true,
                text: "ready".to_owned(),
            })
        }
    }

    fn snapshot_and_key(
        context: &Context,
        identity: DesktopSourceIdentity,
    ) -> (egui::epaint::text::MetricFontSnapshot, CacheKey) {
        let mut snapshot = None;
        let mut epoch = 0;
        let _ = context.run_ui(RawInput::default(), |ui| {
            snapshot = Some(ui.fonts(|fonts| fonts.metric_snapshot()));
            epoch = ui.fonts(|fonts| fonts.layout_identity_epoch());
        });
        (
            snapshot.expect("metric snapshot"),
            CacheKey {
                identity,
                read_lease_id: None,
                format: TextFormat::default(),
                pixels_per_point_bits: 1.0_f32.to_bits(),
                layout_identity_epoch: epoch,
                wrap_width_bits: 80.0_f32.to_bits(),
                break_anywhere: false,
            },
        )
    }

    fn job(
        source: Arc<dyn DesktopLineSource + Send + Sync>,
        snapshot: egui::epaint::text::MetricFontSnapshot,
        key: CacheKey,
        generation: u64,
    ) -> StreamedWorkerJob {
        StreamedWorkerJob {
            slot: 0,
            generation,
            key,
            source,
            metric_snapshot: snapshot,
            visible_rows: 0..MAX_FRAME_ROWS,
            retain_last_tail: false,
            navigation_byte_target: None,
        }
    }

    #[test]
    fn worker_advances_without_ui_pump_and_emits_bounded_scan() {
        let context = Context::default();
        let identity = DesktopSourceIdentity {
            buffer_id: BufferId(700),
            snapshot_id: SnapshotId(701),
            buffer_version: BufferVersion(1),
            line: 0,
            line_start_byte: 8_192,
            logical_end_byte: 108_192,
            source_key: 702,
        };
        let (snapshot, key) = snapshot_and_key(&context, identity);
        let source: Arc<dyn DesktopLineSource + Send + Sync> = Arc::new(WorkerSource {
            identity,
            text: "word ".repeat(20_000),
            revoked: Arc::new(AtomicBool::new(false)),
        });
        let scheduler = StreamedWorkerScheduler::new();
        let handle = scheduler
            .submit(job(source, snapshot, key, 1))
            .expect("worker admission");
        let mut result = None;
        for _ in 0..1_000 {
            if let Some(item) = scheduler.drain_results().into_iter().next() {
                result = Some(item);
                if result.as_ref().is_some_and(|item| item.eof) {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let result = result.expect("worker progressed without frame pumping");
        assert_eq!(result.slot, handle.slot());
        assert!(result.eof, "worker must publish an EOF result");
        assert!(
            result.error.is_none(),
            "worker scan failed: {:?}",
            result.error
        );
        assert!(result.scan.metric_next_byte > 0 || !result.scan.rows.is_empty());
        assert!(
            !result.scan.rows.is_empty(),
            "worker must retain scanned rows"
        );
        assert!(result.scan.rows.len() <= MAX_FRAME_ROWS);
        assert_eq!(result.scan.row_next_byte, identity.logical_end_byte);
        std::thread::sleep(Duration::from_millis(5));
        assert!(scheduler.drain_results().is_empty());
        handle.cancel();
    }

    #[test]
    fn worker_admission_is_bounded_and_cancel_rejects_late_output() {
        let context = Context::default();
        let identity = DesktopSourceIdentity {
            buffer_id: BufferId(710),
            snapshot_id: SnapshotId(711),
            buffer_version: BufferVersion(1),
            line: 0,
            line_start_byte: 0,
            logical_end_byte: 10,
            source_key: 712,
        };
        let (snapshot, key) = snapshot_and_key(&context, identity);
        let revoked = Arc::new(AtomicBool::new(false));
        let source: Arc<dyn DesktopLineSource + Send + Sync> = Arc::new(WorkerSource {
            identity,
            text: "0123456789".to_owned(),
            revoked: Arc::clone(&revoked),
        });
        let scheduler = StreamedWorkerScheduler::new();
        let mut handles = Vec::new();
        for generation in 0..MAX_ADMITTED_JOBS as u64 {
            handles.push(
                scheduler
                    .submit(job(
                        Arc::clone(&source),
                        snapshot.clone(),
                        key.clone(),
                        generation,
                    ))
                    .expect("admitted bounded worker"),
            );
        }
        assert!(matches!(
            scheduler.submit(job(
                Arc::clone(&source),
                snapshot.clone(),
                key.clone(),
                10_000,
            )),
            Err(StreamedWorkerSubmitError::AdmissionFull)
        ));
        revoked.store(true, Ordering::Release);
        for handle in &handles {
            handle.cancel();
        }
        // `cancel` removes the registry slot outright, taking with it any result
        // the worker had already published into it, so this holds the instant
        // the cancels return and needs no barrier. The stronger property -- that
        // a cancelled job publishes nothing *later*, once the worker finally
        // works through the commands that were queued behind the cancels -- is
        // asserted continuously inside the churn loop below, which drains on
        // every poll and requires every result it sees to belong to the one live
        // job. That window covers the whole backlog drain, where the previous
        // fixed 5ms sleep covered only 5ms of it.
        assert!(
            scheduler.drain_results().is_empty(),
            "cancelling every admitted job must leave no drainable result"
        );

        let live_source: Arc<dyn DesktopLineSource + Send + Sync> = Arc::new(WorkerSource {
            identity,
            text: "0123456789".to_owned(),
            revoked: Arc::new(AtomicBool::new(false)),
        });
        for generation in 0..128_u64 {
            // Admission is free here -- every earlier job was cancelled -- but
            // the bounded command mailbox still holds the 64 submits and the
            // cancels queued behind them, and only the worker thread draining it
            // frees a slot. Wait for that observable condition instead of
            // assuming a sleep drained it.
            let handle = submit_when_mailbox_accepts(
                &scheduler,
                || {
                    job(
                        Arc::clone(&live_source),
                        snapshot.clone(),
                        key.clone(),
                        20_000 + generation,
                    )
                },
                "cancellation churn slot must be reusable",
            );
            // The submit above now lands at the first moment the mailbox has a
            // free slot, which can be while the worker is still chewing through
            // the commands queued ahead of it, so the wait for this job's
            // terminal result is sized for a loaded runner rather than for the
            // one second the previous fixed iteration count allowed.
            let mut terminal = None;
            let published = wait_for(|| {
                let drained = scheduler.drain_results();
                assert!(
                    drained.iter().all(|result| result.slot == handle.slot()),
                    "cancelled jobs must publish nothing: expected only slot {}, drained {:?}",
                    handle.slot(),
                    drained.iter().map(|result| result.slot).collect::<Vec<_>>()
                );
                terminal = drained.into_iter().find(|result| {
                    result.slot == handle.slot() && (result.eof || result.error.is_some())
                });
                terminal.is_some()
            });
            assert!(
                published,
                "every churn generation must publish a terminal result within {:?}",
                WORKER_WAIT_BUDGET
            );
            let terminal = terminal.expect("every churn generation must publish a terminal result");
            assert!(terminal.eof, "churn generation must reach EOF");
            assert!(
                terminal.error.is_none(),
                "churn scan failed: {:?}",
                terminal.error
            );
            handle.cancel();
        }
    }

    #[test]
    fn dropping_scheduler_releases_queued_sources_with_live_handles() {
        let context = Context::default();
        let identity = DesktopSourceIdentity {
            buffer_id: BufferId(730),
            snapshot_id: SnapshotId(731),
            buffer_version: BufferVersion(1),
            line: 0,
            line_start_byte: 0,
            logical_end_byte: 5,
            source_key: 732,
        };
        let (snapshot, key) = snapshot_and_key(&context, identity);
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let scheduler = StreamedWorkerScheduler::new();
        let source: Arc<dyn DesktopLineSource + Send + Sync> = Arc::new(BlockingWorkerSource {
            identity,
            entered: Arc::clone(&entered),
            release: Arc::clone(&release),
            stop: scheduler.stop_flag(),
        });
        let source_weak = Arc::downgrade(&source);
        let mut handles = Vec::new();
        for generation in 0..MAX_ADMITTED_JOBS as u64 {
            handles.push(
                scheduler
                    .submit(job(
                        Arc::clone(&source),
                        snapshot.clone(),
                        key.clone(),
                        generation,
                    ))
                    .expect("admit queued shutdown job"),
            );
        }
        for _ in 0..1_000 {
            if entered.load(Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            entered.load(Ordering::Acquire),
            "worker must enter source before drop"
        );
        drop(source);
        drop(scheduler);
        release.store(true, Ordering::Release);
        for _ in 0..1_000 {
            if source_weak.upgrade().is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            source_weak.upgrade().is_none(),
            "scheduler drop must release queued and active source clones"
        );
        drop(handles);
    }

    #[test]
    fn terminal_worker_result_is_published_once_and_releases_source() {
        let context = Context::default();
        let identity = DesktopSourceIdentity {
            buffer_id: BufferId(720),
            snapshot_id: SnapshotId(721),
            buffer_version: BufferVersion(1),
            line: 0,
            line_start_byte: 0,
            logical_end_byte: 5,
            source_key: 722,
        };
        let (snapshot, mut key) = snapshot_and_key(&context, identity);
        key.identity.source_key = key.identity.source_key.saturating_add(1);
        let source: Arc<dyn DesktopLineSource + Send + Sync> = Arc::new(WorkerSource {
            identity,
            text: "hello".to_owned(),
            revoked: Arc::new(AtomicBool::new(false)),
        });
        let source_weak = Arc::downgrade(&source);
        let scheduler = StreamedWorkerScheduler::new();
        let handle = scheduler
            .submit(job(source, snapshot, key, 900))
            .expect("terminal worker admission");
        let mut error_seen = false;
        for _ in 0..100 {
            if let Some(result) = scheduler.drain_results().into_iter().next() {
                error_seen = result.error.is_some();
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(error_seen);
        // The worker drops the failed `WorkerState` -- and with it the last
        // strong reference to the source -- immediately after publishing the
        // terminal result, but "immediately" is the worker thread's next few
        // instructions, and a loaded runner may not schedule them inside any
        // fixed sleep. Wait for the release itself; a failed job is never
        // re-queued, so nothing further can be published while we wait and the
        // drain assertion below keeps its meaning.
        let released = wait_for(|| source_weak.upgrade().is_none());
        assert!(
            scheduler.drain_results().is_empty(),
            "a terminal worker result must be published exactly once"
        );
        assert!(
            released && source_weak.upgrade().is_none(),
            "worker must release the source after its terminal result within {:?}",
            WORKER_WAIT_BUDGET
        );
        handle.cancel();
    }
}
