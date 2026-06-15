//! Helpers for redacting and bounding model-facing text payloads.

use legion_security::{RedactionPayloadKind, scan_payload_for_sensitive_markers};

/// Redacted and bounded text payload ready for a model-facing boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelBoundOutput {
    /// Sanitized output text.
    pub redacted_text: String,
    /// Original byte count before redaction or truncation.
    pub byte_count: u64,
    /// Whether the output was truncated to fit the configured bound.
    pub truncated: bool,
    /// Whether any sensitive marker was redacted from the payload.
    pub redacted: bool,
}

/// Redacts common secret markers and truncates the result to a byte ceiling.
#[must_use]
pub fn redact_model_bound_output(output: &str, max_bytes: usize) -> ModelBoundOutput {
    let scan = scan_payload_for_sensitive_markers(RedactionPayloadKind::Log, output);
    let mut redacted_text = output.to_string();

    for (needle, replacement) in [
        ("OPENAI_API_KEY", "[redacted]"),
        ("aws_secret_access_key", "[redacted]"),
        ("Authorization: Bearer", "Authorization: [redacted]"),
        ("authorization: bearer", "authorization: [redacted]"),
        ("ghp_", "[redacted]"),
        ("gho_", "[redacted]"),
        ("proposal_content", "[redacted]"),
        ("terminal_excerpt", "[redacted]"),
        ("terminal_excerpts", "[redacted]"),
        ("retained_context", "[redacted]"),
        ("ejected_context", "[redacted]"),
        ("xoxb-", "[redacted]"),
        ("sk-", "[redacted]"),
        ("secret", "[redacted]"),
    ] {
        redacted_text = redacted_text.replace(needle, replacement);
    }

    let truncated = if redacted_text.len() > max_bytes {
        let bound = char_boundary_floor(&redacted_text, max_bytes);
        redacted_text.truncate(bound);
        true
    } else {
        false
    };

    ModelBoundOutput {
        redacted_text,
        byte_count: output.len() as u64,
        truncated,
        redacted: scan.redaction_required,
    }
}

fn char_boundary_floor(text: &str, limit: usize) -> usize {
    let mut end = limit.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::redact_model_bound_output;

    #[test]
    fn redact_model_bound_output_scrubs_known_secret_markers() {
        let payload =
            "prefix OPENAI_API_KEY=sk-test-12345 Authorization: Bearer abcdef secret blob";

        let result = redact_model_bound_output(payload, 128);

        assert!(result.redacted);
        assert!(!result.truncated);
        assert!(result.redacted_text.contains("[redacted]"));
        assert!(!result.redacted_text.contains("sk-test-12345"));
        assert!(!result.redacted_text.contains("Authorization: Bearer"));
    }

    #[test]
    fn redact_model_bound_output_truncates_on_char_boundaries() {
        let payload = "tool output 😀😀😀😀😀";
        let limit = "tool output 😀😀".len();

        let result = redact_model_bound_output(payload, limit);

        assert!(result.truncated);
        assert_eq!(result.byte_count, payload.len() as u64);
        assert!(result.redacted_text.len() <= limit);
        assert!(
            result
                .redacted_text
                .is_char_boundary(result.redacted_text.len())
        );
    }
}
