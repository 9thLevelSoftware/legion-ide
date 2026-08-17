# Real secret scanning for proposal, terminal, and retained content — 2026-08-17 (P9.F2.T2)

Backlog card: `P9.F2.T2` in `plans/kanban/legion-ga-backlog.toml`.
Roadmap item: Phase 5, item 13 — "regex ruleset + entropy detection replacing
the 4 substring markers in `legion-security/src/lib.rs:303`".

## Status

**Task status: `done`.** The acceptance is "Each secret-detection rule has a
fixture and a test." All 18 rules have a fixture;
`every_declared_rule_has_a_fixture` and
`each_rule_fixture_is_detected_by_its_own_rule` both assert it and both were
watched passing, as were both of the card's own verification commands. Every
required gate was executed — see "Verification".

This document was written in two passes and the second pass corrected the first.
Where a claim changed, both versions are shown rather than the earlier one being
quietly replaced, because "we believed X and measured not-X" is the part worth
keeping. Three corrections, in order of importance:

1. The entropy rule's stated safety argument was wrong, and the rule as first
   written produced **5042 false positives against this repository**. Measured,
   fixed, re-measured to 3. See "Correction: the character-class condition was
   not sufficient".
2. The claim that *every* rule id collides with `contains_forbidden_phase8_payload`
   was wrong — 10 of 18 do, not 18. The count-only annotation it justified is
   still correct, for a slightly different reason.
3. Two of the five "repo-specific false positives" cited as found in this
   workspace do not occur in it. Three of the five do, verbatim.

The claim that the backlog card names a surface that was never built
(`retained_context`/`ejected_context`) was re-checked independently and **holds**.

## What already existed

The roadmap describes "4 substring markers". The actual pre-existing state was
larger than that but not stronger in kind. Three independent, divergent
implementations, all substring-based:

1. `legion-security/src/lib.rs` — `scan_payload_for_sensitive_markers`.
   17 case-insensitive substring markers, of which 11
   (`proposal_content`, `terminal_excerpt`, `retained_context`, `ejected_context`,
   `source_body`, `provider_payload`, `raw prompt`, `terminal output`, ...) are
   *DTO field names*, not credentials. Only 6 are credential-related:
   `-----begin`, `aws_secret_access_key`, `openai_api_key`, `api_key=`,
   `authorization: bearer`, `ghp_`, plus prefix checks for `xoxb-` and `sk-`.
   Detection only — it returns a boolean and offsets, and redacts nothing.
   **Exactly one call site in the workspace**: `legion-ai`.
2. `legion-ai/src/redaction.rs` — `redact_model_bound_output`. Four regexes:
   an `Authorization: Bearer` header, a `KEY=value` assignment for three
   hardcoded key names, the `sk-|xoxb-|ghp_|gho_` prefixes, and a bare-marker
   fallback. Used by `legion-agent` for tool output and evidence summaries.
3. `legion-terminal/src/lib.rs` — `redact_secrets`, a hand-rolled scanner with
   its own copy of the same knowledge (`authorization:` headers, `NAME=value`
   where `NAME` contains one of seven keywords, four token prefixes, three bare
   markers). `legion-terminal` did not depend on `legion-security` at all.
4. `legion-protocol` — `contains_forbidden_phase8_payload` and siblings, a
   substring list applied to DTO debug renderings. This is *schema* validation
   (no raw payload field may appear in a metadata-only record), not credential
   detection, and it is not replaceable by a secret scanner.

What all of this misses: an AWS access key id, an AWS secret that is not next to
its keyword, a GitHub fine-grained token, a GitLab PAT, a Google API key, an npm
token, a Stripe key, a JWT, a Slack webhook URL, credentials in a URL userinfo
component, and any credential in a `NAME=value` pair whose `NAME` is not one of
the hardcoded few. There was no entropy detection anywhere.

## What was built

`crates/legion-security/src/secrets.rs` (new module, pure functions, no I/O):

- 16 structural provider rules, each a regex plus an optional value-shape
  validator: `pem-private-key-block`, `aws-access-key-id`,
  `aws-secret-access-key`, `github-fine-grained-token`, `github-token`,
  `gitlab-personal-access-token`, `slack-webhook-url`, `slack-token`,
  `anthropic-api-key`, `stripe-secret-key`, `openai-api-key`, `google-api-key`,
  `npm-access-token`, `json-web-token`, `http-authorization-header`,
  `url-embedded-credentials`.
- 1 keyword-anchored contextual rule (`generic-secret-assignment`), split into a
  strong-keyword and a weak-keyword pattern with different value-shape bars.
- 1 entropy heuristic (`high-entropy-token`).
- Shape predicates, all public and individually tested:
  `shannon_entropy_bits_per_char`, `is_digest_shaped`, `is_uuid_shaped`,
  `is_separator_delimited_word_phrase`, `is_not_placeholder_value`,
  `is_indirection_reference`, `is_credential_like_value`,
  `is_high_entropy_credential_candidate`.
- `scan_proposal_for_secrets` / `scan_proposal_payload_for_secrets`, which walk
  every text-bearing field of a `WorkspaceProposal` including nested batch items.

Findings carry a rule id, a confidence tier, a severity, and a byte span.
**They never carry the matched text**, because findings are copied into audit
records and evidence summaries — which is precisely what the scanner exists to
keep clean.

## Detection posture, and why

False negatives and false positives are not symmetric, and they are not
asymmetric in the *same direction* at every boundary:

- A missed secret that reaches a hosted endpoint, a retained artifact, or a
  model provider is unrecoverable.
- A false positive shown to a person reading a proposal preview or a terminal
  pane destroys something they needed, and teaches them that `[redacted]` is
  noise. Once a reviewer learns to ignore the marker, the control is dead even
  when it is right.

The resolution is a split by *action*, not a single threshold. `ScanPosture` has
two values:

| Posture | Used at | Structural | Contextual | Heuristic |
| --- | --- | --- | --- | --- |
| `DisplayPrecision` | terminal pane projection | redacts | redacts | reported, not redacted |
| `EgressRecall` | model-bound output, retention capture, trace/export scan | redacts | redacts | redacts |

**Precision-first where a human reads; recall-first where bytes leave.** That is
the whole posture, and it is enforced in one function (`finding_applies`).

### Entropy is deliberately narrow

This is where naive implementations become unusable. This workspace is full of
high-entropy strings that are not secrets: `FileFingerprint` SHA-256 digests,
`content_hash` values, `SnapshotId` and `CorrelationId` UUIDs, FNV constants,
lockfile checksums, and long `CamelCase` type names. The entropy rule requires
*all* of:

- length in `32..=512` — below ~32 characters Shannon entropy is capped by
  `log2(len)` and no meaningful threshold exists;
- at least one uppercase letter, one lowercase letter, **and** one digit. This
  condition eliminates every lowercase hex digest, every UUID, every
  `SCREAMING_SNAKE` constant, and every `snake_case` identifier in the tree;
- not digest-shaped (all-hex at 32/40/56/64/96/128) and not UUID-shaped;
- not a separator-delimited run of alphabetic words;
- not a structured identifier path (`is_structured_identifier_path`);
- Shannon entropy at or above 4.3 bits/char. Hex is capped at 4.0 by its
  alphabet, so no digest can reach the floor however random it is.

Base64 blobs of non-secret data still trip this rule. That residual false
positive is exactly why the rule is `Heuristic` and off the display path.

#### Correction: the character-class condition was not sufficient

An earlier revision of this document claimed the character-class condition alone
made the rule safe here. **That claim was wrong, and the conditions above were
not sufficient as first written.** It was never measured against the repository;
it was reasoned about from a list of shapes chosen by inspection.

Scanning all 1407 UTF-8 files / 46 MB of this worktree produced **5053 findings,
of which 5042 were a single shape the character-class test cannot see**: plan and
evidence paths whose segments interleave words and numbers.

    planning/phases/05-control-trust-and-assisted-ai-surfaces/05-01-RESULT
    plans/evidence/production/M5/WS18-T3-platform-parity-matrix
    20260602T182617_rebaseline_product_surface_gates
    plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN

Each is over 32 characters, mixes case (an uppercase tag such as `RESULT`),
contains digits (a phase or workstream number), and is neither digest- nor
UUID-shaped — so it satisfied every precondition. The existing
`is_separator_delimited_word_phrase` guard did not catch any of them, because it
requires *every* segment to be wholly alphabetic and these contain `05`, `WS18`,
and `T3`.

This mattered operationally, not cosmetically. The entropy rule fires under
`EgressRecall`, and `legion-retention` **fails closed** on any finding
(`deny_capture_on_detected_secrets` defaults to `true`). Retaining any source
file that merely mentions a plan path would have been denied. The feature would
have been unusable in this repository.

The fix is `is_structured_identifier_path`, which asks *where the digits sit*.
Human-authored identifiers put digits at a segment's edges (`WS18`, `T3`, `05`,
`20260602T182617`), leaving an alphabetic core; random credential material
interleaves them (`A1b2C3d4`), so stripping the leading and trailing digit runs
still leaves digits behind. Segments of four characters or fewer are treated as
abbreviations (`E2E`, `i18n`, `M5`) because they cannot carry credential entropy,
and digest-shaped segments are recognised inside a path so that a git object id
in a permalink URL is treated as the digest it is.

Measured on the same corpus after the fix:

| Rule | Before | After |
|---|---|---|
| `high-entropy-token` (egress only) | 5042 | 3 |
| `http-authorization-header` | 6 | 6 |
| `pem-private-key-block` | 3 | 3 |
| `aws-access-key-id` | 1 | 1 |
| `generic-secret-assignment` | 1 | 1 |
| **Total** | **5053** | **14** |

The 11 non-heuristic findings are unchanged and are all intentional test
fixtures or marker constants. The 3 remaining heuristic findings are a macOS
temp directory with a random component and a ULID-style session id appearing
twice — genuinely opaque tokens, which is what the rule says it detects.

Reproduce with:

    cargo run --release -p legion-security --example corpus_scan -- .

The five corpus-derived shapes are now in the negative fixture table, so this
regression cannot return silently. Note that the pre-existing test suite passed
both before and after the fix: **fixture tests chosen by inspection did not
detect a 5042-finding false-positive rate.** Only the corpus scan did.

### Accepted false negatives, stated explicitly

- Hex-only credentials at exactly a digest length (32/40/56/64/96/128) are
  suppressed, because suppressing them is the price of not flagging every
  `content_hash` in the tree.
- A credential whose value contains a placeholder word (`none`, `todo`,
  `example`, ...) is suppressed by the placeholder filter.
- `token`, `credential`, `auth_key`, `session_key`, and `bearer` are *weak*
  keywords requiring a much stricter value shape, because this is an IDE
  codebase with lexers and tokenizers where `token`-named bindings are constant.
  A short, low-entropy value under a `token` name is not flagged.
- A credential that is a single lowercase word under a weak keyword is not
  flagged.
- **Added with `is_structured_identifier_path`:** a credential formatted as short
  hyphenated groups (`A1B2-C3D4-E5F6-G7H8-...`, a product licence key shape) is
  suppressed, because every segment falls under `MAX_OPAQUE_SEGMENT_LEN`. This is
  the price of excluding `E2E`, `i18n`, and `M5` from paths. It costs only the
  entropy heuristic; provider keys and bearer tokens are unformatted and remain
  covered by structural rules.

### Repo-specific false positives found by inspection during design

These five were identified by reading the codebase, not by scanning it. Each is
in the negative fixture table. Their provenance has since been re-checked against
the tree, because "a real workspace string" and "a plausible string" are
different kinds of evidence:

| Shape | Grounded in this repo? |
|---|---|
| `PROVIDER_SECRET_SERVICE = "legion-ai-providers"` | **Yes** — `crates/legion-storage/src/secrets.rs:11`, verbatim |
| `let token = lexer.next_token();` | **Yes** — `next_token` has 3 occurrences outside `legion-security` |
| `tokenizer_config = "gpt2-medium-fast"` | **Partly** — `tokenizer` appears 6 times; this exact assignment is constructed |
| `api_key = process.env.OPENAI_API_KEY` | **No** — 0 occurrences; `process.env` is a JavaScript idiom, not used in this Rust workspace |
| `sk-learn-model-training-pipeline` | **No** — 0 occurrences; a plausible shape, not one found here |

The last two are still worth keeping as fixtures — they guard real rule
behaviour (`is_indirection_reference` and the `sk-` opacity test) — but they were
presented as discoveries from this repository and they are not. Inspection found
three real strings; it found none of the 5042 entropy false positives, which is
the argument for scanning over reading.

## Surfaces: which are covered, which are not

The task names three surfaces. They are not equally real in this codebase.

### 1. Terminal excerpts — COVERED

`legion-terminal` now depends on `legion-security` (already permitted by
`plans/dependency-policy.md`). `redact_terminal_projection` runs the original
hand-rolled pass first (so its proven behaviour is preserved byte for byte) and
then the shared ruleset under `DisplayPrecision`. Both live terminal paths —
launch output and `poll_output` — route through this one function.

### 2. Proposal content — PARTIALLY COVERED, and the gap is structural

`scan_proposal_for_secrets` covers `TextEdit`, `CreateFile.initial_content`,
`CodeAction`, `WorkspaceEdit`, `TerminalCommand` (command and environment), and
`Batch` recursively, plus `PreviewSummary`. It is wired into
`legion-agent::external::external_workspace_edit_proposal`, the chokepoint where
externally and AI-authored edits become proposals; a finding adds a
count-only annotation to the proposal preview details.

**Not covered:** `AssistedAiEditProposalOutput::to_workspace_proposal` in
`legion-protocol`, which is the *other* producer of AI proposals and today runs
only `contains_forbidden_phase8_payload`. `legion-protocol` is the base of the
dependency graph and cannot depend on `legion-security`, so covering it requires
either inverting that edge or moving the call to every caller. That is beyond
this task and is stated here rather than papered over.

**A second structural constraint, found during the work — restated after
verification.** The original claim here was that
`contains_forbidden_phase8_payload` rejects any proposal whose rendering contains
`secret`, `token`, `password`, or `api_key`, and that *every* secret rule id
contains one of those words. Cross-checking the marker list at
`crates/legion-protocol/src/lib.rs` against `SecretRuleId::stable_id()` shows
that is **not true**: 10 of the 18 rule ids are rejected, 8 are not.

Rejected (10): `aws-secret-access-key`, `github-token`,
`github-fine-grained-token`, `gitlab-personal-access-token`, `slack-token`,
`stripe-secret-key`, `npm-access-token`, `json-web-token`,
`generic-secret-assignment`, `high-entropy-token`.

Representable (8): `aws-access-key-id`, `slack-webhook-url`, `openai-api-key`,
`anthropic-api-key`, `google-api-key`, `pem-private-key-block`,
`http-authorization-header`, `url-embedded-credentials`.

The conclusion survives the correction, for a slightly different reason. A
rule-id-bearing annotation would make a proposal unrepresentable *depending on
which credential was found* — a DTO that fails to serialize only sometimes is
worse than one that never names the rule. So the preview annotation stays
count-only (`credential_scan_sites=N credential_scan_findings=M`).

The eight that pass do so only because the marker list spells `api_key` with an
underscore while rule ids use a hyphen. That is punctuation luck, not a design
property: normalising separators in the marker list would push all eighteen into
the rejected set, not free the eight. The eight are not headroom.

### 3. Retained/ejected artifacts — COVERED BY SUBSTITUTION; the named surface does not exist

There is no "retained context" or "ejected context" feature in this codebase.
`retained_context` and `ejected_context` exist only as *substring markers* in
`legion-security` and `legion-protocol`, and as the string literals in the three
pre-existing tests. Grepping `crates/` for `eject` returns nothing but
`reject`/`rejected`. The backlog card names a surface that was never built.

*Re-verified independently.* Every occurrence of either identifier across
`crates/`, `xtask/`, and `plans/kanban/` is one of: the marker list in
`crates/legion-security/src/lib.rs:356-357`, the two forbidden-payload lists in
`crates/legion-protocol/src/lib.rs` (`contains_forbidden_collaboration_payload`
and `contains_forbidden_phase8_payload`), the redaction regex in
`crates/legion-ai/src/redaction.rs:37`, or a test asserting one of those. There
is no struct, field, enum variant, or function of that name. The claim holds.

The real retained-artifact path is the `legion-retention` raw-source vault.
`scan_raw_source_capture_files` now scans capture files before sealing, and
`capture_bundle` fails closed when credentials are detected
(`RawSourceVaultConfig::deny_capture_on_detected_secrets`, default `true`).
Rationale: consent to retain source is not consent to retain the credentials
sitting in it, and a retained bundle is an exportable bundle — refusing the
capture keeps the credential out of the vault, the index, and any future hosted
export in one decision. The denial reason is metadata-only.

Also upgraded transitively, because they route through
`redact_model_bound_output`: agent tool output, `legion-agent` evidence
summaries (`external_log_evidence_record`, `debug_adapter_audit_evidence_record`,
`test_run_summary_evidence_record`).

## Tests written

`crates/legion-security/tests/secrets.rs` — 20 tests (17 new, 3 pre-existing and
preserved unchanged):

| Test | Asserts |
| --- | --- |
| `every_declared_rule_has_a_fixture` | fixture table and `SecretRuleId::all()` are one-to-one |
| `each_rule_fixture_is_detected_by_its_own_rule` | 18 positive fixtures, one per rule |
| `no_workspace_string_produces_a_finding` | 23 real-workspace negative fixtures |
| `generated_fixture_bodies_clear_the_entropy_floor` | guards the fixture generator itself |
| `display_posture_keeps_heuristic_findings_out_of_redaction` | posture split |
| `egress_posture_redacts_heuristic_findings` | posture split |
| `structural_findings_redact_under_every_posture` | 17 non-heuristic rules |
| `redaction_removes_the_entire_credential_value` | prefix and body both go |
| `digest_and_uuid_shapes_are_recognized` | 6 digest lengths plus UUID |
| `word_phrase_placeholder_and_indirection_shapes_are_recognized` | shape predicates |
| `shannon_entropy_separates_digests_from_credentials` | threshold separates the populations |
| `proposal_create_file_content_is_scanned` | proposal surface |
| `proposal_terminal_command_environment_is_scanned` | proposal surface |
| `proposal_preview_summary_is_scanned` | proposal surface |
| `proposal_batch_items_are_scanned_recursively` | a nested batch item cannot evade |
| `clean_proposal_produces_no_sites` | no false positive on ordinary code |
| `marker_scanner_now_reports_credentials_the_marker_list_cannot_see` | the delta itself |
| `proposal_content_payload_requires_redaction` | pre-existing, preserved |
| `terminal_excerpt_payload_requires_redaction` | pre-existing, preserved |
| `retained_and_ejected_context_payload_requires_redaction` | pre-existing, preserved |

`crates/legion-ai/tests/redaction.rs` — 2 added:
`redact_model_bound_output_scrubs_credentials_with_no_marker_or_prefix`,
`redact_model_bound_output_leaves_digests_and_identifiers_intact`.

`crates/legion-terminal/src/lib.rs` — 2 added:
`terminal_projection_redacts_credentials_the_marker_pass_cannot_see`,
`terminal_projection_keeps_high_entropy_build_output_readable`.

`crates/legion-retention/src/lib.rs` — 2 added:
`file_backed_vault_refuses_to_retain_detected_credentials`,
`credential_free_capture_still_succeeds_after_scanning`.

`crates/legion-agent/src/external.rs` — 2 added:
`external_workspace_edit_proposal_flags_credentials_in_preview_details`,
`external_workspace_edit_proposal_leaves_clean_previews_unannotated`.

Total: 25 new tests, 3 pre-existing tests preserved.

### No credential-shaped literal is committed

Every fixture credential is *generated at run time* from a fixed-seed LCG. Two
reasons. First, a committed literal with the shape of a key is what the
repository's pre-commit secret scanning exists to stop, and a test suite that
needs a scanner allowlist entry is a test suite people eventually turn off.
Second, the generator's class cycle (`U, L, D, U, L`) never emits two adjacent
letters of the same case, which makes it *structurally impossible* for a
generated body to spell `AKIA`, `eyJ`, `AIza`, or `xox` and be claimed by the
wrong rule.

## Verification

### First pass (2026-08-17, original author): nothing was run

The Bash tool was unavailable for that entire session, so no gate was executed
and the code was never compiled. The author said so plainly and left the card at
`todo`. That honesty is why this second pass had a usable starting point.

### Second pass: every gate executed

Run on Windows 11, `cargo 1.97.1 (c980f4866 2026-06-30)`, worktree
`agent-a595baf3225cb6571` at base `f58a949`.

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all` | 0 | reformatted `secrets.rs`, `tests/secrets.rs`, `external.rs` |
| `cargo fmt --all --check` | 0 | clean after the above |
| `cargo test --workspace --all-targets --no-fail-fast` | 101 | 267 suites ok; 1 suite failed — see below |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo run -p xtask -- extract-before-modify` | 0 | "no chokepoint file grew past its slack" |
| `cargo run -p xtask -- check-deps` | 0 | "dependency policy checks passed" |
| `cargo run -p xtask -- docs-hygiene` | 0 | "documentation hygiene checks passed" |
| `cargo run -p xtask -- claim-audit` | 0 | "claim audit passed" |
| `cargo run -p xtask -- verify-kanban-backlog` | 0 | 10 epics, 41 features, 161 tasks |
| `cargo run -p xtask -- verify-readiness-consistency` | 0 | 161 tasks cross-checked |
| `cargo deny check` | 0 | "advisories ok, bans ok, licenses ok, sources ok" |

Card verification commands:

| Command | Exit | Result |
|---|---|---|
| `cargo test -p legion-security --test secrets` | 0 | 22 passed, 0 failed |
| `cargo test -p legion-ai` | 0 | 144 passed across 10 suites, 0 failed |

The first author's static predictions were checked and all held:
`extract-before-modify` watches exactly the three files they named and none was
touched; the two new internal edges are permitted; `claim-audit` reads only
`README.md` and top-level `docs/*.md`. Their one prediction that fmt was the
likeliest gate to trip was also right — it did reformat three files, though it
fixed them rather than failing.

### The one workspace-suite failure, and why it is not this change

`legion-app --test delegated_task_integration`: 20 passed, 5 failed. All five
failed with `Timeout` on worker-thread waits
(`delegated_worker_panic_reports_failure_and_cleans_sandbox`,
`delegated_cancel_after_worker_completion_preserves_completed_outcome`,
`delegated_background_submit_stays_responsive_and_manual_waits_for_cancel_ack`,
`delegated_background_rejects_sync_overlap_and_defers_assist_downgrade`,
`dropping_app_cancels_worker_without_blocking_and_reaper_joins_cleanup`).

Re-running the target alone: **25 passed, 0 failed, exit 0.** These are
wall-clock timeouts under full parallel load in a crate whose worker/sandbox
threading this change does not touch.

Two caveats recorded rather than smoothed over:

- This is **not** the flake family that was described to this session as known
  (four workflow-supervisor tests plus
  `terminal_orphan_cleanup_kills_and_records_evidence`). It is a different set
  of five tests in a different target.
- The evidence file those known flakes were said to live in,
  `plans/evidence/production/WS-P0/2026-08-17-parallel-load-flakes.md`, **does
  not exist**, and no file matching `*flake*` exists anywhere under `plans/`. So
  "already recorded" could not be confirmed for either family.

Two earlier full-suite attempts failed to link rather than to test, with
`LNK1201` then `LNK1285` (corrupt PDB) on `legion-desktop`, at 1063 GB free —
PDB write contention under parallel linking, not disk. Deleting the poisoned
PDBs and re-running at `-j 4` cleared it.

## Not claimed

- The `legion-protocol` proposal producer
  (`AssistedAiEditProposalOutput::to_workspace_proposal`) is **not** covered.
  `legion-security` depends on `legion-protocol` (verified in
  `crates/legion-security/Cargo.toml:9`), so the reverse edge would be a cycle.
  Proposal-content coverage is partial and this is the gap.
- The 4.3 bits/char entropy floor is still a reasoned starting point, not a
  corpus-tuned one. The corpus scan measured the *rule set's* false-positive rate
  against this repository; it did not tune the threshold against real credential
  payloads, which would need a labelled corpus this repo does not have.
- Base64-encoded non-secret data still produces heuristic-tier false positives at
  egress boundaries. Designed trade, unmeasured cost.
- The scan corpus is this repository's own text. It is representative of what
  `legion-retention` will capture from *this* workspace and of the plan/evidence
  prose that dominates it. It says nothing about a user's workspace with a
  different file mix.
- The card's `files` list names three files; the implementation also modified
  `legion-terminal`, `legion-retention`, `legion-agent`, and three `Cargo.toml`s.
  That expansion is what makes the three named surfaces actually covered, but it
  is outside the card's declared file set and is recorded here rather than
  hidden.
- The entropy threshold (4.3 bits/char) and the 32-character floor were derived
  by hand-calculation against representative workspace strings, **not** by
  measuring a corpus. They are defensible starting values, not tuned ones.
- The rule set is not exhaustive. Azure, Twilio, SendGrid, Mailgun, DigitalOcean,
  HashiCorp Vault, Datadog, and SSH `authorized_keys` material have no rules.
- Binary payloads are scanned via `String::from_utf8_lossy`, which can split a
  credential that straddles an invalid byte sequence.
- No rate limiting or size ceiling is applied to scanning. A very large payload
  runs every regex over the whole text. The existing byte caps upstream bound
  this in practice, but the scanner itself has no self-protection.
- Nothing was pushed. Work is on the agent worktree branch only.
