//! Secret detection for text that crosses a review, display, or retention boundary.
//!
//! This module replaces substring marker matching (`"ghp_"`, `"api_key="`, ...)
//! with a rule table of provider-specific structural patterns, keyword-anchored
//! assignment rules, and a narrowly scoped entropy heuristic.
//!
//! # Detection posture
//!
//! False negatives and false positives are not symmetric, and they are not
//! symmetric *in the same direction* at every boundary:
//!
//! * A missed secret that reaches a hosted endpoint, a retained artifact, or a
//!   model provider is unrecoverable. Recall matters more than precision there.
//! * A false positive shown to a person reading a proposal preview or a terminal
//!   pane destroys something they needed to read, and — worse — teaches them that
//!   `[redacted]` is noise. Once a reviewer learns to ignore the marker, the
//!   control is dead even when it is right.
//!
//! The resolution is not a single threshold but a split by *action*, expressed as
//! [`ScanPosture`]:
//!
//! * [`SecretConfidence::Structural`] and [`SecretConfidence::Contextual`]
//!   findings redact under every posture. These rules are anchored on a provider
//!   prefix and charset, or on a credential-named assignment whose value passes a
//!   shape test, so their false-positive rate is close to zero.
//! * [`SecretConfidence::Heuristic`] findings (the entropy rule) redact only under
//!   [`ScanPosture::EgressRecall`] — the posture used where data leaves the device
//!   or is persisted. Under [`ScanPosture::DisplayPrecision`] they are reported but
//!   do not blank text a human is reading.
//!
//! So: precision-first where a human reads, recall-first where bytes leave.
//!
//! # Why the entropy rule is deliberately narrow
//!
//! Entropy is the part of a secret scanner that most often makes it unusable.
//! This workspace is full of high-entropy strings that are not secrets:
//! `FileFingerprint` SHA-256 digests, `content_hash` values, `SnapshotId` and
//! `CorrelationId` UUIDs, FNV constants, Cargo.lock checksums, and long
//! `CamelCase` type names. The entropy rule therefore requires *all* of:
//!
//! * length in `32..=512` (short strings cannot exceed `log2(len)` bits/char, so a
//!   meaningful threshold is unreachable below ~32 characters);
//! * at least one uppercase letter, one lowercase letter, and one digit — this
//!   condition eliminates every lowercase hex digest, every UUID, every
//!   `SCREAMING_SNAKE` constant, and every `snake_case` identifier in the tree;
//! * not digest-shaped and not UUID-shaped (belt and braces on the point above);
//! * not [`is_structured_identifier_path`] — see below;
//! * Shannon entropy at or above [`HIGH_ENTROPY_BITS_PER_CHAR`].
//!
//! The character-class condition on its own is *not* sufficient, and an earlier
//! revision of this module claimed it was. Scanning this repository showed 5053
//! findings, of which 5042 were one shape the character-class test cannot see:
//! plan and evidence paths such as
//! `planning/phases/05-control-trust-and-assisted-ai-surfaces/05-01-RESULT`.
//! Those mix case (uppercase suffix), contain digits (phase number), are well
//! over 32 characters, and are neither digest- nor UUID-shaped. They are excluded
//! by [`is_structured_identifier_path`], which is the guard that actually carries
//! this rule. See `plans/evidence/production/2026-08-17-secret-scanning-ruleset.md`
//! for the measurement.
//!
//! Base64 blobs of non-secret data still trip this rule. That residual false
//! positive is the reason the rule is `Heuristic` and not on the display path.

use std::{collections::HashSet, sync::OnceLock};

use legion_protocol::{ProposalId, ProposalPayload, WorkspaceProposal};
use regex::Regex;

/// Text substituted for a detected secret span.
pub const SECRET_REDACTION_PLACEHOLDER: &str = "[redacted]";

/// Shannon entropy floor, in bits per character, for the entropy heuristic.
///
/// Chosen to sit between two populations found in this workspace. Hexadecimal
/// digests are capped at 4.0 bits/char by their 16-symbol alphabet no matter how
/// random they are; UUIDs land near 4.0 with the hyphens counted; long
/// `CamelCase` Rust type names land near 3.9. Random 32+ character credential
/// material over a 62-symbol alphabet lands at 4.5 or above.
///
/// This value is a reasoned starting point, not a corpus-tuned one. Retuning it
/// requires measuring against real payloads, which has not been done.
pub const HIGH_ENTROPY_BITS_PER_CHAR: f64 = 4.3;

/// Minimum candidate length considered by the entropy heuristic.
pub const HIGH_ENTROPY_MIN_LEN: usize = 32;

/// Maximum candidate length considered by the entropy heuristic.
pub const HIGH_ENTROPY_MAX_LEN: usize = 512;

/// Stable identifier for one secret-detection rule.
///
/// The identifier is part of the audit surface: it appears in findings and in
/// evidence, so it must stay stable once shipped. It never carries matched text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SecretRuleId {
    /// AWS access key identifier (`AKIA`/`ASIA`/`ABIA`/`ACCA` + 16 uppercase alphanumerics).
    AwsAccessKeyId,
    /// AWS secret access key bound to an `aws_secret_access_key` assignment.
    AwsSecretAccessKey,
    /// GitHub classic personal access, OAuth, user, server, or refresh token.
    GithubToken,
    /// GitHub fine-grained personal access token.
    GithubFineGrainedToken,
    /// GitLab personal access token.
    GitlabPersonalAccessToken,
    /// Slack bot, user, app, refresh, or legacy workspace token.
    SlackToken,
    /// Slack incoming-webhook URL, which is itself a bearer credential.
    SlackWebhookUrl,
    /// OpenAI-style `sk-` API key.
    OpenAiApiKey,
    /// Anthropic `sk-ant-` API key.
    AnthropicApiKey,
    /// Stripe live or test secret/restricted key.
    StripeSecretKey,
    /// Google API key (`AIza` + 35 characters).
    GoogleApiKey,
    /// npm automation or publish token.
    NpmAccessToken,
    /// PEM private key block header.
    PemPrivateKeyBlock,
    /// JSON Web Token in `header.payload.signature` form.
    JsonWebToken,
    /// HTTP `Authorization` header carrying a bearer, basic, or token credential.
    HttpAuthorizationHeader,
    /// Credentials embedded in a URL userinfo component.
    UrlEmbeddedCredentials,
    /// Credential-named assignment whose value passes the credential shape test.
    GenericSecretAssignment,
    /// Unlabelled high-entropy token.
    HighEntropyToken,
}

impl SecretRuleId {
    /// Returns the stable, display-safe rule identifier.
    pub fn stable_id(self) -> &'static str {
        match self {
            Self::AwsAccessKeyId => "aws-access-key-id",
            Self::AwsSecretAccessKey => "aws-secret-access-key",
            Self::GithubToken => "github-token",
            Self::GithubFineGrainedToken => "github-fine-grained-token",
            Self::GitlabPersonalAccessToken => "gitlab-personal-access-token",
            Self::SlackToken => "slack-token",
            Self::SlackWebhookUrl => "slack-webhook-url",
            Self::OpenAiApiKey => "openai-api-key",
            Self::AnthropicApiKey => "anthropic-api-key",
            Self::StripeSecretKey => "stripe-secret-key",
            Self::GoogleApiKey => "google-api-key",
            Self::NpmAccessToken => "npm-access-token",
            Self::PemPrivateKeyBlock => "pem-private-key-block",
            Self::JsonWebToken => "json-web-token",
            Self::HttpAuthorizationHeader => "http-authorization-header",
            Self::UrlEmbeddedCredentials => "url-embedded-credentials",
            Self::GenericSecretAssignment => "generic-secret-assignment",
            Self::HighEntropyToken => "high-entropy-token",
        }
    }

    /// Returns every rule identifier, so tests can assert full fixture coverage.
    pub fn all() -> &'static [SecretRuleId] {
        &[
            Self::AwsAccessKeyId,
            Self::AwsSecretAccessKey,
            Self::GithubToken,
            Self::GithubFineGrainedToken,
            Self::GitlabPersonalAccessToken,
            Self::SlackToken,
            Self::SlackWebhookUrl,
            Self::OpenAiApiKey,
            Self::AnthropicApiKey,
            Self::StripeSecretKey,
            Self::GoogleApiKey,
            Self::NpmAccessToken,
            Self::PemPrivateKeyBlock,
            Self::JsonWebToken,
            Self::HttpAuthorizationHeader,
            Self::UrlEmbeddedCredentials,
            Self::GenericSecretAssignment,
            Self::HighEntropyToken,
        ]
    }
}

/// How much a rule's match is trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SecretConfidence {
    /// Provider-specific prefix, charset, and length. Effectively no false positives.
    Structural,
    /// Credential-named assignment whose value passed the credential shape test.
    Contextual,
    /// Entropy-only signal with no credential naming or provider prefix.
    Heuristic,
}

/// Impact class of a detected credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SecretSeverity {
    /// Long-lived cloud or asymmetric key material.
    Critical,
    /// Provider API token or session bearer credential.
    High,
    /// Probable credential without a proven provider binding.
    Medium,
}

/// Which side of the precision/recall trade a caller sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanPosture {
    /// Rendering to a person on this device: only high-confidence findings redact.
    ///
    /// Over-redacting a preview or terminal pane trains reviewers to ignore
    /// `[redacted]`, which costs more than the heuristic recall is worth.
    DisplayPrecision,
    /// Leaving the device or being persisted: heuristic findings redact too.
    ///
    /// A leak past this boundary cannot be undone, so over-redaction is the
    /// cheaper error.
    EgressRecall,
}

/// Byte range of a detected secret inside the scanned text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SecretSpan {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

impl SecretSpan {
    /// Returns true when the two spans share at least one byte.
    pub fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// One detected secret. Deliberately carries no matched text.
///
/// Findings are copied into audit records and evidence summaries. Carrying the
/// matched bytes would move the credential into exactly the artifacts this
/// scanner exists to keep clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretFinding {
    /// Rule that produced the finding.
    pub rule_id: SecretRuleId,
    /// Confidence tier of the rule.
    pub confidence: SecretConfidence,
    /// Impact class of the credential.
    pub severity: SecretSeverity,
    /// Byte range of the credential value in the scanned text.
    pub span: SecretSpan,
}

/// Result of scanning one text payload.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SecretScanReport {
    /// Number of bytes scanned.
    pub scanned_bytes: usize,
    /// Findings ordered by start offset.
    pub findings: Vec<SecretFinding>,
}

impl SecretScanReport {
    /// Returns true when nothing at all was detected.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// Returns true when the given posture requires redaction before release.
    pub fn requires_redaction(&self, posture: ScanPosture) -> bool {
        self.findings
            .iter()
            .any(|finding| finding_applies(*finding, posture))
    }

    /// Returns the highest severity present, or `None` when there are no findings.
    ///
    /// `SecretSeverity` is declared most-severe-first, so the maximum impact is
    /// the ordinal minimum.
    pub fn max_severity(&self) -> Option<SecretSeverity> {
        self.findings.iter().map(|finding| finding.severity).min()
    }

    /// Returns the distinct rule identifiers that fired, sorted and deduplicated.
    pub fn rule_ids(&self) -> Vec<SecretRuleId> {
        let unique: HashSet<SecretRuleId> = self
            .findings
            .iter()
            .map(|finding| finding.rule_id)
            .collect();
        let mut ids: Vec<SecretRuleId> = unique.into_iter().collect();
        ids.sort_unstable();
        ids
    }
}

/// Redaction result for a text payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedText {
    /// Text with every applicable finding replaced by [`SECRET_REDACTION_PLACEHOLDER`].
    pub text: String,
    /// Findings observed in the original text, including ones the posture did not redact.
    pub report: SecretScanReport,
    /// Whether any span was actually replaced.
    pub redacted: bool,
}

fn finding_applies(finding: SecretFinding, posture: ScanPosture) -> bool {
    match posture {
        ScanPosture::DisplayPrecision => finding.confidence != SecretConfidence::Heuristic,
        ScanPosture::EgressRecall => true,
    }
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Scans `text` for credentials using the full rule table.
///
/// Overlapping matches are resolved in rule order: the first rule to claim a byte
/// range wins, so `sk-ant-...` is reported as `anthropic-api-key` rather than as
/// the more general `openai-api-key`, and a structurally identified provider key
/// is never also reported as an anonymous high-entropy token.
pub fn scan_text_for_secrets(text: &str) -> SecretScanReport {
    let mut findings: Vec<SecretFinding> = Vec::new();

    for rule in structural_rules() {
        for captures in rule.regex.captures_iter(text) {
            let Some(value) = captures.get(rule.value_group) else {
                continue;
            };
            if let Some(validate) = rule.validate
                && !validate(value.as_str())
            {
                continue;
            }
            push_non_overlapping(
                &mut findings,
                SecretFinding {
                    rule_id: rule.id,
                    confidence: rule.confidence,
                    severity: rule.severity,
                    span: SecretSpan {
                        start: value.start(),
                        end: value.end(),
                    },
                },
            );
        }
    }

    scan_credential_assignments(text, &mut findings);
    scan_high_entropy_tokens(text, &mut findings);

    findings.sort_by_key(|finding| finding.span.start);
    SecretScanReport {
        scanned_bytes: text.len(),
        findings,
    }
}

/// Replaces every finding the posture applies to with [`SECRET_REDACTION_PLACEHOLDER`].
pub fn redact_secrets_in_text(text: &str, posture: ScanPosture) -> RedactedText {
    let report = scan_text_for_secrets(text);
    let mut spans: Vec<SecretSpan> = report
        .findings
        .iter()
        .filter(|finding| finding_applies(**finding, posture))
        .map(|finding| finding.span)
        .collect();
    spans.sort_by_key(|span| span.start);

    if spans.is_empty() {
        return RedactedText {
            text: text.to_string(),
            report,
            redacted: false,
        };
    }

    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for span in spans {
        // `push_non_overlapping` already discarded overlaps, but a defensive skip
        // keeps this loop total if the finding list is ever built by hand.
        if span.start < cursor {
            continue;
        }
        out.push_str(&text[cursor..span.start]);
        out.push_str(SECRET_REDACTION_PLACEHOLDER);
        cursor = span.end;
    }
    out.push_str(&text[cursor..]);

    RedactedText {
        text: out,
        report,
        redacted: true,
    }
}

fn push_non_overlapping(findings: &mut Vec<SecretFinding>, candidate: SecretFinding) {
    if findings
        .iter()
        .any(|existing| existing.span.overlaps(candidate.span))
    {
        return;
    }
    findings.push(candidate);
}

// ---------------------------------------------------------------------------
// Structural rules
// ---------------------------------------------------------------------------

struct StructuralRule {
    id: SecretRuleId,
    severity: SecretSeverity,
    confidence: SecretConfidence,
    regex: Regex,
    /// Capture group whose byte range is reported and redacted.
    value_group: usize,
    /// Shape test applied to the captured value when the pattern alone is not
    /// specific enough to stand on its own.
    validate: Option<fn(&str) -> bool>,
}

fn structural_rule(
    id: SecretRuleId,
    severity: SecretSeverity,
    confidence: SecretConfidence,
    pattern: &str,
    value_group: usize,
    validate: Option<fn(&str) -> bool>,
) -> StructuralRule {
    StructuralRule {
        id,
        severity,
        confidence,
        regex: Regex::new(pattern).expect("static secret rule pattern must compile"),
        value_group,
        validate,
    }
}

fn structural_rules() -> &'static [StructuralRule] {
    static RULES: OnceLock<Vec<StructuralRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        // Order is significant: earlier rules claim byte ranges first, so the most
        // specific provider pattern must precede the more general one.
        vec![
            structural_rule(
                SecretRuleId::PemPrivateKeyBlock,
                SecretSeverity::Critical,
                SecretConfidence::Structural,
                r"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----",
                0,
                None,
            ),
            structural_rule(
                SecretRuleId::AwsAccessKeyId,
                SecretSeverity::Critical,
                SecretConfidence::Structural,
                r"\b((?:AKIA|ASIA|ABIA|ACCA)[0-9A-Z]{16})\b",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::AwsSecretAccessKey,
                SecretSeverity::Critical,
                SecretConfidence::Structural,
                r#"(?i)aws_secret_access_key["']?\s*[:=]\s*["']?([A-Za-z0-9/+=]{40})"#,
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::GithubFineGrainedToken,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b(github_pat_[A-Za-z0-9_]{40,})\b",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::GithubToken,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b(gh[pousr]_[A-Za-z0-9]{36})\b",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::GitlabPersonalAccessToken,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b(glpat-[A-Za-z0-9_\-]{20,})",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::SlackWebhookUrl,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"(https://hooks\.slack\.com/services/[A-Za-z0-9_/+\-]{20,})",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::SlackToken,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b(xox[abeprs]-[A-Za-z0-9\-]{12,})",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::AnthropicApiKey,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b(sk-ant-[A-Za-z0-9_\-]{24,})",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::StripeSecretKey,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b((?:sk|rk)_(?:live|test)_[A-Za-z0-9]{16,})\b",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::OpenAiApiKey,
                SecretSeverity::High,
                SecretConfidence::Structural,
                // `sk-` alone is not distinctive: `sk-learn-model-training-pipeline`
                // is a plausible identifier. The length floor plus the opacity test
                // is what separates a key from a kebab-case name.
                r"\b(sk-[A-Za-z0-9_\-]{32,})",
                1,
                Some(is_opaque_credential_token),
            ),
            structural_rule(
                SecretRuleId::GoogleApiKey,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b(AIza[A-Za-z0-9_\-]{35})",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::NpmAccessToken,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b(npm_[A-Za-z0-9]{36})\b",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::JsonWebToken,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"\b(eyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{8,})",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::HttpAuthorizationHeader,
                SecretSeverity::High,
                SecretConfidence::Structural,
                r"(?i)(authorization\s*:\s*(?:bearer|basic|token)\s+[A-Za-z0-9._~+/=\-]{8,})",
                1,
                None,
            ),
            structural_rule(
                SecretRuleId::UrlEmbeddedCredentials,
                SecretSeverity::Critical,
                SecretConfidence::Structural,
                r"[a-zA-Z][a-zA-Z0-9+.\-]*://[^\s/:@]{1,64}:([^\s/@]{3,64})@",
                1,
                Some(is_not_placeholder_value),
            ),
        ]
    })
}

// ---------------------------------------------------------------------------
// Keyword-anchored assignment rule
// ---------------------------------------------------------------------------

/// Credential names that are unambiguous in a source tree.
///
/// `token` and `credential` are deliberately *absent*: this workspace is an IDE
/// with lexers, parsers, and tokenizers, so `token`-named bindings are common and
/// almost never credentials. They are matched by the weak pattern instead, which
/// demands a stricter value shape.
const STRONG_KEYWORD_PATTERN: &str = concat!(
    r#"(?i)[a-z0-9_.\-]{0,40}"#,
    r#"(?:password|passwd|client[_.\-]?secret|private[_.\-]?key"#,
    r#"|api[_.\-]?key|apikey|access[_.\-]?key|secret[_.\-]?key|secret)"#,
    r#"[a-z0-9_.\-]{0,24}["']?\s*[:=]\s*["']?([A-Za-z0-9_\-./+=:~]{8,200})"#,
);

/// Credential-adjacent names that are common in non-credential code.
const WEAK_KEYWORD_PATTERN: &str = concat!(
    r#"(?i)[a-z0-9_.\-]{0,40}"#,
    r#"(?:token|credential|auth[_.\-]?key|session[_.\-]?key|bearer)"#,
    r#"[a-z0-9_.\-]{0,24}["']?\s*[:=]\s*["']?([A-Za-z0-9_\-./+=:~]{8,200})"#,
);

fn credential_assignment_rules() -> &'static (Regex, Regex) {
    static RULES: OnceLock<(Regex, Regex)> = OnceLock::new();
    RULES.get_or_init(|| {
        (
            Regex::new(STRONG_KEYWORD_PATTERN).expect("strong keyword pattern must compile"),
            Regex::new(WEAK_KEYWORD_PATTERN).expect("weak keyword pattern must compile"),
        )
    })
}

fn scan_credential_assignments(text: &str, findings: &mut Vec<SecretFinding>) {
    let (strong, weak) = credential_assignment_rules();
    for (regex, strong_keyword) in [(strong, true), (weak, false)] {
        for captures in regex.captures_iter(text) {
            let Some(value) = captures.get(1) else {
                continue;
            };
            let quoted = value
                .start()
                .checked_sub(1)
                .and_then(|index| text.as_bytes().get(index))
                .is_some_and(|byte| *byte == b'"' || *byte == b'\'');
            if !is_credential_like_value(value.as_str(), quoted, strong_keyword) {
                continue;
            }
            push_non_overlapping(
                findings,
                SecretFinding {
                    rule_id: SecretRuleId::GenericSecretAssignment,
                    confidence: SecretConfidence::Contextual,
                    severity: SecretSeverity::Medium,
                    span: SecretSpan {
                        start: value.start(),
                        end: value.end(),
                    },
                },
            );
        }
    }
}

/// Decides whether the right-hand side of a credential-named assignment looks
/// like a literal credential rather than a placeholder, a reference, a digest, or
/// a human-readable name.
///
/// `quoted` records whether the value was directly preceded by a quote character,
/// which is strong evidence that it is a literal rather than an expression.
pub fn is_credential_like_value(value: &str, quoted: bool, strong_keyword: bool) -> bool {
    if !(8..=200).contains(&value.len()) {
        return false;
    }
    if !is_not_placeholder_value(value) {
        return false;
    }
    if is_indirection_reference(value) {
        return false;
    }
    // A digest of a credential is not a credential. This workspace stores SHA-256
    // digests in `FileFingerprint`, `content_hash`, and manifest integrity fields;
    // flagging them would make the scanner unusable here. The cost is a false
    // negative on hex-only credentials of exactly a digest length.
    if is_digest_shaped(value) || is_uuid_shaped(value) {
        return false;
    }
    // `PROVIDER_SECRET_SERVICE = "legion-ai-providers"` is a real constant in this
    // workspace. A separator-delimited run of purely alphabetic words is a name,
    // not a key.
    if is_separator_delimited_word_phrase(value) {
        return false;
    }

    let has_upper = value.bytes().any(|byte| byte.is_ascii_uppercase());
    let has_lower = value.bytes().any(|byte| byte.is_ascii_lowercase());
    let has_digit = value.bytes().any(|byte| byte.is_ascii_digit());

    // An explicitly quoted literal under an unambiguous credential name is taken
    // at face value: `password = "correcthorsebattery"` is a secret even though it
    // has no digits and low entropy.
    if strong_keyword && quoted {
        return true;
    }
    if has_upper && has_lower && has_digit && value.len() >= 12 {
        return true;
    }
    value.len() >= 20 && shannon_entropy_bits_per_char(value) >= 4.0
}

// ---------------------------------------------------------------------------
// Entropy heuristic
// ---------------------------------------------------------------------------

/// Matches a maximal run of credential-plausible characters.
///
/// `=` is deliberately excluded even though it is base64 padding. Including it
/// merged an identifier with its value across an assignment
/// (`CONTENT_HASH=<64 hex>` became one 77-character candidate), which let an
/// uppercase identifier prefix supply the uppercase character class that the
/// lowercase-hex value on its own could never have. Splitting on `=` means each
/// side is judged on its own shape. Trailing padding is trimmed below.
fn entropy_candidate_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9+/_\-]{32,512}").expect("entropy candidate pattern must compile")
    })
}

fn scan_high_entropy_tokens(text: &str, findings: &mut Vec<SecretFinding>) {
    for candidate in entropy_candidate_regex().find_iter(text) {
        let token = candidate.as_str().trim_matches(['-', '_']);
        if !is_high_entropy_credential_candidate(token) {
            continue;
        }
        push_non_overlapping(
            findings,
            SecretFinding {
                rule_id: SecretRuleId::HighEntropyToken,
                confidence: SecretConfidence::Heuristic,
                severity: SecretSeverity::Medium,
                span: SecretSpan {
                    start: candidate.start(),
                    end: candidate.end(),
                },
            },
        );
    }
}

/// Returns true when a bare token is entropic enough, and structurally unlike
/// enough to this workspace's identifiers and digests, to be worth flagging.
///
/// See the module documentation for why every condition here is required.
pub fn is_high_entropy_credential_candidate(token: &str) -> bool {
    if !(HIGH_ENTROPY_MIN_LEN..=HIGH_ENTROPY_MAX_LEN).contains(&token.len()) {
        return false;
    }
    let has_upper = token.bytes().any(|byte| byte.is_ascii_uppercase());
    let has_lower = token.bytes().any(|byte| byte.is_ascii_lowercase());
    let has_digit = token.bytes().any(|byte| byte.is_ascii_digit());
    if !(has_upper && has_lower && has_digit) {
        return false;
    }
    if is_digest_shaped(token) || is_uuid_shaped(token) {
        return false;
    }
    // `is_structured_identifier_path` subsumes `is_separator_delimited_word_phrase`
    // here: an all-alphabetic segmented value satisfies both.
    if is_structured_identifier_path(token) {
        return false;
    }
    shannon_entropy_bits_per_char(token) >= HIGH_ENTROPY_BITS_PER_CHAR
}

// ---------------------------------------------------------------------------
// Shape helpers
// ---------------------------------------------------------------------------

/// Shannon entropy of `value` in bits per byte.
pub fn shannon_entropy_bits_per_char(value: &str) -> f64 {
    if value.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for byte in value.bytes() {
        counts[byte as usize] += 1;
    }
    let total = value.len() as f64;
    counts
        .iter()
        .filter(|count| **count > 0)
        .map(|count| {
            let probability = *count as f64 / total;
            -probability * probability.log2()
        })
        .sum()
}

/// Returns true when `value` is all-hex at a canonical digest length.
///
/// Lengths cover MD5 (32), SHA-1 and git object ids (40), SHA-224 (56),
/// SHA-256 (64), SHA-384 (96), and SHA-512 (128).
pub fn is_digest_shaped(value: &str) -> bool {
    const DIGEST_LENGTHS: [usize; 6] = [32, 40, 56, 64, 96, 128];
    DIGEST_LENGTHS.contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Returns true when `value` is a hyphenated 8-4-4-4-12 hexadecimal UUID.
pub fn is_uuid_shaped(value: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut groups = value.split('-');
    for expected in GROUPS {
        match groups.next() {
            Some(group)
                if group.len() == expected
                    && group.bytes().all(|byte| byte.is_ascii_hexdigit()) => {}
            _ => return false,
        }
    }
    groups.next().is_none()
}

/// Splits `value` on the separators that delimit identifier and path segments.
fn identifier_segments(value: &str) -> Vec<&str> {
    value
        .split(['-', '_', '.', '/', ':', '~', '+', '='])
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// Returns true when `value` is two or more separator-delimited alphabetic words.
///
/// This is the shape of identifiers, kebab-case names, and dotted module paths —
/// `legion-ai-providers`, `assemble.context.manifest` — none of which are keys.
///
/// This predicate is deliberately strict: it gates the *contextual* assignment
/// rule, where the surrounding text already named a credential, so widening it
/// would suppress real findings. The entropy heuristic uses the broader
/// [`is_structured_identifier_path`] instead.
pub fn is_separator_delimited_word_phrase(value: &str) -> bool {
    let segments = identifier_segments(value);
    segments.len() >= 2
        && segments
            .iter()
            .all(|segment| segment.bytes().all(|byte| byte.is_ascii_alphabetic()))
}

/// Returns true when `value` is a separator-delimited path or identifier in which
/// every segment keeps its digits at the segment's edges.
///
/// This is the dominant non-secret high-entropy shape in this workspace, and
/// [`is_separator_delimited_word_phrase`] does not catch any of it. Plan paths,
/// evidence identifiers, ADR references, and run identifiers all interleave
/// numeric and alphabetic segments:
///
/// * `planning/phases/05-control-trust-and-assisted-ai-surfaces/05-01-RESULT`
/// * `plans/evidence/production/M5/WS18-T3-platform-parity-matrix`
/// * `20260602T182617_rebaseline_product_surface_gates`
///
/// Each is over 32 characters, mixes case because of an uppercase tag, and
/// contains digits because of a phase, workstream, or timestamp number — so each
/// satisfies every other entropy precondition. Measured against this repository,
/// this shape accounted for 5042 of 5053 findings before this predicate existed.
///
/// The discriminator is *where the digits sit*. Human-authored identifiers put
/// digits at a segment's start or end (`WS18`, `T3`, `05`, `20260602T182617`),
/// leaving an alphabetic core. Random credential material interleaves them
/// (`A1b2C3d4`), so stripping the leading and trailing digit runs still leaves
/// digits behind. `ghp_` bodies, `sk-proj-` bodies, and base64 blobs are all
/// unaffected by this predicate.
pub fn is_structured_identifier_path(value: &str) -> bool {
    let segments = identifier_segments(value);
    if segments.len() < 2 {
        return false;
    }
    let mut has_alphabetic_core = false;
    for segment in &segments {
        if !is_identifier_segment(segment) {
            return false;
        }
        has_alphabetic_core |= segment.bytes().any(|byte| byte.is_ascii_alphabetic());
    }
    has_alphabetic_core
}

/// Maximum length at which a segment is too short to carry credential entropy.
///
/// Four characters over the 62-symbol alphanumeric alphabet is under 24 bits.
/// Abbreviations of this length (`E2E`, `i18n`, `a11y`, `k8s`, `M5`) are common in
/// paths and defeat the digit-edge test because their digits sit in the middle.
///
/// # Accepted false negative
///
/// A credential formatted as short hyphenated groups —
/// `A1B2-C3D4-E5F6-G7H8-I9J0-K1L2-M3N4-O5P6`, the shape of a product licence key —
/// has every segment under this length and is therefore excluded from the entropy
/// heuristic. This is accepted rather than fixed: the rule is `Heuristic` and
/// applies only under [`ScanPosture::EgressRecall`], and the credentials that
/// actually matter here (provider API keys and bearer tokens) are unformatted and
/// are covered by structural rules. Lowering this constant to 3 would recover the
/// licence-key shape at the cost of re-admitting `E2E`, which is the far more
/// common string in this repository.
const MAX_OPAQUE_SEGMENT_LEN: usize = 4;

/// Returns true when one path or identifier segment carries no credential signal.
fn is_identifier_segment(segment: &str) -> bool {
    // A segment short enough to be an abbreviation cannot hold a credential.
    if segment.len() <= MAX_OPAQUE_SEGMENT_LEN {
        return true;
    }
    // A git object id or content digest embedded in a path — a permalink URL, a
    // build artifact directory — is a digest wherever it appears. `is_digest_shaped`
    // only sees whole values, so it is applied again per segment here.
    if is_digest_shaped(segment) {
        return true;
    }
    // Otherwise the digits must sit at the segment's edges, leaving an
    // alphabetic core.
    segment
        .trim_start_matches(|character: char| character.is_ascii_digit())
        .trim_end_matches(|character: char| character.is_ascii_digit())
        .bytes()
        .all(|byte| byte.is_ascii_alphabetic())
}

/// Returns true when `value` is not a documentation placeholder.
///
/// Redacting `<your-api-key>` teaches reviewers that the marker is noise, which
/// is the specific failure mode this scanner must avoid.
pub fn is_not_placeholder_value(value: &str) -> bool {
    const PLACEHOLDERS: [&str; 18] = [
        "changeme",
        "change_me",
        "dummy",
        "example",
        "fake",
        "insert",
        "none",
        "notset",
        "not_set",
        "placeholder",
        "redacted",
        "replaceme",
        "sample",
        "test_value",
        "todo",
        "unset",
        "xxxxxxxx",
        "your",
    ];
    let lower = value.to_ascii_lowercase();
    if PLACEHOLDERS
        .iter()
        .any(|placeholder| lower.contains(placeholder))
    {
        return false;
    }
    // A run of one repeated character (`********`, `xxxxxxxx`, `00000000`) is a
    // mask, not a credential.
    let mut characters = value.chars();
    if let Some(first) = characters.next()
        && characters.all(|character| character == first)
    {
        return false;
    }
    true
}

/// Returns true when `value` reads as an indirection rather than a literal.
///
/// `api_key = process.env.API_KEY` names a credential without containing one.
pub fn is_indirection_reference(value: &str) -> bool {
    const REFERENCES: [&str; 10] = [
        "env.",
        "env:",
        "environ",
        "getenv",
        "config.",
        "settings.",
        "secrets.",
        "vault.",
        "self.",
        "this.",
    ];
    let lower = value.to_ascii_lowercase();
    REFERENCES.iter().any(|reference| lower.contains(reference))
}

/// Returns true when a prefixed token body looks opaque rather than descriptive.
fn is_opaque_credential_token(value: &str) -> bool {
    if is_separator_delimited_word_phrase(value) {
        return false;
    }
    let has_digit = value.bytes().any(|byte| byte.is_ascii_digit());
    let has_alpha = value.bytes().any(|byte| byte.is_ascii_alphabetic());
    has_digit && has_alpha && shannon_entropy_bits_per_char(value) >= 4.0
}

// ---------------------------------------------------------------------------
// Proposal surface
// ---------------------------------------------------------------------------

/// One scanned location inside a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalSecretSite {
    /// Display-safe label naming the location, never the content.
    pub site_label: String,
    /// Findings at that location.
    pub report: SecretScanReport,
}

/// Result of scanning every text-bearing field of a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalSecretScan {
    /// Proposal that was scanned.
    pub proposal_id: ProposalId,
    /// Sites that produced at least one finding.
    pub sites: Vec<ProposalSecretSite>,
}

impl ProposalSecretScan {
    /// Returns true when no site produced a finding.
    pub fn is_clean(&self) -> bool {
        self.sites.is_empty()
    }

    /// Returns true when the posture requires redaction at any site.
    pub fn requires_redaction(&self, posture: ScanPosture) -> bool {
        self.sites
            .iter()
            .any(|site| site.report.requires_redaction(posture))
    }

    /// Returns the distinct rule identifiers that fired across all sites.
    pub fn rule_ids(&self) -> Vec<SecretRuleId> {
        let mut ids: Vec<SecretRuleId> = self
            .sites
            .iter()
            .flat_map(|site| site.report.rule_ids())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

/// Scans every text-bearing field of a proposal, including nested batch items.
///
/// Proposal payloads are the one place where model-authored bytes are staged for
/// human review before they touch disk. A credential pasted into an edit
/// replacement, a created file's initial content, or a terminal command's
/// environment reaches the reviewer's screen and then the working tree, so it is
/// scanned before either happens.
pub fn scan_proposal_for_secrets(proposal: &WorkspaceProposal) -> ProposalSecretScan {
    let mut sites = Vec::new();
    scan_preview_site(&mut sites, "preview.summary", &proposal.preview.summary);
    for (index, detail) in proposal.preview.details.iter().enumerate() {
        scan_preview_site(&mut sites, &format!("preview.details[{index}]"), detail);
    }
    scan_payload_sites(&mut sites, "payload", &proposal.payload);

    ProposalSecretScan {
        proposal_id: proposal.proposal_id,
        sites,
    }
}

/// Scans a proposal payload that has not yet been wrapped in an envelope.
///
/// Producers build the payload before the [`WorkspaceProposal`] exists, so this
/// is the entry point they can call at construction time rather than after the
/// content has already been committed to a proposal record.
pub fn scan_proposal_payload_for_secrets(payload: &ProposalPayload) -> Vec<ProposalSecretSite> {
    let mut sites = Vec::new();
    scan_payload_sites(&mut sites, "payload", payload);
    sites
}

fn scan_preview_site(sites: &mut Vec<ProposalSecretSite>, label: &str, text: &str) {
    let report = scan_text_for_secrets(text);
    if !report.is_clean() {
        sites.push(ProposalSecretSite {
            site_label: label.to_string(),
            report,
        });
    }
}

fn scan_payload_sites(
    sites: &mut Vec<ProposalSecretSite>,
    prefix: &str,
    payload: &ProposalPayload,
) {
    match payload {
        ProposalPayload::TextEdit(edit) => {
            for (index, text_edit) in edit.edits.edits.iter().enumerate() {
                scan_preview_site(
                    sites,
                    &format!("{prefix}.text_edit.edits[{index}].replacement"),
                    &text_edit.replacement,
                );
            }
        }
        ProposalPayload::CreateFile(create) => {
            if let Some(content) = create.initial_content.as_deref() {
                let label = format!("{prefix}.create_file.initial_content");
                scan_preview_site(sites, &label, content);
            }
        }
        ProposalPayload::CodeAction(action) => {
            scan_preview_site(sites, &format!("{prefix}.code_action.title"), &action.title);
            for (index, text_edit) in action.edits.iter().enumerate() {
                scan_preview_site(
                    sites,
                    &format!("{prefix}.code_action.edits[{index}].replacement"),
                    &text_edit.replacement,
                );
            }
        }
        ProposalPayload::WorkspaceEdit(edit) => {
            scan_preview_site(
                sites,
                &format!("{prefix}.workspace_edit.title"),
                &edit.title,
            );
            for (file_index, file_edit) in edit.file_edits.iter().enumerate() {
                for (index, text_edit) in file_edit.edits.edits.iter().enumerate() {
                    scan_preview_site(
                        sites,
                        &format!(
                            "{prefix}.workspace_edit.file_edits[{file_index}].edits[{index}].replacement"
                        ),
                        &text_edit.replacement,
                    );
                }
            }
        }
        ProposalPayload::TerminalCommand(command) => {
            scan_preview_site(
                sites,
                &format!("{prefix}.terminal_command.command"),
                &command.command,
            );
            let mut env_names: Vec<&String> = command.env.keys().collect();
            env_names.sort();
            for name in env_names {
                let value = &command.env[name];
                // The assignment is reconstructed so the keyword-anchored rule can
                // see `NAME=value`; a bare value would lose the naming context that
                // makes the contextual rule safe to trust.
                scan_preview_site(
                    sites,
                    &format!("{prefix}.terminal_command.env[{name}]"),
                    &format!("{name}={value}"),
                );
            }
        }
        ProposalPayload::Batch(batch) => {
            for item in &batch.items {
                scan_payload_sites(
                    sites,
                    &format!("{prefix}.batch.items[{}]", item.item_id),
                    &item.payload,
                );
            }
        }
        ProposalPayload::DeleteFile(_)
        | ProposalPayload::RenameFile(_)
        | ProposalPayload::SaveFile(_)
        | ProposalPayload::FormatFile(_) => {
            // Metadata-only payloads: identities, versions, and option maps. No
            // caller-authored free text reaches these variants.
        }
    }
}
