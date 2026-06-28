//! Helpers for redacting and bounding model-facing text payloads.

use std::sync::OnceLock;

use legion_security::{RedactionPayloadKind, scan_payload_for_sensitive_markers};
use regex::Regex;

const REDACTED: &str = "[redacted]";

/// Compiled redaction patterns, built once and reused.
///
/// Each pattern matches an *entire* secret (marker + value), not just the
/// leading marker, so the unique suffix of a key can never survive
/// redaction. Patterns are applied in order; earlier, more specific
/// patterns (assignments, auth headers) run before the bare-marker fallback
/// so a `KEY=value` pair collapses to a single `[redacted]`.
fn redaction_patterns() -> &'static [(Regex, &'static str)] {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let specs: &[&str] = &[
            // `Authorization: Bearer <token>` — redact the scheme and token together.
            r"(?i)authorization:\s*bearer\s+\S+",
            // `KEY=value` secret assignments — redact the whole assignment.
            r"(?i)(?:openai_api_key|aws_secret_access_key|api_key)\s*=\s*\S+",
            // Provider/token prefixes — redact the prefix and its trailing value.
            r"(?i)(?:sk-|xoxb-|ghp_|gho_)\S+",
            // Bare secret/raw-context markers that must never cross the boundary.
            r"(?i)proposal_content|terminal_excerpts|terminal_excerpt|retained_context|ejected_context|source_body|provider_payload|openai_api_key|aws_secret_access_key",
        ];
        specs
            .iter()
            .map(|spec| (Regex::new(spec).expect("static redaction regex must compile"), REDACTED))
            .collect()
    })
}

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

    for (pattern, replacement) in redaction_patterns() {
        redacted_text = pattern.replace_all(&redacted_text, *replacement).into_owned();
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
            "prefix OPENAI_API_KEY=sk-test-mock-value Authorization: Bearer mock-bearer-value blob";

        let result = redact_model_bound_output(payload, 128);

        assert!(result.redacted);
        assert!(!result.truncated);
        assert!(result.redacted_text.contains("[redacted]"));
        // The full secret value must be gone, not just its marker/prefix.
        assert!(!result.redacted_text.contains("sk-test-mock-value"));
        assert!(!result.redacted_text.contains("test-mock-value"));
        assert!(!result.redacted_text.contains("Authorization: Bearer"));
        assert!(!result.redacted_text.contains("mock-bearer-value"));
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
