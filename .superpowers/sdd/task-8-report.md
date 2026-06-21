# Task 8 Report: LANG.09 — Route rust-analyzer rename/code-action edits to proposals

## Status: DONE

---

## Implementation

### Files changed

- **Created** `crates/legion-app/src/language/proposal.rs`
  - Implements `workspace_edit_to_proposal_input(workspace_edit, request, proposal_id, principal, capability, preconditions, lifecycle_state, privacy_label, preview, created_at, expires_at)` → `LspEditProposalConversionInput`
  - Takes an already-structured `WorkspaceEditProposalPayload` (source variant already set by caller)
  - Sets `diagnostics: Vec::new()`, `schema_version: 1`

- **Modified** `crates/legion-app/src/language/mod.rs`
  - Added `mod proposal;` and `pub use proposal::workspace_edit_to_proposal_input;`

- **Created** `crates/legion-app/tests/language_edit_proposal_routing.rs`
  - Two integration tests: `rust_analyzer_rename_edit_becomes_workspace_proposal` and `rust_analyzer_code_action_edit_becomes_workspace_proposal`

- **Created** `crates/legion-app/tests/proposal_fixture/mod.rs`
  - Fixture helpers mirroring `dto_contracts.rs`: `workspace_edit_payload(source)`, `correlation()`, `preconditions()`, `batch_target_coverage()`, `file_identity()`, `fingerprint()`, `privacy_label()`, `preview()`, `created_at()`, `principal()`, `capability()`

### API corrections applied

- `WorkspaceEditSourceKind` variants used: `LspRename`, `LspCodeAction` (no `LanguageServer` variant exists)
- `WorkspaceEditProposalPayload` built field-by-field (no `from_lsp_changes` constructor)
- `workspace_edit_to_proposal_input` receives an already-structured `WorkspaceEditProposalPayload` (not raw JSON `serde_json::Value`)
- `expires_at: Option<TimestampMillis>` added as explicit parameter (not hardcoded `None`)

### Fixture validation note

`validate_lsp_edit_proposal_contract` requires `workspace_edit.required_capability == input.capability` (line 16650). The fixture sets both to `"language.rename"` consistently.

---

## TDD RED/GREEN

### RED (compile succeeds, tests fail)

After wiring the impl but before fixing the `required_capability` / `capability` mismatch in the fixture:

```
$ cargo test -p legion-app --test language_edit_proposal_routing

running 2 tests
test rust_analyzer_rename_edit_becomes_workspace_proposal ... FAILED
test rust_analyzer_code_action_edit_becomes_workspace_proposal ... FAILED

failures:
  rename edit must convert to proposal without error: CapabilityMismatch
  code-action edit must convert to proposal without error: CapabilityMismatch

test result: FAILED. 0 passed; 2 failed
```

### GREEN

After aligning `required_capability` and `capability` in fixture:

```
$ cargo test -p legion-app --test language_edit_proposal_routing

running 2 tests
test rust_analyzer_code_action_edit_becomes_workspace_proposal ... ok
test rust_analyzer_rename_edit_becomes_workspace_proposal ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Check + Clippy (clean)

```
$ cargo check -p legion-app --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.93s

$ cargo clippy -p legion-app --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.02s
```

---

## Deferred: Raw LSP JSON parsing

Translating raw rust-analyzer `WorkspaceEdit` JSON (LSP `{line, character}` positions and `uri`-based file references) into `WorkspaceEditProposalPayload` (with byte-accurate `TextRange` and resolved `FileIdentity`) requires:

1. **Document text** — to convert LSP line/character offsets to byte offsets (the protocol's `TextRange::byte(start, end)` is strictly byte-based)
2. **Workspace state lookup** — to map `"file:///workspace/src/lib.rs"` URIs to `FileIdentity { file_id, workspace_id, canonical_path, content_version, content_hash }`

Neither is available at this adapter layer. The orchestrator (which has both a buffer/document store and workspace registry) must perform this translation before calling `workspace_edit_to_proposal_input`. Faking `char == byte` would be incorrect for any non-ASCII content.

---

## Self-review / Concerns

- The function signature accepts `expires_at: Option<TimestampMillis>` as a parameter rather than hardcoding `None`, consistent with `LspEditProposalConversionInput` having it as `Option`. The brief mentioned no `expires_at` parameter but the live struct has it — included to be complete.
- No concerns: implementation is thin (pure struct assembly), fully exercised by two tests that go through the real `convert_lsp_edit_to_workspace_proposal` and `validate_lsp_edit_proposal_contract`.

---

## Review fix (Task 8 review — Important + Minors)

### Important: code-action test now uses a distinct capability

The original fixture hard-coded `required_capability: "language.rename"` for both tests, so the `LspCodeAction` test passed only because both the payload and envelope happened to carry the same string — it did not independently verify the `required_capability == capability` invariant for code actions.

Fix:
- `proposal_fixture/mod.rs`: parameterized `workspace_edit_payload(source, required_capability: &str)`. Added `RENAME_CAPABILITY = "language.rename"` and `CODE_ACTION_CAPABILITY = "language.code_action"` consts, plus builders `rename_payload()` / `code_action_payload()` and envelope helpers `rename_capability()` / `code_action_capability()`. Each test keeps `required_capability == capability` (validator-enforced) but the code-action test now uses `"language.code_action"` on **both** payload and envelope.
- `language_edit_proposal_routing.rs`: rename test uses `rename_payload()` + `rename_capability()`; code-action test uses `code_action_payload()` + `code_action_capability()`. Both assertions retained (`matches! ProposalPayload::WorkspaceEdit` AND `source == LspCodeAction`).

### Minors

- Applied: added a one-line comment in `preconditions()` noting `file_version` is a legacy alias of `file_content_version` (both set equal).
- Skipped: reordering `expires_at` before `created_at` in `workspace_edit_to_proposal_input`. This would ripple into both call sites' positional arguments (they currently pass `created_at()` then `None`) for a cosmetic gain — judged "awkward ripple" per the review's own condition, so left as-is.

### Verification

```
$ cargo test -p legion-app --test language_edit_proposal_routing

running 2 tests
test rust_analyzer_code_action_edit_becomes_workspace_proposal ... ok
test rust_analyzer_rename_edit_becomes_workspace_proposal ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```
$ cargo clippy -p legion-app --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
```
