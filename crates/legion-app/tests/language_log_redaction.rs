use legion_app::language::redact_lsp_stderr;

#[test]
fn redaction_keeps_only_metadata_counts() {
    let raw = "INFO starting\nERROR cannot open /home/secret/path\nWARN slow\nERROR boom";
    let summary = redact_lsp_stderr(raw);
    assert_eq!(summary.line_count, 4);
    assert_eq!(summary.error_lines, 2);
    assert_eq!(summary.warn_lines, 1);
}
