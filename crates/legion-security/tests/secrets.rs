use legion_security::{RedactionPayloadKind, scan_payload_for_sensitive_markers};

#[test]
fn proposal_content_payload_requires_redaction() {
    let report = scan_payload_for_sensitive_markers(
        RedactionPayloadKind::Trace,
        "proposal_content: serialized workspace proposal payload",
    );

    assert!(!report.passed());
    assert!(report.redaction_required);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.marker_label == "raw-proposal-content"));
}

#[test]
fn terminal_excerpt_payload_requires_redaction() {
    let report = scan_payload_for_sensitive_markers(
        RedactionPayloadKind::Log,
        "terminal_excerpts: captured shell excerpt payload",
    );

    assert!(!report.passed());
    assert!(report.redaction_required);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.marker_label == "raw-terminal-excerpts"));
}

#[test]
fn retained_and_ejected_context_payload_requires_redaction() {
    let report = scan_payload_for_sensitive_markers(
        RedactionPayloadKind::Diff,
        "retained_context: workspace buffer snapshot\nejected_context: discarded buffer snapshot",
    );

    assert!(!report.passed());
    assert!(report.redaction_required);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.marker_label == "retained-context"));
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.marker_label == "ejected-context"));
}
