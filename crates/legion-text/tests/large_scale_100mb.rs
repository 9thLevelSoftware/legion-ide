//! 100MB scale integration tests for the text model.
//!
//! These tests are marked `#[ignore]` and must be run explicitly:
//!
//! ```
//! cargo test -p legion-text --test large_scale_100mb -- --ignored --nocapture
//! ```

use legion_protocol::BufferVersion;
use legion_text::{DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, TextBuffer, TextError};
use std::time::Instant;

const TARGET_BYTES: usize = 100 * 1024 * 1024; // 100 MB
const LINE_PATTERN: &str = "abcdefghijklmnopqrstuvwxyz0123456789_|\n"; // 39 bytes/line

/// Generate a string that is approximately `target_bytes` in size by repeating LINE_PATTERN.
fn generate_100mb_text() -> String {
    let line_len = LINE_PATTERN.len(); // 39 bytes
    let line_count = TARGET_BYTES / line_len;
    let mut s = String::with_capacity(line_count * line_len);
    for _ in 0..line_count {
        s.push_str(LINE_PATTERN);
    }
    s
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_buffer_creation_under_budget() {
    let text = generate_100mb_text();
    let text_len = text.len();
    eprintln!(
        "Generated text: {} bytes ({} MB)",
        text_len,
        text_len / (1024 * 1024)
    );

    let t0 = Instant::now();
    let buf = TextBuffer::try_with_version(text, BufferVersion(0))
        .expect("buffer creation should succeed");
    let elapsed = t0.elapsed();
    eprintln!("Buffer creation elapsed: {:?}", elapsed);

    assert!(
        elapsed.as_secs() < 5,
        "buffer creation took {:?}, expected < 5s",
        elapsed
    );

    // Must be in degraded mode — 100MB far exceeds the 5MB full-cache budget
    assert!(
        text_len > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES,
        "text ({} bytes) should exceed budget ({} bytes)",
        text_len,
        DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES
    );
    assert!(
        matches!(
            buf.try_full_text(),
            Err(TextError::FullCacheBudgetExceeded { .. })
        ),
        "expected FullCacheBudgetExceeded for 100MB buffer"
    );

    // Byte length approximately 100MB
    let byte_len = buf.len();
    eprintln!(
        "Buffer byte length: {} bytes ({} MB)",
        byte_len,
        byte_len / (1024 * 1024)
    );
    assert!(
        byte_len >= 90 * 1024 * 1024,
        "expected byte_len >= 90MB, got {} bytes",
        byte_len
    );

    // Line count > 2M (100MB / 39 bytes per line ≈ 2.69M lines)
    let line_count = buf.line_count();
    eprintln!("Line count: {}", line_count);
    assert!(
        line_count > 2_000_000,
        "expected line_count > 2M, got {}",
        line_count
    );
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_viewport_slice_under_budget() {
    let text = generate_100mb_text();
    let buf = TextBuffer::try_with_version(text, BufferVersion(0))
        .expect("buffer creation should succeed");

    let total_lines = buf.line_count();
    let mid = total_lines / 2;
    let viewport_start = mid.saturating_sub(20);
    let viewport_end = viewport_start + 40;

    eprintln!(
        "Requesting lines {}..{} (total lines: {})",
        viewport_start, viewport_end, total_lines
    );

    let t0 = Instant::now();
    let slices = buf
        .visible_line_slices(viewport_start, viewport_end)
        .expect("viewport slice should succeed");
    let elapsed = t0.elapsed();
    eprintln!("visible_line_slices elapsed: {:?}", elapsed);

    assert!(
        elapsed.as_millis() < 1,
        "viewport slice took {:?}, expected < 1ms",
        elapsed
    );

    assert_eq!(slices.len(), 40, "expected 40 slices, got {}", slices.len());

    for (i, slice) in slices.iter().enumerate() {
        assert!(
            !slice.truncated,
            "slice[{}] (line {}) unexpectedly truncated; line content len = {}",
            i, slice.line, slice.line_content_byte_len
        );
    }

    eprintln!("All {} slices returned non-truncated", slices.len());
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_single_keystroke_edit_under_budget() {
    let text = generate_100mb_text();
    let original_len = text.len();
    let mut buf = TextBuffer::try_with_version(text, BufferVersion(0))
        .expect("buffer creation should succeed");

    // Find a safe char boundary near the middle of the document
    let mid = original_len / 2;
    // LINE_PATTERN is all ASCII so any byte offset aligned to a line boundary is a char boundary.
    // Walk forward to find a valid char boundary using position().
    let insert_offset = (mid..(mid + 100))
        .find(|&off| buf.position(off).is_some())
        .expect("should find a char boundary within 100 bytes of midpoint");

    eprintln!(
        "Inserting 'X' at byte offset {} (doc len = {})",
        insert_offset, original_len
    );

    let t0 = Instant::now();
    buf.try_insert(insert_offset, "X")
        .expect("insert should succeed");
    let elapsed = t0.elapsed();
    eprintln!("Insert elapsed: {:?}", elapsed);

    // 50ms target for release builds; debug builds (unoptimized) may take up to 500ms.
    #[cfg(not(debug_assertions))]
    let threshold_ms = 50u128;
    #[cfg(debug_assertions)]
    let threshold_ms = 500u128;

    assert!(
        elapsed.as_millis() < threshold_ms,
        "single keystroke insert took {:?}, expected < {}ms",
        elapsed,
        threshold_ms
    );

    assert_eq!(
        buf.len(),
        original_len + 1,
        "expected len to increase by 1 after insert"
    );
    eprintln!("Post-edit len: {} (was {})", buf.len(), original_len);
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_memory_ceiling() {
    let text = generate_100mb_text();
    let text_len = text.len();
    let buf = TextBuffer::try_with_version(text, BufferVersion(0))
        .expect("buffer creation should succeed");

    let footprint = buf.memory_footprint_bytes();
    eprintln!(
        "Memory footprint: {} bytes ({} MB) for {} MB document",
        footprint,
        footprint / (1024 * 1024),
        text_len / (1024 * 1024)
    );

    let ceiling = 400 * 1024 * 1024; // 400 MB
    assert!(
        footprint < ceiling,
        "memory footprint {} bytes ({} MB) exceeds 400MB ceiling",
        footprint,
        footprint / (1024 * 1024)
    );
}

#[test]
fn streaming_from_reader_matches_string_path_10mb() {
    // Non-ignored smaller-scale test that validates the streaming constructor
    // produces identical results to the String-based constructor.
    let target_bytes = 10 * 1024 * 1024; // 10 MB
    let text = {
        let line_len = LINE_PATTERN.len();
        let line_count = target_bytes / line_len;
        let mut s = String::with_capacity(line_count * line_len);
        for _ in 0..line_count {
            s.push_str(LINE_PATTERN);
        }
        s
    };
    let text_len = text.len();

    let t0 = Instant::now();
    let string_buf =
        TextBuffer::try_with_version(text.clone(), BufferVersion(0)).expect("string buffer");
    let string_elapsed = t0.elapsed();

    let t1 = Instant::now();
    let reader_buf = TextBuffer::from_reader_with_version(
        std::io::Cursor::new(text.as_bytes()),
        BufferVersion(0),
    )
    .expect("reader buffer");
    let reader_elapsed = t1.elapsed();

    eprintln!(
        "10MB text ({} bytes): string path {:?}, reader path {:?}",
        text_len, string_elapsed, reader_elapsed
    );

    // Structural equivalence: identical line count, byte length, and chunk hashes.
    assert_eq!(reader_buf.len(), string_buf.len());
    assert_eq!(reader_buf.line_count(), string_buf.line_count());
    assert_eq!(
        reader_buf.chunk_descriptors().len(),
        string_buf.chunk_descriptors().len(),
        "chunk count mismatch"
    );
    for (r, s) in reader_buf
        .chunk_descriptors()
        .iter()
        .zip(string_buf.chunk_descriptors().iter())
    {
        assert_eq!(
            r.hash, s.hash,
            "chunk hash mismatch at ordinal {}",
            r.ordinal
        );
    }

    // Both must be above the full-cache budget.
    assert!(text_len > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES);
    assert!(matches!(
        reader_buf.try_full_text(),
        Err(TextError::FullCacheBudgetExceeded { .. })
    ));
}

#[test]
fn streaming_from_file_matches_string_path() {
    // Write a test file, build TextBuffer via from_file, and verify equivalence.
    let text = "hello\nworld\nstreaming from file\n";
    let dir = std::env::temp_dir().join("legion_text_streaming_test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let file_path = dir.join("test_stream.txt");
    std::fs::write(&file_path, text).expect("write test file");

    let string_buf = TextBuffer::new(text);
    let file_buf = TextBuffer::from_file(&file_path).expect("from_file");

    assert_eq!(file_buf.text(), string_buf.text());
    assert_eq!(file_buf.len(), string_buf.len());
    assert_eq!(file_buf.line_count(), string_buf.line_count());
    for (r, s) in file_buf
        .chunk_descriptors()
        .iter()
        .zip(string_buf.chunk_descriptors().iter())
    {
        assert_eq!(r.hash, s.hash);
    }

    // Cleanup.
    let _ = std::fs::remove_file(&file_path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_streaming_from_reader() {
    let text = generate_100mb_text();
    let text_len = text.len();

    let t0 = Instant::now();
    let buf = TextBuffer::from_reader_with_version(
        std::io::Cursor::new(text.as_bytes()),
        BufferVersion(0),
    )
    .expect("streaming buffer creation should succeed");
    let elapsed = t0.elapsed();
    eprintln!(
        "Streaming 100MB buffer creation: {:?} for {} bytes",
        elapsed, text_len
    );

    assert!(
        elapsed.as_secs() < 10,
        "streaming buffer creation took {:?}, expected < 10s",
        elapsed
    );
    assert_eq!(buf.len(), text_len);
    assert!(buf.line_count() > 2_000_000);
    assert!(matches!(
        buf.try_full_text(),
        Err(TextError::FullCacheBudgetExceeded { .. })
    ));
}

#[test]
#[ignore = "100MB scale test - run with: cargo test -p legion-text --test large_scale_100mb -- --ignored"]
fn scale_100mb_snapshot_creation_and_chunk_iteration() {
    let text = generate_100mb_text();
    let doc_len = text.len();
    let buf = TextBuffer::try_with_version(text, BufferVersion(0))
        .expect("buffer creation should succeed");

    let snapshot = buf
        .try_snapshot()
        .expect("snapshot creation should succeed");

    let chunks = snapshot.chunk_descriptors();
    eprintln!("Chunk count: {}", chunks.len());

    assert!(
        chunks.len() > 100,
        "expected > 100 chunks for 100MB document, got {}",
        chunks.len()
    );

    // Verify chunks are contiguous: each chunk's start_byte == previous chunk's end_byte
    for i in 1..chunks.len() {
        assert_eq!(
            chunks[i].start_byte,
            chunks[i - 1].end_byte,
            "chunk[{}].start_byte ({}) != chunk[{}].end_byte ({}): chunks are not contiguous",
            i,
            chunks[i].start_byte,
            i - 1,
            chunks[i - 1].end_byte
        );
    }

    // Verify the last chunk ends at the document end
    let last = chunks.last().expect("chunk list should be non-empty");
    assert_eq!(
        last.end_byte, doc_len,
        "last chunk end_byte ({}) != document length ({})",
        last.end_byte, doc_len
    );

    eprintln!(
        "Chunks verified: {} contiguous chunks spanning {} bytes",
        chunks.len(),
        doc_len
    );
}
