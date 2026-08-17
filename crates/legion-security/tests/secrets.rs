//! Fixture-per-rule coverage for the secret scanner.
//!
//! Every fixture credential in this file is **generated at run time** from a
//! deterministic pseudo-random sequence rather than written as a string literal.
//! A committed literal that has the shape of a real key is exactly what the
//! repository's pre-commit secret scanning exists to stop, and a test suite that
//! has to be added to a scanner allowlist is a test suite people eventually turn
//! off. Generating the bodies also makes cross-rule contamination structurally
//! impossible: [`mixed_class_body`] never emits two adjacent letters of the same
//! case, so no generated body can accidentally contain `AKIA`, `eyJ`, `AIza`, or
//! `xox` and be claimed by the wrong rule.

use std::collections::HashMap;

use legion_protocol::{
    CanonicalPath, CapabilityId, CorrelationId, CreateFileProposal, PreviewSummary, PrincipalId,
    ProposalBatchAtomicity, ProposalBatchItem, ProposalBatchRollbackPolicy, ProposalId,
    ProposalPayload, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalVersionPreconditions, RedactionHint, TerminalCommandProposal, TimestampMillis,
    WorkspaceProposal,
};
use legion_security::secrets::{
    HIGH_ENTROPY_BITS_PER_CHAR, is_digest_shaped, is_indirection_reference,
    is_not_placeholder_value, is_separator_delimited_word_phrase, is_structured_identifier_path,
    is_uuid_shaped, redact_secrets_in_text, shannon_entropy_bits_per_char,
};
use legion_security::{
    RedactionPayloadKind, ScanPosture, SecretConfidence, SecretRuleId,
    scan_payload_for_sensitive_markers, scan_proposal_for_secrets, scan_text_for_secrets,
};

// ---------------------------------------------------------------------------
// Deterministic fixture generation
// ---------------------------------------------------------------------------

/// Advances a 64-bit LCG. Deterministic across platforms and Rust versions,
/// unlike `DefaultHasher`, so fixtures never differ between developer machines
/// and CI.
fn next_state(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state >> 33
}

/// Generates a body guaranteed to contain uppercase, lowercase, and digits.
///
/// The class cycle is `U, L, D, U, L`, so two letters of the same case are never
/// adjacent. That is what makes the generated bodies unable to spell any other
/// rule's prefix.
fn mixed_class_body(len: usize, seed: u64) -> String {
    const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const DIGIT: &[u8] = b"0123456789";
    let mut state = seed | 1;
    let mut out = String::with_capacity(len);
    for index in 0..len {
        let draw = next_state(&mut state) as usize;
        let class: &[u8] = match index % 5 {
            0 | 3 => UPPER,
            1 | 4 => LOWER,
            _ => DIGIT,
        };
        out.push(class[draw % class.len()] as char);
    }
    out
}

/// Generates an uppercase-alphanumeric body, the AWS access key id charset.
fn upper_alnum_body(len: usize, seed: u64) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut state = seed | 1;
    (0..len)
        .map(|_| ALPHABET[next_state(&mut state) as usize % ALPHABET.len()] as char)
        .collect()
}

/// Generates a lowercase hexadecimal body, the shape of every digest in this
/// workspace (`FileFingerprint`, `content_hash`, git object ids, lockfile
/// checksums).
fn hex_body(len: usize, seed: u64) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdef";
    let mut state = seed | 1;
    (0..len)
        .map(|_| ALPHABET[next_state(&mut state) as usize % ALPHABET.len()] as char)
        .collect()
}

/// Generates a lowercase hyphenated UUID, the shape of `CorrelationId` and
/// `CausalityId` renderings.
fn uuid_body(seed: u64) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        hex_body(8, seed),
        hex_body(4, seed + 1),
        hex_body(4, seed + 2),
        hex_body(4, seed + 3),
        hex_body(12, seed + 4)
    )
}

// ---------------------------------------------------------------------------
// Positive fixtures: one per rule
// ---------------------------------------------------------------------------

/// Returns one structurally correct but entirely synthetic fixture per rule.
fn positive_fixtures() -> Vec<(SecretRuleId, String)> {
    vec![
        (
            SecretRuleId::AwsAccessKeyId,
            format!(
                "deployment role uses AKIA{} in staging",
                upper_alnum_body(16, 11)
            ),
        ),
        (
            SecretRuleId::AwsSecretAccessKey,
            format!("aws_secret_access_key={}", mixed_class_body(40, 12)),
        ),
        (
            SecretRuleId::GithubToken,
            format!("checked out with ghp_{} ok", mixed_class_body(36, 13)),
        ),
        (
            SecretRuleId::GithubFineGrainedToken,
            format!("actions runner github_pat_{} ok", mixed_class_body(60, 14)),
        ),
        (
            SecretRuleId::GitlabPersonalAccessToken,
            format!("mirror configured glpat-{} ok", mixed_class_body(20, 15)),
        ),
        (
            SecretRuleId::SlackToken,
            format!("bot connected xoxb-{} ok", mixed_class_body(24, 16)),
        ),
        (
            SecretRuleId::SlackWebhookUrl,
            format!(
                "alert sink https://hooks.slack.com/services/{} ok",
                mixed_class_body(24, 17)
            ),
        ),
        (
            SecretRuleId::OpenAiApiKey,
            format!("provider configured sk-{} ok", mixed_class_body(48, 18)),
        ),
        (
            SecretRuleId::AnthropicApiKey,
            format!(
                "provider configured sk-ant-api03-{} ok",
                mixed_class_body(32, 19)
            ),
        ),
        (
            SecretRuleId::StripeSecretKey,
            format!("billing configured sk_live_{} ok", mixed_class_body(24, 20)),
        ),
        (
            SecretRuleId::GoogleApiKey,
            format!("maps client AIza{} ok", mixed_class_body(35, 21)),
        ),
        (
            SecretRuleId::NpmAccessToken,
            format!("publish uses npm_{} ok", mixed_class_body(36, 22)),
        ),
        (
            SecretRuleId::PemPrivateKeyBlock,
            // Assembled so no complete PEM header literal is committed.
            format!("-----BEGIN {} PRIVATE KEY-----", "RSA"),
        ),
        (
            SecretRuleId::JsonWebToken,
            format!(
                "session eyJ{}.eyJ{}.{} ok",
                mixed_class_body(20, 23),
                mixed_class_body(40, 24),
                mixed_class_body(43, 25)
            ),
        ),
        (
            SecretRuleId::HttpAuthorizationHeader,
            format!("Authorization: Bearer {}", mixed_class_body(32, 26)),
        ),
        (
            SecretRuleId::UrlEmbeddedCredentials,
            format!(
                "https://ci-bot:{}@git.internal.invalid/repo.git",
                mixed_class_body(20, 27)
            ),
        ),
        (
            SecretRuleId::GenericSecretAssignment,
            format!("client_secret = \"{}\"", mixed_class_body(24, 28)),
        ),
        (
            SecretRuleId::HighEntropyToken,
            format!("artifact reference {} recorded", mixed_class_body(48, 29)),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Negative fixtures: real strings from this workspace
// ---------------------------------------------------------------------------

/// Returns strings taken from, or shaped exactly like, real content in this
/// workspace. A scanner that fires on any of these is unusable here.
fn workspace_negative_fixtures() -> Vec<(&'static str, String)> {
    vec![
        (
            "sha256 content hash assignment",
            format!("content_hash = \"{}\"", hex_body(64, 41)),
        ),
        ("git object id", format!("commit {}", hex_body(40, 42))),
        (
            "cargo lockfile checksum",
            format!("checksum = \"{}\"", hex_body(64, 43)),
        ),
        (
            "correlation uuid rendering",
            format!("causality_id = {}", uuid_body(44)),
        ),
        (
            "file fingerprint record",
            format!(
                "FileFingerprint {{ algorithm: \"sha256\", value: \"{}\" }}",
                hex_body(64, 45)
            ),
        ),
        (
            "fnv offset basis constant",
            "const FNV_OFFSET: u64 = 0xcbf29ce484222325;".to_string(),
        ),
        (
            "deterministic manifest identifier",
            format!("manifest:{}", hex_body(16, 46)),
        ),
        (
            "build artifact hash suffix",
            format!("legion_security-{}", hex_body(16, 47)),
        ),
        (
            "provider secret service constant declaration",
            "const PROVIDER_SECRET_SERVICE: &str = \"legion-ai-providers\";".to_string(),
        ),
        (
            "provider secret service constant assignment",
            "PROVIDER_SECRET_SERVICE = \"legion-ai-providers\"".to_string(),
        ),
        (
            "rust api key field declaration",
            "pub api_key: Option<String>,".to_string(),
        ),
        (
            "rust password field declaration",
            "pub password: String,".to_string(),
        ),
        (
            "lexer token binding",
            "let token = lexer.next_token();".to_string(),
        ),
        (
            "tokenizer configuration name",
            "tokenizer_config = \"gpt2-medium-fast\"".to_string(),
        ),
        (
            "environment indirection",
            "api_key = process.env.OPENAI_API_KEY".to_string(),
        ),
        (
            "documentation placeholder",
            "api_key = \"your-api-key-here\"".to_string(),
        ),
        (
            "long camel case type name",
            "AssistedAiTrustProjectionReferenceCollectionBuilder".to_string(),
        ),
        (
            "cipher type name with digits",
            "ChaCha20Poly1305VaultCipher".to_string(),
        ),
        (
            "long snake case test name",
            "dto_contracts_phase8_terminal_and_transport_audits_reject_raw_markers".to_string(),
        ),
        (
            "workspace relative module path",
            "crates/legion-security/src/secrets.rs".to_string(),
        ),
        (
            "evidence identifier",
            "legion-evidence:worker:local:external-log:build.log".to_string(),
        ),
        (
            "vault bundle and lease identifiers",
            "bundle:9:901 lease:9:901".to_string(),
        ),
        (
            "ordinary terminal output",
            "Compiling legion-security v0.1.0 (crates/legion-security)".to_string(),
        ),
        // The fixtures below were not chosen by inspection. They are verbatim
        // strings taken from a scan of this repository, which produced 5053
        // findings before `is_structured_identifier_path` existed — 5042 of them
        // this one shape. Every entry stands for a distinct reason the entropy
        // rule fired, so deleting any one of them re-opens a distinct hole.
        (
            "phase plan result path with numeric segments",
            "planning/phases/05-control-trust-and-assisted-ai-surfaces/05-01-RESULT".to_string(),
        ),
        (
            "adr reference with a zero-padded number",
            "plans/adrs/ADR-0019-legion-workflow-orchestration".to_string(),
        ),
        (
            "workstream evidence path with alphanumeric tags",
            "plans/evidence/production/M5/WS18-T3-platform-parity-matrix".to_string(),
        ),
        (
            "compact timestamp prefixed evidence identifier",
            "plans/evidence/legion-e2e/20260602T182617_rebaseline_product_surface_gates"
                .to_string(),
        ),
        (
            "path containing a letter-digit-letter abbreviation",
            "plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN".to_string(),
        ),
        (
            "permalink url embedding a git object id",
            format!(
                "https://github.com/example/legion-ide/blob/{}/crates/legion-desktop/src/beta.rs",
                hex_body(40, 48)
            ),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Rule coverage
// ---------------------------------------------------------------------------

#[test]
fn every_declared_rule_has_a_fixture() {
    let covered: Vec<SecretRuleId> = positive_fixtures()
        .into_iter()
        .map(|(rule_id, _)| rule_id)
        .collect();
    for rule_id in SecretRuleId::all() {
        assert!(
            covered.contains(rule_id),
            "rule `{}` has no fixture; a rule without a fixture must not ship",
            rule_id.stable_id()
        );
    }
    assert_eq!(
        covered.len(),
        SecretRuleId::all().len(),
        "fixture table and rule table must stay one-to-one"
    );
}

#[test]
fn each_rule_fixture_is_detected_by_its_own_rule() {
    for (rule_id, fixture) in positive_fixtures() {
        let report = scan_text_for_secrets(&fixture);
        assert!(
            report.rule_ids().contains(&rule_id),
            "fixture for `{}` produced {:?} instead",
            rule_id.stable_id(),
            report
                .rule_ids()
                .iter()
                .map(|id| id.stable_id())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn no_workspace_string_produces_a_finding() {
    for (label, fixture) in workspace_negative_fixtures() {
        let report = scan_text_for_secrets(&fixture);
        assert!(
            report.is_clean(),
            "`{label}` is real workspace content and must not be flagged; got {:?}",
            report
                .rule_ids()
                .iter()
                .map(|id| id.stable_id())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn generated_fixture_bodies_clear_the_entropy_floor() {
    // Guards the fixture generator itself: if a future seed change produced a
    // low-entropy body, the detection tests above would fail for a reason that
    // has nothing to do with the rules. This fails first, and says why.
    let body = mixed_class_body(48, 29);
    let entropy = shannon_entropy_bits_per_char(&body);
    assert!(
        entropy >= HIGH_ENTROPY_BITS_PER_CHAR,
        "generated fixture entropy {entropy} is below the {HIGH_ENTROPY_BITS_PER_CHAR} floor"
    );
}

// ---------------------------------------------------------------------------
// Posture
// ---------------------------------------------------------------------------

#[test]
fn display_posture_keeps_heuristic_findings_out_of_redaction() {
    let text = format!("artifact reference {} recorded", mixed_class_body(48, 29));
    let report = scan_text_for_secrets(&text);

    let all_heuristic = report
        .findings
        .iter()
        .all(|finding| finding.confidence == SecretConfidence::Heuristic);
    assert!(
        all_heuristic,
        "this fixture must exercise the heuristic tier only"
    );
    assert!(!report.requires_redaction(ScanPosture::DisplayPrecision));
    assert!(report.requires_redaction(ScanPosture::EgressRecall));

    let displayed = redact_secrets_in_text(&text, ScanPosture::DisplayPrecision);
    assert!(!displayed.redacted);
    assert_eq!(displayed.text, text);
}

#[test]
fn egress_posture_redacts_heuristic_findings() {
    let body = mixed_class_body(48, 29);
    let text = format!("artifact reference {body} recorded");

    let egressed = redact_secrets_in_text(&text, ScanPosture::EgressRecall);

    assert!(egressed.redacted);
    assert!(!egressed.text.contains(&body));
    assert!(egressed.text.contains("[redacted]"));
    assert!(egressed.text.starts_with("artifact reference "));
}

#[test]
fn structural_findings_redact_under_every_posture() {
    for (rule_id, fixture) in positive_fixtures() {
        if rule_id == SecretRuleId::HighEntropyToken {
            continue;
        }
        let report = scan_text_for_secrets(&fixture);
        assert!(
            report.requires_redaction(ScanPosture::DisplayPrecision),
            "`{}` must redact even on a human-facing surface",
            rule_id.stable_id()
        );
    }
}

#[test]
fn redaction_removes_the_entire_credential_value() {
    let body = mixed_class_body(36, 13);
    let text = format!("checked out with ghp_{body} ok");

    let redacted = redact_secrets_in_text(&text, ScanPosture::DisplayPrecision);

    // The unique suffix must not survive: redacting only the `ghp_` prefix would
    // leave the whole secret recoverable.
    assert!(!redacted.text.contains(&body));
    assert!(!redacted.text.contains("ghp_"));
    assert!(redacted.text.starts_with("checked out with "));
    assert!(redacted.text.ends_with(" ok"));
}

// ---------------------------------------------------------------------------
// Shape helpers
// ---------------------------------------------------------------------------

#[test]
fn digest_and_uuid_shapes_are_recognized() {
    for length in [32usize, 40, 56, 64, 96, 128] {
        assert!(
            is_digest_shaped(&hex_body(length, 60 + length as u64)),
            "{length}-character hex must be treated as a digest"
        );
    }
    assert!(!is_digest_shaped(&mixed_class_body(64, 61)));
    assert!(is_uuid_shaped(&uuid_body(62)));
    assert!(!is_uuid_shaped(&hex_body(32, 63)));
}

#[test]
fn word_phrase_placeholder_and_indirection_shapes_are_recognized() {
    assert!(is_separator_delimited_word_phrase("legion-ai-providers"));
    assert!(is_separator_delimited_word_phrase(
        "assemble.context.manifest"
    ));
    assert!(!is_separator_delimited_word_phrase("sk-proj-A1b2C3d4"));
    assert!(!is_separator_delimited_word_phrase("singleword"));
    // The strict word-phrase predicate does not see numeric segments, which is
    // exactly why the entropy rule needs the broader one.
    assert!(!is_separator_delimited_word_phrase(
        "plans/adrs/ADR-0019-legion-workflow-orchestration"
    ));

    assert!(!is_not_placeholder_value("your-api-key-here"));
    assert!(!is_not_placeholder_value("changeme"));
    assert!(!is_not_placeholder_value("xxxxxxxxxxxx"));
    assert!(is_not_placeholder_value(&mixed_class_body(24, 64)));

    assert!(is_indirection_reference("process.env.OPENAI_API_KEY"));
    assert!(is_indirection_reference("os.getenv"));
    assert!(!is_indirection_reference(&mixed_class_body(24, 65)));
}

#[test]
fn structured_identifier_paths_are_recognized() {
    // Digits at a segment's edges leave an alphabetic core: a human-authored name.
    for identifier in [
        "planning/phases/05-control-trust-and-assisted-ai-surfaces/05-01-RESULT",
        "plans/evidence/production/M5/WS18-T3-platform-parity-matrix",
        "20260602T182617_rebaseline_product_surface_gates",
        "plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN",
        "crates/legion-security/src/secrets.rs",
    ] {
        assert!(
            is_structured_identifier_path(identifier),
            "`{identifier}` is a workspace identifier and must be excluded from the entropy rule"
        );
    }

    // Digits interleaved through a segment: no alphabetic core survives, so the
    // entropy rule still gets to judge it.
    //
    // The bodies are generated, not written. `mixed_class_body` interleaves
    // classes on a `U, L, D` cycle, which is precisely the property under test —
    // and it keeps a `ghp_`-shaped literal out of the committed source, which is
    // the rule the module doc at the top of this file states.
    for credential in [
        format!("sk-proj-{}", mixed_class_body(16, 73)),
        format!("ghp_{}", mixed_class_body(36, 74)),
        format!("glpat-{}", mixed_class_body(20, 75)),
    ] {
        assert!(
            !is_structured_identifier_path(&credential),
            "a generated credential body must stay visible to the entropy rule"
        );
    }

    // A single segment is not a path, whatever it contains.
    assert!(!is_structured_identifier_path("singleword"));
    assert!(!is_structured_identifier_path(&mixed_class_body(48, 71)));
    // All-numeric segments carry no name, so this is not an identifier either.
    assert!(!is_structured_identifier_path("2026/08/17"));
}

#[test]
fn structured_identifier_exclusion_does_not_swallow_generated_credentials() {
    // The guard must not create a false negative: a generated credential body
    // embedded in an otherwise path-like string is still detected. This is the
    // recall side of the precision fix above, and it is the assertion that would
    // fail first if `is_structured_identifier_path` were ever widened too far.
    let body = mixed_class_body(48, 72);
    let report = scan_text_for_secrets(&format!("plans/evidence/production/{body}"));
    assert!(
        report.rule_ids().contains(&SecretRuleId::HighEntropyToken),
        "a credential body inside a path must still be flagged; got {:?}",
        report
            .rule_ids()
            .iter()
            .map(|id| id.stable_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn shannon_entropy_separates_digests_from_credentials() {
    let digest = hex_body(64, 66);
    let uuid = uuid_body(67);
    let credential = mixed_class_body(48, 68);

    // Hex is capped at 4.0 bits/char by its alphabet, so no digest can reach the
    // floor no matter how random it is.
    assert!(shannon_entropy_bits_per_char(&digest) < HIGH_ENTROPY_BITS_PER_CHAR);
    assert!(shannon_entropy_bits_per_char(&uuid) < HIGH_ENTROPY_BITS_PER_CHAR);
    assert!(shannon_entropy_bits_per_char(&credential) >= HIGH_ENTROPY_BITS_PER_CHAR);
    assert!(shannon_entropy_bits_per_char("") < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// Proposal surface
// ---------------------------------------------------------------------------

fn empty_preconditions() -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: None,
        buffer_version: None,
        snapshot_id: None,
        generation: None,
        file_content_version: None,
        workspace_generation: None,
        expected_fingerprint: None,
        expected_file_length: None,
        expected_modified_at: None,
    }
}

fn proposal_with(payload: ProposalPayload, summary: &str) -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id: ProposalId(4242),
        principal: PrincipalId("principal:test".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(4242),
        payload,
        preconditions: empty_preconditions(),
        preview: PreviewSummary {
            summary: summary.to_string(),
            details: vec!["metadata-only".to_string()],
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

#[test]
fn proposal_create_file_content_is_scanned() {
    let body = mixed_class_body(36, 70);
    let proposal = proposal_with(
        ProposalPayload::CreateFile(CreateFileProposal {
            path: CanonicalPath("/workspace/.env".to_string()),
            initial_content: Some(format!("GITHUB_TOKEN=ghp_{body}\n")),
        }),
        "create configuration file",
    );

    let scan = scan_proposal_for_secrets(&proposal);

    assert!(!scan.is_clean());
    assert!(scan.rule_ids().contains(&SecretRuleId::GithubToken));
    assert!(scan.requires_redaction(ScanPosture::DisplayPrecision));
    assert_eq!(
        scan.sites[0].site_label,
        "payload.create_file.initial_content"
    );
}

#[test]
fn proposal_terminal_command_environment_is_scanned() {
    let mut env = HashMap::new();
    env.insert(
        "AWS_SECRET_ACCESS_KEY".to_string(),
        mixed_class_body(40, 71),
    );
    let proposal = proposal_with(
        ProposalPayload::TerminalCommand(TerminalCommandProposal {
            session_id: None,
            command: "aws s3 sync ./dist s3://bucket".to_string(),
            cwd: None,
            env,
        }),
        "run deployment",
    );

    let scan = scan_proposal_for_secrets(&proposal);

    let expected = "payload.terminal_command.env[AWS_SECRET_ACCESS_KEY]";
    assert!(scan.rule_ids().contains(&SecretRuleId::AwsSecretAccessKey));
    assert!(scan.sites.iter().any(|site| site.site_label == expected));
}

#[test]
fn proposal_preview_summary_is_scanned() {
    let proposal = proposal_with(
        ProposalPayload::CreateFile(CreateFileProposal {
            path: CanonicalPath("/workspace/notes.txt".to_string()),
            initial_content: Some("nothing sensitive here\n".to_string()),
        }),
        &format!("rotate key AKIA{}", upper_alnum_body(16, 72)),
    );

    let scan = scan_proposal_for_secrets(&proposal);

    assert!(scan.rule_ids().contains(&SecretRuleId::AwsAccessKeyId));
    assert_eq!(scan.sites[0].site_label, "preview.summary");
}

#[test]
fn proposal_batch_items_are_scanned_recursively() {
    let body = mixed_class_body(48, 73);
    let nested = ProposalPayload::CreateFile(CreateFileProposal {
        path: CanonicalPath("/workspace/config.toml".to_string()),
        initial_content: Some(format!("openai = \"sk-{body}\"\n")),
    });
    let batch = ProposalPayload::Batch(legion_protocol::BatchProposalPayload {
        batch_id: uuid::Uuid::nil(),
        atomicity: ProposalBatchAtomicity::AllOrNothing,
        rollback_policy: ProposalBatchRollbackPolicy::Required,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: Vec::new(),
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        items: vec![ProposalBatchItem {
            order: 0,
            item_id: "item-0".to_string(),
            payload: Box::new(nested),
            target_ids: Vec::new(),
            required_capability: CapabilityId("fs.write".to_string()),
            rollback_step_ids: Vec::new(),
        }],
        dependency_edges: Vec::new(),
        rollback_steps: Vec::new(),
        partial_failures: Vec::new(),
        preview_warnings: Vec::new(),
        schema_version: 1,
    });

    let scan = scan_proposal_for_secrets(&proposal_with(batch, "batched change"));

    assert!(
        scan.rule_ids().contains(&SecretRuleId::OpenAiApiKey),
        "a credential nested inside a batch item must not evade the scan"
    );
    let expected = "payload.batch.items[item-0].create_file.initial_content";
    assert_eq!(scan.sites[0].site_label, expected);
}

#[test]
fn clean_proposal_produces_no_sites() {
    let proposal = proposal_with(
        ProposalPayload::CreateFile(CreateFileProposal {
            path: CanonicalPath("/workspace/src/lib.rs".to_string()),
            initial_content: Some(
                "//! Deterministic risk rules.\npub fn evaluate() -> bool { true }\n".to_string(),
            ),
        }),
        "add module",
    );

    let scan = scan_proposal_for_secrets(&proposal);

    assert!(scan.is_clean());
    assert!(!scan.requires_redaction(ScanPosture::EgressRecall));
}

// ---------------------------------------------------------------------------
// Integration with the existing marker scanner
// ---------------------------------------------------------------------------

#[test]
fn marker_scanner_now_reports_credentials_the_marker_list_cannot_see() {
    // The pre-existing marker list has no entry that matches an AWS access key
    // id. Before the ruleset landed this payload scanned clean.
    let payload = format!("rotated to AKIA{} today", upper_alnum_body(16, 80));

    let report = scan_payload_for_sensitive_markers(RedactionPayloadKind::Log, &payload);

    assert!(!report.passed());
    assert!(report.redaction_required);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.marker_label == SecretRuleId::AwsAccessKeyId.stable_id())
    );
}

#[test]
fn proposal_content_payload_requires_redaction() {
    let report = scan_payload_for_sensitive_markers(
        RedactionPayloadKind::Trace,
        "proposal_content: serialized workspace proposal payload",
    );

    assert!(!report.passed());
    assert!(report.redaction_required);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.marker_label == "raw-proposal-content")
    );
}

#[test]
fn terminal_excerpt_payload_requires_redaction() {
    let report = scan_payload_for_sensitive_markers(
        RedactionPayloadKind::Log,
        "terminal_excerpts: captured shell excerpt payload",
    );

    assert!(!report.passed());
    assert!(report.redaction_required);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.marker_label == "raw-terminal-excerpts")
    );
}

#[test]
fn retained_and_ejected_context_payload_requires_redaction() {
    let report = scan_payload_for_sensitive_markers(
        RedactionPayloadKind::Diff,
        "retained_context: workspace buffer snapshot\nejected_context: discarded buffer snapshot",
    );

    assert!(!report.passed());
    assert!(report.redaction_required);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.marker_label == "retained-context")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.marker_label == "ejected-context")
    );
}
