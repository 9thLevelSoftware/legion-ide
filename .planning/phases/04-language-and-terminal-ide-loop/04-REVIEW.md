# Phase 4 Review: Language And Terminal IDE Loop

Result: PASSED
Date: 2026-05-27
Cycles: 2

## Review Panel

- testing-qa-verification-specialist: found terminal policy/validator/audit gaps and missing exact integration target coverage.
- testing-test-results-analyzer: found stale exact-filter drift between the plan and test names.
- engineering-senior-developer: found proposal lifecycle ordering, unsafe synthetic edit payloads, stale language projection rows, and terminal lifecycle validator gaps.
- Review coordinator: remediated blockers, updated evidence, and verified current source plus phase gates from the live checkout.

## Findings

| Severity | Status | Location | Finding | Resolution |
| --- | --- | --- | --- | --- |
| Blocker | Fixed | `crates/devil-app/src/lib.rs` | Language edit actions registered Created then attempted Preview directly, violating the lifecycle requirement that proposals validate before preview. | Language proposal dispatch now registers lifecycle context, records Created, validates, then previews. Regression coverage asserts proposal ledger rows reach `Previewed`. |
| High | Fixed | `crates/devil-app/src/lib.rs` | Rename and non-rename language edit payloads used unsafe synthetic zero-length edits. | Rename now targets the identifier byte range at the cursor. Formatting, organize-imports, and code-action paths produce safe full-buffer no-op preview edits with warning diagnostics until live LSP edits exist. |
| High | Fixed | `crates/devil-app/src/lib.rs` | Terminal launch/lifecycle behavior was not consistently policy-gated, validator-checked, or durably audited. | Terminal launch/input/resize/close/kill now use `DenyByDefaultBroker`/terminal validators where applicable, and terminal operations persist metadata-only `TerminalAuditRecord`s and emit audit events. |
| Medium | Fixed | `crates/devil-app/src/lib.rs` | Language projection rows could preserve stale buffer-specific hover/completion/outline data across buffer switches. | Language reads now clear prior buffer-specific rows when workspace/buffer/file identity changes; regression coverage proves stale outline rows do not survive a buffer switch. |
| Medium | Fixed | `crates/devil-app/tests/*`, `crates/devil-desktop/tests/*` | Plan 04-03, 04-04, and 04-05 exact verification filters referenced test names that were absent or drifted. | Added/renamed the exact integration test targets and kept broader file-level tests passing. |

No remaining blockers or warnings were found after remediation and gate verification.

## Verification

| Command | Result |
| --- | --- |
| `cargo test -p devil-app --test language_terminal_integration -- --nocapture` | passed; 5 tests |
| `cargo test -p devil-app --test terminal_workflow -- --nocapture` | passed; 3 tests |
| `cargo test -p devil-app --test language_tooling_workflow -- --nocapture` | passed; 2 tests |
| `cargo test -p devil-desktop --test language_terminal_view -- --nocapture` | passed; 3 tests |
| `cargo test -p devil-desktop --test language_terminal_workflow -- --nocapture` | passed; 3 tests |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |
| `cargo check --workspace --all-targets` | passed |
| `cargo test --workspace --all-targets` | passed; workspace tests passed with three performance-suite workloads ignored by design |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| `cargo deny check` | passed with existing warning-level duplicate-crate findings |

## Residual Risk

- Language results are lexical/semantic projections, not a production supervised LSP runtime.
- Formatting, organize-imports, and code-action preview edits are deliberately safe no-ops with warning diagnostics until live LSP edit providers exist.
- Terminal lifecycle acceptance is proven through the deterministic fixture path; production native PTY activation remains governed by existing terminal/security gates.
- `cargo deny check` continues to emit duplicate-crate warnings while exiting 0 under current policy.

## Next Action

Run `/legion:plan 5 --auto-refine` for Phase 5 when ready.
