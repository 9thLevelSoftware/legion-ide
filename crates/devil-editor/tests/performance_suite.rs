use std::time::{Duration, Instant};

use devil_editor::{
    EditorEngine, SnapshotEvictionPreference, SnapshotRetentionPolicy, TextPosition,
};
use devil_protocol::{FileId, TransactionSource, WorkspaceId};
use devil_text::{TextEdit, TextRange};

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
    let p95 = percentile(&mut sorted, 0.95);
    eprintln!("ci budget edit latency p95={p95:?}");
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
#[ignore = "performance suite: 100MB+ workload"]
fn large_file_100mb_keystroke_latency() {
    let mut engine = EditorEngine::new();
    let size = 100 * 1024 * 1024;
    let text = "a".repeat(size);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(100), "big.txt", text)
        .expect("open large buffer");

    let at = TextPosition::new(0, size / 2);
    let mut samples = Vec::new();
    for _ in 0..64 {
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

        // delete inserted byte to keep cursor position stable
        let delete = TextEdit::delete(TextRange::new(
            at,
            TextPosition::new(at.line, at.column + 1),
        ));
        let _ = engine.apply_edit(buffer, delete, TransactionSource::User, None, None);
    }

    let mut sorted = samples.clone();
    let p50 = percentile(&mut sorted, 0.50);
    let p95 = percentile(&mut sorted, 0.95);

    eprintln!("100MB keystroke latency p50={p50:?} p95={p95:?}");
    assert!(p95 < Duration::from_secs(2));
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
