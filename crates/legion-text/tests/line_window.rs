use legion_text::{DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, TextError, TextSnapshot};

#[test]
fn line_window_is_bounded_and_reports_absolute_ranges() {
    let snapshot = TextSnapshot::new("prefix:: αβγ ::suffix\nnext");
    let caret = "prefix:: ".len();
    let window = snapshot.line_window_around_byte(caret, 9).unwrap();

    assert_eq!(window.line, 0);
    assert_eq!(window.line_start_byte, 0);
    assert_eq!(window.caret_byte, caret);
    assert!(window.start_byte <= caret && caret <= window.end_byte);
    assert!(window.text.len() <= 9);
    assert_eq!(window.end_byte - window.start_byte, window.text.len());
    assert_eq!(window.logical_end_byte, "prefix:: αβγ ::suffix".len());
    assert_eq!(
        window.grapheme_boundaries.first().copied(),
        Some(window.start_byte)
    );
    assert!(window.grapheme_boundaries.contains(&caret));
}

#[test]
fn line_window_handles_empty_lines_and_crlf_content_edges() {
    let snapshot = TextSnapshot::new("before\r\n\r\nafter");
    let empty = snapshot
        .line_window_around_byte("before\r\n".len(), 8)
        .unwrap();
    assert_eq!(empty.line, 1);
    assert_eq!(empty.start_byte, empty.end_byte);
    assert_eq!(empty.logical_end_byte, empty.line_start_byte);
    assert!(empty.complete_logical_start && empty.complete_logical_end);

    let crlf_interior = "before\r".len();
    assert!(matches!(
        snapshot.line_window_around_byte(crlf_interior, 8),
        Err(TextError::InvalidRange { .. })
    ));
}

#[test]
fn line_window_rejects_zero_or_oversized_budgets_without_materializing_snapshot() {
    let snapshot = TextSnapshot::new("abc");
    assert!(matches!(
        snapshot.line_window_around_byte(1, 0),
        Err(TextError::InvalidWindowBudget { .. })
    ));
    assert!(matches!(
        snapshot.line_window_around_byte(1, 96 * 1024 + 1),
        Err(TextError::InvalidWindowBudget { .. })
    ));

    let large = TextSnapshot::new("x".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES * 2 + 1));
    let window = large
        .line_window_around_byte(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, 32)
        .unwrap();
    assert!(window.text.len() <= 32);
    assert!(large.try_full_text().is_err());
}

#[test]
fn line_window_keeps_cross_window_grapheme_boundaries_truthful() {
    let text = "a\u{301}👩‍💻🇺🇸z";
    let snapshot = TextSnapshot::new(text);
    let combining = snapshot.line_window_around_byte(1, 1).unwrap();
    assert_eq!(combining.start_byte, 0);
    assert_eq!(combining.end_byte, 1);
    assert_eq!(combining.grapheme_boundaries, vec![0]);

    let woman_start = "a\u{301}".len();
    let woman = snapshot
        .line_window_around_byte(woman_start + 4, 4)
        .unwrap();
    assert_eq!(woman.start_byte, woman_start);
    assert_eq!(woman.end_byte, woman_start + 4);
    assert_eq!(woman.grapheme_boundaries, vec![woman_start]);

    let flag_start = woman_start + "👩‍💻".len();
    let flag = snapshot.line_window_around_byte(flag_start + 4, 4).unwrap();
    assert_eq!(flag.start_byte, flag_start);
    assert_eq!(flag.end_byte, flag_start + 4);
    assert_eq!(flag.grapheme_boundaries, vec![flag_start]);

    let window = snapshot.line_window_around_byte(1, 2).unwrap();

    assert!(window.text.len() <= 2);
    assert!(
        window
            .grapheme_boundaries
            .iter()
            .all(|boundary| *boundary >= window.start_byte && *boundary <= window.end_byte)
    );
    assert!(
        !window.grapheme_boundaries.contains(&window.start_byte)
            || window.start_byte == 0
            || snapshot
                .line_window_around_byte(window.start_byte, 32)
                .unwrap()
                .grapheme_boundaries
                .contains(&window.start_byte)
    );
}

#[test]
fn line_window_rejects_insufficient_scalar_budget_and_invalid_carets() {
    let snapshot = TextSnapshot::new("é");
    assert!(matches!(
        snapshot.line_window_around_byte(0, 1),
        Err(TextError::InvalidRange { .. })
    ));
    assert!(matches!(
        snapshot.line_window_around_byte(1, 1),
        Err(TextError::NotUtf8Boundary { .. })
    ));
    assert!(matches!(
        snapshot.line_window_around_byte(2, 2),
        Ok(window) if window.text == "é"
    ));
    assert!(matches!(
        snapshot.line_window_around_byte(3, 2),
        Err(TextError::ByteOffsetOutOfBounds { .. })
    ));
    assert!(matches!(
        snapshot.line_window_around_byte(2, 1),
        Err(TextError::InvalidRange { .. })
    ));
}

#[test]
fn line_window_preserves_nonzero_origin_and_snapshot_immutability() {
    let prefix = "header\n";
    let suffix = "\ntrailer";
    let snapshot = TextSnapshot::new(format!("{prefix}middle αβγ{suffix}"));
    let line_start = prefix.len();
    let before = snapshot.descriptor().clone();
    let window = snapshot.line_window_around_byte(line_start + 7, 8).unwrap();

    assert_eq!(window.line_start_byte, line_start);
    assert!(window.start_byte >= line_start);
    assert!(window.end_byte <= line_start + "middle αβγ".len());
    assert_eq!(*snapshot.descriptor(), before);
}

#[test]
fn line_window_uses_full_rope_context_across_an_actual_chunk_seam() {
    let combining = "\u{301}";
    let cluster = format!("a{}", combining.repeat(3_000));
    let cluster_start = 1_024;
    let text = format!("{}{}z", "x".repeat(cluster_start), cluster);
    let snapshot = TextSnapshot::new(text);
    let rope = snapshot.rope();
    let cluster_end = cluster_start + cluster.len();
    let mut absolute = 0;
    let seam_offset = rope
        .chunks()
        .find_map(|chunk| {
            let start = absolute;
            absolute += chunk.len();
            if cluster_start < start && start < cluster_end {
                Some(start)
            } else if cluster_start < absolute && absolute < cluster_end {
                Some(absolute)
            } else {
                None
            }
        })
        .expect("fixture must cross a rope chunk seam inside one grapheme");
    assert!(cluster_start < seam_offset && seam_offset < cluster_start + cluster.len());
    let window = snapshot
        .line_window_around_byte(cluster_start + 1, 1)
        .unwrap();
    assert_eq!(window.start_byte, cluster_start);
    assert_eq!(window.end_byte, cluster_start + 1);
    assert_eq!(window.grapheme_boundaries, vec![cluster_start]);
}
