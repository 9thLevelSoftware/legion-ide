# Module depth implementation evidence — 2026-10-08

Scope: ADR-0057, concentrating existing Git/LSP lifecycle behavior and Vim text
resolution. No product-readiness promotion, new egress capability, or save
authority change is claimed.

## Verification

Local Windows checks:

| Check | Result |
| --- | --- |
| `cargo test -p legion-app --lib -- language:: git_inspection:: vim_session::` | 190 passed initially; two missing-fixture failures passed in a targeted rerun after building `mock_lsp_server`. All 192 selected tests passed. |
| `cargo test -p legion-app --test git_workflow --test git_remote_policy_workflow --test lsp_rename_across_files` | 13 Git workflow, 11 remote policy, and 6 LSP proposal integration tests passed. |
| `cargo test -p legion-editor -p legion-ui --lib vim` | 5 editor and 88 UI tests passed. |
| `cargo test -p legion-app --test vim_modal_editing` | 29 passed initially; the differential comparison passed in a targeted rerun after correcting its legacy endpoint conversion. All 30 tests passed. |
| `cargo fmt --all --check` | Passed. Subsequent lint-only edits receive scoped formatting checks. |
| `cargo run -p xtask -- check-deps` | Passed; no dependency edges changed. |
| `cargo clippy -p legion-app -p legion-editor -p legion-ui --all-targets -- -D warnings` | Passed after replacing redundant Option branches and fixing test-only path comparisons/module placement. |
| `cargo run -p xtask -- docs-hygiene` | Passed. |
| `git diff --check` | Passed. |

Total: 345 distinct focused tests passed. Successful tests were not repeatedly
run. The mock-server prerequisite was built with
`cargo build -p legion-lsp --bin mock_lsp_server`; only the two failed tests
were rerun. An initial LSP fixture compile error used 64-bit values for 128-bit
protocol IDs and was corrected before test execution.

Final documentation and whitespace verification passed. Formatting also passed
for the three files changed after the workspace-wide formatting check.

## Behavior and review

- Git scheduling tests exercise coalescing, mutation-before-refresh ordering,
  backpressure, disconnection, and preserving the original mutation repository
  while switching the projected workspace. Remote policy denial remains tested.
- LSP tests exercise bounded admission before transport submission, paired
  invalidation, cancellation/late responses, and review-mediated edits.
- Vim tests compare all motion kinds and operator ranges with the prior effective
  character-to-byte conversion, including Unicode, empty/final lines, and CRLF.
  The original pure resolver can return an endpoint beyond an empty line; the
  old app clamped it during conversion. The differential test now models that
  existing effective range rather than requiring invalid native coordinates.
- A streamed app fixture larger than the 5 MiB cache budget exercises real
  movement and deletion near EOF without a full-text read. This establishes the
  bounded-access behavior, not a measured latency claim.
- Independent source review found an append-at-line-end regression. The fix
  uses the next scalar's end as the insertion position; normal cursor motion
  still clamps to content. Tests type after the last multibyte scalar on LF and
  CRLF lines. Review found no material Git/LSP issues or new authority bypass.
  The reviewer confirmed the append finding resolved after inspecting the fix;
  the corrected differential comparison also passed.

Cargo emitted nonfatal Windows incremental-cache finalization warnings
(`Access is denied`); compilation and test execution proceeded. No cache
cleanup or permission changes were made.

## Changed files

- Git: `crates/legion-app/src/git_inspection.rs` and
  `crates/legion-app/tests/git_workflow.rs`.
- LSP: `crates/legion-app/src/language/{write_lifecycle.rs,lsp_reads.rs,app_lsp.rs,code_actions.rs,mod.rs}`
  and the adjacent `code_action_tests.rs` / `write_operation_tests.rs`.
- Vim: `crates/legion-editor/src/{lib.rs,vim.rs}`,
  `crates/legion-ui/src/vim_motion.rs`,
  `crates/legion-app/src/vim_session.rs`, and
  `crates/legion-app/tests/vim_modal_editing.rs`.
- Shared app wiring: `crates/legion-app/src/lib.rs`.
- Documentation: ADR-0057, this evidence record, `plans/dependency-policy.md`,
  and two new index entries in `docs/INDEX.md`.

The earlier agent-skill setup edits and `.omp/plans/FRONTIER_INTEGRATION_PLAN.md`
were preserved. No commit, push, or external publication was performed.

## External gates

Cross-platform CI, windowed GUI runs, and measured performance are not part of
this local verification. Full workspace release gates were not run. Tracker
persistence remains deferred. Vim put/open-line commands retain their existing
full-text path, and document synchronization remains outside the new LSP
write-lifecycle module. This change does not claim complete large-file Vim
support or promote product-readiness rows.
