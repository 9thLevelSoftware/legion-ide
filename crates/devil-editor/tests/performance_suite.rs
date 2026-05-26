use std::time::{Duration, Instant};

use devil_editor::{
    BufferMode, EditorEngine, EditorThresholds, SnapshotEvictionPreference,
    SnapshotRetentionPolicy, TextPosition,
};
use devil_protocol::{
    BufferId, CollaborationOperationId, CollaborationParticipantId, CollaborationSessionId,
    EditBatch, EditorApplyTransactionRequest, EditorViewportRequest, FileId, SnapshotConsumerKind,
    TextEdit as ProtocolTextEdit, TextTransactionDescriptor, TransactionSource, ViewportDimensions,
    ViewportProjection, ViewportProjectionMode, ViewportScroll, WorkspaceId,
};
use devil_text::{DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, TextEdit, TextRange};

const CI_LARGE_FILE_BYTES: usize = DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + (128 * 1024);
const LARGE_TEXT_LINE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n";

fn running_in_ci() -> bool {
    std::env::var_os("CI").is_some()
}

fn ci_large_file_open_threshold() -> Duration {
    if cfg!(windows) || running_in_ci() {
        Duration::from_secs(8)
    } else {
        Duration::from_secs(5)
    }
}

fn collaboration_edit_p99_overhead_threshold() -> Duration {
    if cfg!(windows) || running_in_ci() {
        Duration::from_millis(20)
    } else {
        Duration::from_millis(5)
    }
}

fn collaboration_edit_p95_overhead_threshold() -> Duration {
    if cfg!(windows) {
        Duration::from_millis(5)
    } else {
        Duration::from_millis(2)
    }
}

fn percentile(durations: &mut [Duration], pct: f64) -> Duration {
    durations.sort();
    let idx = ((durations.len() as f64 - 1.0) * pct).round() as usize;
    durations[idx]
}

fn deterministic_retention_policy(max_snapshot_count: usize) -> SnapshotRetentionPolicy {
    SnapshotRetentionPolicy {
        max_snapshot_count,
        max_estimated_bytes: usize::MAX,
        eviction_preference: SnapshotEvictionPreference::UndoThenRedo,
    }
}

fn deterministic_large_text(byte_len: usize) -> String {
    let mut text = String::with_capacity(byte_len);
    while text.len() + LARGE_TEXT_LINE.len() <= byte_len {
        text.push_str(LARGE_TEXT_LINE);
    }
    while text.len() < byte_len {
        text.push('z');
    }
    text
}

fn viewport_request(
    buffer_id: BufferId,
    top_line: u32,
    visible_lines: u32,
) -> EditorViewportRequest {
    EditorViewportRequest {
        buffer_id,
        scroll: ViewportScroll {
            top_line,
            left_column: 0,
        },
        dimensions: ViewportDimensions {
            width_px: 1_200,
            height_px: visible_lines.saturating_mul(16),
        },
    }
}

fn viewport_payload_bytes(viewport: &ViewportProjection) -> usize {
    viewport
        .line_slices
        .iter()
        .map(|slice| slice.visible_text.len())
        .sum()
}

#[derive(Debug)]
struct FakeBackgroundConsumer {
    kind: SnapshotConsumerKind,
    queue_capacity: usize,
    queued_events: usize,
    accepted_events: usize,
    dropped_events: usize,
    stale_events: usize,
    skipped_versions: u64,
    last_post_version: Option<u64>,
}

impl FakeBackgroundConsumer {
    fn new(kind: SnapshotConsumerKind, queue_capacity: usize) -> Self {
        Self {
            kind,
            queue_capacity,
            queued_events: 0,
            accepted_events: 0,
            dropped_events: 0,
            stale_events: 0,
            skipped_versions: 0,
            last_post_version: None,
        }
    }

    fn offer_events(&mut self, descriptors: &[TextTransactionDescriptor]) {
        for descriptor in descriptors {
            let post_version = descriptor.post_buffer_version.0;
            if let Some(last_post_version) = self.last_post_version {
                if post_version <= last_post_version {
                    self.stale_events += 1;
                    continue;
                }
                self.skipped_versions = self
                    .skipped_versions
                    .saturating_add(post_version.saturating_sub(last_post_version + 1));
            }
            self.last_post_version = Some(post_version);

            if self.queued_events >= self.queue_capacity {
                self.dropped_events += 1;
            } else {
                self.queued_events += 1;
                self.accepted_events += 1;
            }
        }
    }

    fn drain_one(&mut self) {
        self.queued_events = self.queued_events.saturating_sub(1);
    }
}

#[test]
fn ci_typical_edit_latency_on_budget_sized_file() {
    let mut engine = EditorEngine::new();
    let size = 256 * 1024;
    let text = "a".repeat(size);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(90), "ci-budget.txt", text)
        .expect("open buffer");
    let mut samples = Vec::new();

    for i in 0..16 {
        let at = TextPosition::new(0, size / 2 + i);
        let start = Instant::now();
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(at, "x"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("insert edit");
        samples.push(start.elapsed());
    }

    let mut sorted = samples.clone();
    let p50 = percentile(&mut sorted, 0.50);
    let p95 = percentile(&mut sorted, 0.95);
    eprintln!("ci budget edit latency p50={p50:?} p95={p95:?}");
    assert!(p95 < Duration::from_millis(250));
}

#[test]
fn ci_snapshot_retention_budget_is_enforced() {
    let mut engine =
        EditorEngine::with_snapshot_retention_policy(deterministic_retention_policy(6));
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(91), "ci-retention.txt", "seed")
        .expect("open buffer");

    for _ in 0..24 {
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "x"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("edit");
    }

    assert!(engine.retained_snapshot_count() <= 6);
    assert!(engine.undo_len(buffer).expect("undo len") <= 5);
}

#[test]
fn ci_undo_redo_burst_small_deterministic_sample() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(92), "ci-burst.txt", "a")
        .expect("open buffer");

    for _ in 0..64 {
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "x"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("apply edit");
    }

    let undo_start = Instant::now();
    for _ in 0..64 {
        engine.undo(buffer, None).expect("undo");
    }
    let undo_total = undo_start.elapsed();

    let redo_start = Instant::now();
    for _ in 0..64 {
        engine.redo(buffer, None).expect("redo");
    }
    let redo_total = redo_start.elapsed();

    eprintln!("ci undo_total={undo_total:?} redo_total={redo_total:?}");
    assert!(undo_total < Duration::from_millis(500));
    assert!(redo_total < Duration::from_millis(500));
}

#[test]
fn ci_large_file_degraded_open_and_viewport_are_bounded() {
    let mut engine = EditorEngine::new();
    let text = deterministic_large_text(CI_LARGE_FILE_BYTES);

    let open_start = Instant::now();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(100), "ci-large.txt", text)
        .expect("open large buffer in degraded mode");
    let open_elapsed = open_start.elapsed();

    assert_eq!(
        engine.buffer_mode(buffer).expect("buffer mode"),
        BufferMode::Degraded
    );
    assert!(
        engine.text(buffer).is_err(),
        "large buffers must not expose full-source text through compatibility access"
    );

    let chunks = engine
        .snapshot_chunk_descriptors(buffer)
        .expect("snapshot chunk descriptors");
    assert!(chunks.len() > 1, "large snapshot should be chunked");

    let viewport_start = Instant::now();
    let viewport = engine
        .viewport_projection(viewport_request(buffer, 128, 12))
        .expect("viewport projection");
    let viewport_elapsed = viewport_start.elapsed();
    let payload_bytes = viewport_payload_bytes(&viewport);
    let status = viewport
        .large_file_status
        .as_ref()
        .expect("large file status");

    eprintln!(
        "ci degraded open={open_elapsed:?} viewport={viewport_elapsed:?} payload_bytes={payload_bytes} chunk_count={} threshold_bytes={} byte_len={} overlay_reasons={}",
        chunks.len(),
        status.threshold_bytes,
        status.byte_len,
        status.disabled_overlay_reasons.len()
    );

    assert_eq!(viewport.mode, ViewportProjectionMode::DegradedLargeFile);
    assert!(viewport.line_slices.len() <= 12);
    assert_eq!(viewport.line_slices.len(), viewport.line_metrics.len());
    assert!(
        viewport
            .line_slices
            .iter()
            .all(|slice| slice.visible_text.len() < CI_LARGE_FILE_BYTES)
    );
    assert!(payload_bytes < DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES / 32);
    assert!(payload_bytes < CI_LARGE_FILE_BYTES / 32);
    assert_eq!(
        status.threshold_bytes as usize,
        DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES
    );
    assert!(status.byte_len as usize > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES);
    assert_eq!(status.byte_len as usize, CI_LARGE_FILE_BYTES);
    assert!(status.disabled_overlay_reasons.len() >= 3);
    assert!(viewport.decoration_spans.is_empty());
    assert!(viewport.fold_ranges.is_empty());
    assert!(viewport.semantic_token_overlays.is_empty());
    assert!(open_elapsed < ci_large_file_open_threshold());
    assert!(viewport_elapsed < Duration::from_millis(250));
}

#[test]
#[ignore = "performance suite: 100MB degraded-mode measurement"]
fn large_file_100mb_degraded_mode_measurement() {
    let mut engine = EditorEngine::new();
    let size = 100 * 1024 * 1024;
    let text = deterministic_large_text(size);

    let open_start = Instant::now();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(103), "big-degraded.txt", text)
        .expect("open 100MB degraded buffer");
    let open_elapsed = open_start.elapsed();

    assert_eq!(
        engine.buffer_mode(buffer).expect("buffer mode"),
        BufferMode::Degraded
    );
    assert!(engine.text(buffer).is_err());

    let chunks = engine
        .snapshot_chunk_descriptors(buffer)
        .expect("snapshot chunk descriptors");
    let viewport_start = Instant::now();
    let viewport = engine
        .viewport_projection(viewport_request(buffer, 10_000, 24))
        .expect("viewport projection");
    let viewport_elapsed = viewport_start.elapsed();
    let payload_bytes = viewport_payload_bytes(&viewport);

    let mut samples = Vec::new();
    for i in 0..16 {
        let at = TextPosition::new(1_000 + i, 8);
        let start = Instant::now();
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(at, "x"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("insert edit");
        samples.push(start.elapsed());

        let delete = TextEdit::delete(TextRange::new(
            at,
            TextPosition::new(at.line, at.column + 1),
        ));
        engine
            .apply_edit(buffer, delete, TransactionSource::User, None, None)
            .expect("delete inserted byte");
    }

    let mut sorted = samples.clone();
    let p50 = percentile(&mut sorted, 0.50);
    let p95 = percentile(&mut sorted, 0.95);
    let status = viewport
        .large_file_status
        .as_ref()
        .expect("large file status");

    eprintln!(
        "100MB degraded open={open_elapsed:?} viewport={viewport_elapsed:?} edit_p50={p50:?} edit_p95={p95:?} payload_bytes={payload_bytes} chunk_count={} threshold_bytes={} byte_len={} overlay_reasons={}",
        chunks.len(),
        status.threshold_bytes,
        status.byte_len,
        status.disabled_overlay_reasons.len()
    );

    assert_eq!(viewport.mode, ViewportProjectionMode::DegradedLargeFile);
    assert!(payload_bytes < DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES / 32);
    assert!(chunks.len() > 1);
}

#[test]
fn ci_mixed_fake_consumers_do_not_block_user_edits() {
    let mut engine = EditorEngine::with_transaction_event_queue_capacity(8);
    let buffer = engine
        .open_buffer(
            WorkspaceId(1),
            FileId(104),
            "ci-fake-consumers.txt",
            deterministic_large_text(256 * 1024),
        )
        .expect("open bounded small buffer");
    assert_eq!(
        engine.buffer_mode(buffer).expect("buffer mode"),
        BufferMode::Normal
    );

    let simulated_large_thresholds = EditorThresholds::default();
    assert_eq!(
        simulated_large_thresholds.large_file_threshold_bytes,
        DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES
    );

    let mut consumers = [
        FakeBackgroundConsumer::new(SnapshotConsumerKind::Lsp, 5),
        FakeBackgroundConsumer::new(SnapshotConsumerKind::Index, 5),
        FakeBackgroundConsumer::new(SnapshotConsumerKind::Ai, 5),
        FakeBackgroundConsumer::new(SnapshotConsumerKind::Collaboration, 5),
    ];

    let leases = consumers
        .iter()
        .map(|consumer| {
            let lease = engine
                .lease_snapshot(buffer, consumer.kind)
                .expect("lease snapshot for fake consumer");
            assert_eq!(lease.consumer_kind, consumer.kind);
            assert!(lease.chunk_count > 1);
            lease.lease_id
        })
        .collect::<Vec<_>>();

    let mut total_drained_events = 0usize;
    let mut total_engine_dropped_events = 0u64;
    let mut edit_samples = Vec::new();

    for cycle in 0..3 {
        for edit_idx in 0..12 {
            let at = TextPosition::new(256 + cycle * 16 + edit_idx, 8);
            let start = Instant::now();
            engine
                .apply_edit(
                    buffer,
                    TextEdit::insert(at, "u"),
                    TransactionSource::User,
                    None,
                    None,
                )
                .expect("user edit should not wait on fake consumers");
            edit_samples.push(start.elapsed());
        }

        let drained = engine.drain_transaction_events();
        total_drained_events += drained.descriptors.len();
        total_engine_dropped_events =
            total_engine_dropped_events.saturating_add(drained.dropped_before_drain);

        for consumer in &mut consumers {
            consumer.offer_events(&drained.descriptors);
            consumer.drain_one();
        }
    }

    for lease_id in leases {
        assert!(engine.release_snapshot_lease(lease_id).is_some());
    }

    let mut sorted = edit_samples.clone();
    let p50 = percentile(&mut sorted, 0.50);
    let p95 = percentile(&mut sorted, 0.95);
    let consumer_summary = consumers
        .iter()
        .map(|consumer| {
            format!(
                "{:?}:accepted={} dropped={} stale={} skipped_versions={} queued={}",
                consumer.kind,
                consumer.accepted_events,
                consumer.dropped_events,
                consumer.stale_events,
                consumer.skipped_versions,
                consumer.queued_events
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    eprintln!(
        "fake consumer edit latency p50={p50:?} p95={p95:?} drained_events={total_drained_events} engine_dropped_events={total_engine_dropped_events} consumers=[{consumer_summary}]"
    );

    assert_eq!(edit_samples.len(), 36);
    assert_eq!(total_drained_events, 24);
    assert_eq!(total_engine_dropped_events, 12);
    assert!(p95 < Duration::from_secs(1));
    assert!(
        consumers
            .iter()
            .all(|consumer| consumer.accepted_events > 0 && consumer.dropped_events > 0)
    );
    assert!(
        consumers
            .iter()
            .all(|consumer| consumer.skipped_versions > 0 && consumer.stale_events == 0)
    );
}

#[test]
fn ci_collaboration_edit_overhead_p95_p99_vs_user_baseline() {
    const SAMPLES: usize = 256;
    let mut baseline = EditorEngine::new();
    let baseline_buffer = baseline
        .open_buffer(
            WorkspaceId(1),
            FileId(201),
            "collab-baseline.txt",
            deterministic_large_text(64 * 1024),
        )
        .expect("open baseline buffer");
    let mut collaboration = EditorEngine::new();
    let collaboration_buffer = collaboration
        .open_buffer(
            WorkspaceId(1),
            FileId(202),
            "collab-apply.txt",
            deterministic_large_text(64 * 1024),
        )
        .expect("open collaboration buffer");

    let mut baseline_samples = Vec::with_capacity(SAMPLES);
    let mut collaboration_samples = Vec::with_capacity(SAMPLES);
    for idx in 0..SAMPLES {
        let start = Instant::now();
        baseline
            .apply_edit(
                baseline_buffer,
                TextEdit::insert(TextPosition::new(0, 0), "u"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("baseline user edit");
        baseline_samples.push(start.elapsed());

        let start = Instant::now();
        collaboration
            .apply_protocol_edits(EditorApplyTransactionRequest {
                workspace_id: WorkspaceId(1),
                buffer_id: collaboration_buffer,
                file_id: FileId(202),
                edits: EditBatch {
                    edits: vec![ProtocolTextEdit {
                        range: devil_protocol::TextRange::byte(0, 0),
                        replacement: "c".to_string(),
                    }],
                },
                source: TransactionSource::CollaborationParticipant {
                    session_id: CollaborationSessionId(9001),
                    participant_id: CollaborationParticipantId(1),
                    operation_id: CollaborationOperationId(idx as u128 + 1),
                },
                undo_group_id: None,
                correlation_id: devil_protocol::CorrelationId(idx as u64 + 1),
            })
            .expect("collaboration protocol edit");
        collaboration_samples.push(start.elapsed());
    }

    let mut baseline_sorted = baseline_samples.clone();
    let baseline_p50 = percentile(&mut baseline_sorted, 0.50);
    let baseline_p95 = percentile(&mut baseline_sorted, 0.95);
    let baseline_p99 = percentile(&mut baseline_sorted, 0.99);
    let mut collaboration_sorted = collaboration_samples.clone();
    let collaboration_p50 = percentile(&mut collaboration_sorted, 0.50);
    let collaboration_p95 = percentile(&mut collaboration_sorted, 0.95);
    let collaboration_p99 = percentile(&mut collaboration_sorted, 0.99);
    let p95_overhead = collaboration_p95.saturating_sub(baseline_p95);
    let p99_overhead = collaboration_p99.saturating_sub(baseline_p99);

    eprintln!(
        "collaboration edit overhead samples={SAMPLES} baseline_p50={baseline_p50:?} baseline_p95={baseline_p95:?} baseline_p99={baseline_p99:?} collaboration_p50={collaboration_p50:?} collaboration_p95={collaboration_p95:?} collaboration_p99={collaboration_p99:?} overhead_p95={p95_overhead:?} overhead_p99={p99_overhead:?}"
    );

    assert!(p95_overhead <= collaboration_edit_p95_overhead_threshold());
    assert!(p99_overhead <= collaboration_edit_p99_overhead_threshold());
}

#[test]
fn ci_collaboration_snapshot_consumer_overhead_p95_p99() {
    const SAMPLES: usize = 128;
    let mut engine = EditorEngine::with_transaction_event_queue_capacity(256);
    let buffer = engine
        .open_buffer(
            WorkspaceId(1),
            FileId(203),
            "collab-consumer.txt",
            deterministic_large_text(64 * 1024),
        )
        .expect("open buffer");
    let mut consumer = FakeBackgroundConsumer::new(SnapshotConsumerKind::Collaboration, 64);
    let lease = engine
        .lease_snapshot(buffer, SnapshotConsumerKind::Collaboration)
        .expect("lease collaboration snapshot");

    let mut baseline_samples = Vec::with_capacity(SAMPLES);
    let mut consumer_samples = Vec::with_capacity(SAMPLES);
    for idx in 0..SAMPLES {
        let descriptor = TextTransactionDescriptor {
            workspace_id: WorkspaceId(1),
            buffer_id: buffer,
            file_id: FileId(203),
            transaction_id: uuid::Uuid::now_v7(),
            correlation_id: devil_protocol::CorrelationId(idx as u64 + 1),
            source: TransactionSource::User,
            pre_snapshot_id: devil_protocol::SnapshotId(idx as u128 + 1),
            post_snapshot_id: devil_protocol::SnapshotId(idx as u128 + 2),
            pre_buffer_version: devil_protocol::BufferVersion(idx as u64 + 1),
            post_buffer_version: devil_protocol::BufferVersion(idx as u64 + 2),
            changed_ranges: Vec::new(),
            causality_id: devil_protocol::CausalityId(uuid::Uuid::now_v7()),
            parent_transaction_id: None,
            schema_version: 1,
            undo_group_id: None,
            occurred_at: devil_protocol::TimestampMillis(1),
        };
        let start = Instant::now();
        std::hint::black_box(&descriptor);
        baseline_samples.push(start.elapsed());

        let start = Instant::now();
        consumer.offer_events(std::slice::from_ref(&descriptor));
        consumer.drain_one();
        consumer_samples.push(start.elapsed());
    }
    assert!(engine.release_snapshot_lease(lease.lease_id).is_some());

    let mut baseline_sorted = baseline_samples.clone();
    let baseline_p50 = percentile(&mut baseline_sorted, 0.50);
    let baseline_p95 = percentile(&mut baseline_sorted, 0.95);
    let baseline_p99 = percentile(&mut baseline_sorted, 0.99);
    let mut consumer_sorted = consumer_samples.clone();
    let consumer_p50 = percentile(&mut consumer_sorted, 0.50);
    let consumer_p95 = percentile(&mut consumer_sorted, 0.95);
    let consumer_p99 = percentile(&mut consumer_sorted, 0.99);
    let p95_overhead = consumer_p95.saturating_sub(baseline_p95);
    let p99_overhead = consumer_p99.saturating_sub(baseline_p99);

    eprintln!(
        "collaboration snapshot consumer overhead samples={SAMPLES} baseline_p50={baseline_p50:?} baseline_p95={baseline_p95:?} baseline_p99={baseline_p99:?} consumer_p50={consumer_p50:?} consumer_p95={consumer_p95:?} consumer_p99={consumer_p99:?} overhead_p95={p95_overhead:?} overhead_p99={p99_overhead:?} accepted={} dropped={} queued={}",
        consumer.accepted_events, consumer.dropped_events, consumer.queued_events
    );

    assert!(p95_overhead <= Duration::from_millis(2));
    assert!(p99_overhead <= Duration::from_millis(5));
    assert!(consumer.accepted_events > 0);
}

#[test]
#[ignore = "performance suite: undo/redo latency"]
fn undo_redo_latency_under_edit_burst() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(101), "burst.txt", "a")
        .expect("open buffer");

    for _ in 0..2_000 {
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "x"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("apply edit");
    }

    let undo_start = Instant::now();
    for _ in 0..2_000 {
        let _ = engine.undo(buffer, None).expect("undo");
    }
    let undo_total = undo_start.elapsed();

    let redo_start = Instant::now();
    for _ in 0..2_000 {
        let _ = engine.redo(buffer, None).expect("redo");
    }
    let redo_total = redo_start.elapsed();

    eprintln!("undo_total={undo_total:?} redo_total={redo_total:?}");
    assert!(undo_total < Duration::from_secs(3));
    assert!(redo_total < Duration::from_secs(3));
}

#[test]
#[ignore = "performance suite: snapshot retention"]
fn snapshot_retention_and_release() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(102), "retention.txt", "seed")
        .expect("open buffer");

    for _ in 0..32 {
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "x"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("apply edit");
    }

    let pin_before_save = engine.pinned_snapshot_count();
    let save = engine.request_save(buffer, None).expect("request save");
    let pin_after_save = engine.pinned_snapshot_count();
    assert!(pin_after_save >= pin_before_save);

    engine.acknowledge_save(save.request_id, true);
    let pin_after_ack = engine.pinned_snapshot_count();
    assert!(pin_after_ack <= pin_after_save);
}
