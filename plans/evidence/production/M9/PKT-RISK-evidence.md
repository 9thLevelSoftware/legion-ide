# PKT-RISK Evidence — Graduated Approvals and Risk Gates

**Branch:** `m9/risk-gates`
**Date:** 2026-07-06
**Commits (base d2dad57):**
- `d3de48c` test: verify risk rule allow/deny coverage (P3.F4.T1)
- `c75c69a` feat: graduated approval ladder and auto-approval envelope (P3.F4.T2)
- `27ba7a8` feat: risk strip view model and silent-apply prevention (P3.F4.T3)
- `c77d832` feat: advisory classifier wiring with deterministic-only authority (P3.F4.T4)

---

## Task Coverage Table

| Task | Description | Status | Tests |
|------|-------------|--------|-------|
| T1 | Verify risk rule allow/deny coverage (7 rules × 2 edges) | DONE | Comment added to existing test |
| T2 | ApprovalLevel enum + derive_approval_level + audit metadata + example TOML | DONE | 6 new tests in graduated_approval.rs |
| T3 | DesktopProposalRiskStripViewModel + risk_strip_rows + silent-apply prevention | DONE | 4 new tests in risk_strip.rs |
| T4 | advisory_recommendation on RiskAssessment + evaluate_with_advisory | DONE | 4 new tests in advisory_classifier_wiring.rs |

---

## T1 — Risk Rule Coverage Matrix

All 7 deterministic rules already had both allow and deny test cases enumerated
in `deterministic_risk_rules_cover_allow_and_deny_edges`. Added a comment block
confirming the matrix; no new test cases were needed.

```
Rule                         Allow case                  Deny case
────────────────────────────────────────────────────────────────────
PathScope                   contained path              escaping path
FileCount                   2 files < 4 limit           5 files > 4 limit
DeletionRatio               1/4 = 25% < 49%             3/4 = 75% > 49%
DependencyOrLockfileTouch   src/lib.rs                  Cargo.lock
Migration                   src/lib.rs                  db/migrations/…
SecretsProximity            src/lib.rs                  secrets/api_keys.toml
BinaryOrGeneratedFileChange src/lib.rs                  target/generated/…
```

Test run:
```
running 3 tests
test risk_rule_ids_are_stable_and_enumerated ... ok
test deterministic_risk_rules_cover_allow_and_deny_edges ... ok
test evaluate_risk_rules_uses_default_thresholds ... ok
test result: ok. 3 passed; 0 failed
```

---

## T2 — Graduated Approval Ladder

### Architecture Decisions

**Ladder mapping:**
- `Auto`: all rules allow + `policy.allows_rule_ids(rule_ids)` → apply without human
- `Ask`: all rules allow but policy disabled/not matching → quick confirm
- `RequireExplicit`: any non-critical rule deny → pause, explicit approval required
- `Deny`: PathScope deny (workspace escape) → unconditionally blocked

**Rationale for PathScope = Deny:**
Path-scope violations mean the AI is attempting to write outside the approved
workspace root, which is a security boundary, not a policy preference.
Other rule violations (file count, deletion ratio, etc.) are risk-weighted but
recoverable; path escapes cannot be safely approved by a human quick-confirm.

**Empty-findings guard:**
`allows_rule_ids([])` returns false per the existing vacuous-truth guard in
`ProposalAutoApprovalPolicy`, preventing a zero-evidence proposal from
reaching Auto.

**Files changed:**
- `crates/legion-protocol/src/risk.rs` — added `ApprovalLevel` enum
- `crates/legion-security/src/policy.rs` — added `derive_approval_level()`,
  `approval_level_audit_metadata()`
- `crates/legion-security/src/lib.rs` — re-exported new functions
- `xtask/legion-policy.example.toml` — added graduated ladder documentation

Test run:
```
running 6 tests
test all_allow_with_matching_policy_is_auto ... ok
test all_allow_without_policy_is_ask ... ok
test any_deny_is_require_explicit ... ok
test critical_deny_is_deny ... ok
test empty_rule_ids_never_auto ... ok
test approval_level_appears_in_audit_metadata ... ok
test result: ok. 6 passed; 0 failed
```

---

## T3 — Risk Strip View Model

### Architecture Decisions

**New module:** `crates/legion-desktop/src/view/risk_strip.rs` (not stuffed into
the 3800-line view.rs). The struct and projection functions have clear separation.

**`DesktopProposalRiskStripViewModel` fields:**
- `proposal_id`, `aggregate_risk_label`, `approval_level` — identity + risk
- `findings_summary` — one-line deny summaries for each deny finding
- `requires_human_approval` — true for RequireExplicit/Deny
- `paused` — true for RequireExplicit/Deny

**Dependency policy:** `legion-desktop` cannot depend on `legion-security`
(would create a cycle: legion-security ← legion-ai ← legion-desktop).
Tests construct `RiskAssessment` directly from `legion-protocol` types.
Gate-level wiring is covered in `legion-security/tests/graduated_approval.rs`.

**`risk_strip_rows` renders:**
1. Aggregate risk label row
2. Approval level row
3. One row per deny finding (rule_id + evidence)
4. Pause notice (RequireExplicit) or denial notice (Deny) with reason

**Files changed:**
- `crates/legion-desktop/src/view/risk_strip.rs` — new
- `crates/legion-desktop/src/view.rs` — module declaration + re-exports
- `crates/legion-desktop/tests/risk_strip.rs` — new

Test run:
```
running 4 tests
test low_risk_auto_shows_no_pause ... ok
test medium_risk_require_explicit_pauses ... ok
test high_risk_deny_shows_denial_reason ... ok
test high_risk_never_applies_silently ... ok
test result: ok. 4 passed; 0 failed
```

---

## T4 — Advisory Classifier Wiring

### Architecture Decisions

**No `legion-security` → `legion-ai` dependency:**
`legion-ai` already depends on `legion-security`, so adding the reverse would
create a cycle. Instead, the advisory recommendation is stored as
`Option<ProposalRiskLabel>` (a `legion-protocol` type), mirroring the pattern
already used by `ProposalApplyGate.classifier_recommendation`.

**`advisory_recommendation` is metadata-only:**
- Tagged `#[serde(default)]` for backward compatibility
- Never alters `findings`, `aggregate_risk_label`, or any approval gate
- `evaluate_with_advisory()` runs the deterministic engine then attaches the label

**Production call-site note for PKT-GP2 integrators:**
`evaluate_with_advisory` takes `advisory_label: Option<ProposalRiskLabel>` (a
`legion-protocol` type), **not** `classifier: Option<&AdvisoryRiskClassifier>` as
originally sketched in the brief. The same dep-cycle rationale applies:
`AdvisoryRiskClassifier` lives in `legion-ai`, which already depends on
`legion-security`; reversing that edge would create a cycle.  The production call
site must pre-compute the advisory label before calling this function and pass the
resulting `Option<ProposalRiskLabel>` directly.

**Files changed:**
- `crates/legion-protocol/src/risk.rs` — `advisory_recommendation` field added to
  `RiskAssessment`
- `crates/legion-security/src/risk.rs` — `advisory_recommendation: None` in
  constructor; `evaluate_with_advisory()` added
- `crates/legion-security/tests/advisory_classifier_wiring.rs` — new

Test run:
```
running 4 tests
test classifier_low_does_not_override_deterministic_deny ... ok
test classifier_high_does_not_override_deterministic_allow ... ok
test classifier_recommendation_appears_in_assessment ... ok
test assessment_without_classifier_has_none ... ok
test result: ok. 4 passed; 0 failed
```

---

## Dependency Check

`legion-security` depends on: `legion-protocol` only (unchanged).
`legion-ai` depends on: `legion-protocol`, `legion-security` (unchanged).
`legion-desktop` depends on: `legion-protocol`, `legion-app`, etc. (unchanged).
No new cross-crate edges added. Dependency policy satisfied.

---

## Format Check

```
cargo fmt --check  →  (no output — all files pass)
cargo clippy -p legion-protocol -p legion-security -p legion-desktop
             →  (no output — no warnings or errors)
```

---

## Full Suite Summary

All tests in legion-protocol, legion-security, and legion-desktop pass.
14 new tests added across 3 test files:
- `crates/legion-security/tests/graduated_approval.rs` — 6 tests (T2)
- `crates/legion-desktop/tests/risk_strip.rs` — 4 tests (T3)
- `crates/legion-security/tests/advisory_classifier_wiring.rs` — 4 tests (T4)
