use std::time::{Duration, Instant};

use devil_editor::{EditorEngine, TextPosition};
use devil_protocol::{FileId, TransactionSource, WorkspaceId};
use devil_text::{TextEdit, TextRange};

fn percentile(durations: &mut [Duration], pct: f64) -> Duration {
    durations.sort();
    let idx = ((durations.len() as f64 - 1.0) * pct).round() as usize;
    durations[idx]
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

