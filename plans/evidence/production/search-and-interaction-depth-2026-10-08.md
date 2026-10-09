# Search and interaction depth evidence — 2026-10-08

Scope: ADR-0058, shared worktree search traversal, app-owned completion/hover
scheduling, and cohesive explorer activation. Existing work in progress is
preserved; no commit, push, or external publication is authorized by this work.

## Verification

Local Windows results:

| Check | Result |
| --- | --- |
| `cargo test -p legion-agent --lib agent_loop` | 17 passed, 0 failed. |
| `cargo test -p legion-agent --test agent_loop_integration` | 23 passed, 0 failed. |
| `cargo test -p legion-app --test interaction_workflows` | 11 passed, 0 failed after correcting a test fixture's ID from u64 to u128. |
| `cargo test -p legion-desktop --test input_conformance --test explorer_activation --test completion_popup --test hover_definition --test intent_bridge` | 29 input, 6 explorer, 8 completion, 7 hover/definition, and 19 bridge tests passed; 0 failed. |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy -p legion-agent -p legion-app -p legion-desktop --all-targets -- -D warnings` | Passed. |
| `cargo run -p xtask -- check-deps` | Passed; no dependency edges changed. |
| `cargo run -p xtask -- docs-hygiene` | Passed. |
| `git diff --check` | Passed. |

The 120 distinct focused tests passed without behavioral test failures. The app
test target was rerun only after its fixture compile error; successful checks
were not repeated. Six new search tests cover hidden-directory versus
hidden-file behavior, distinct glob matching rules, binary/invalid-UTF-8
handling, line versus file limits, zero-limit dispatch, and worktree-to-workspace
forbidden-path mapping. Existing alias-root, junction/cycle, forbidden-path,
and I/O-failure contracts passed. Cargo emitted nonfatal Windows incremental
cache access-denied notices; no cache or permission changes were made.
Desktop compilation also reported an existing float-literal future-compatibility
warning in vendored `epaint`; this did not fail tests or affected-crate Clippy,
and vendor code was not changed.

Independent read-only review of the shared traversal found no material issue.
It compared both tool implementations with the original walkers and confirmed
matching/limit semantics, audit placement, filtering, and error propagation.
The reviewer did not repeat the already passing tests.

The app-interface tests cover exact 50/200 ms scheduling, rearming, ordinary
and directed edits, post-action cursor coordinates, rejected/no-op actions,
tab/workspace invalidation, stale text versions, and existing transport dispatch
without recursive rearming. Explorer tests cover the actual opened identity,
tab reuse, and failed activation preserving the previous selection and dirty text.

Independent app/desktop review also found no material issue. The reviewed
production file hashes were checked against the final implementation and matched.
Desktop still owns input gating, frame timing, and popup presentation; app owns
language-read timing and file-activation ordering. No live language server or
windowed GUI run was used by this review.

## Changed files and WIP preservation

- Search: `crates/legion-agent/src/agent_loop.rs` and private
  `crates/legion-agent/src/agent_loop/search_traversal.rs`.
- App: `crates/legion-app/src/lib.rs`, private
  `crates/legion-app/src/lsp_interaction.rs`, and
  `crates/legion-app/tests/interaction_workflows.rs`.
- Desktop: `crates/legion-desktop/src/bridge.rs`,
  `crates/legion-desktop/src/workflow.rs`, and
  `crates/legion-desktop/tests/intent_bridge.rs`.
- Documentation: root glossary, ADR-0058, this evidence record, documentation
  index, and dependency-policy clarification (no new edges).

A pre-edit copy of existing WIP was retained outside the repository. All 21
preexisting files outside the intended app root/index/policy edits compared
byte-for-byte equal afterward. The app root delta was reviewed against that
copy, excluding the earlier Git, LSP write-lifecycle, and Vim changes.

## External gates

Cross-platform CI, native windowed input acceptance, measured performance, and
full release gates are outside this local refactor verification. Existing
headless desktop tests do not satisfy packaged native-input acceptance. No
product-readiness promotion is claimed.
