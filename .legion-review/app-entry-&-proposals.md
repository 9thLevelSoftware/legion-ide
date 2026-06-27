# App Entry & Proposals Review

Scope reviewed:
- `crates/legion-app/src/lib.rs`
- `crates/legion-app/src/main.rs`
- `crates/legion-app/src/first_run.rs`
- `crates/legion-app/src/offline_ai.rs`
- `crates/legion-app/src/proposal.rs`

Validation performed:
- `cargo check -p legion-app --all-targets` passed.
- `cargo clippy -p legion-app --all-targets -- -D warnings` passed.
- `cargo check -p legion-app --no-default-features` passed.
- `cargo check -p legion-app --no-default-features --all-targets` failed with the offline test initializer issue documented below.
- Direct `target/debug/legion-app` launch from an empty temporary directory failed on the default `scratch.txt` path, as documented below.

Severity breakdown:
- Critical: 0
- High: 4
- Medium: 3
- Low: 0

Total findings: 7

## `crates/legion-app/src/main.rs`

### Finding 1
- Category: failure-point
- Severity: medium
- Line numbers: 11-22
- Description: The CLI entry point defaults to `scratch.txt` but opens it with `AppComposition::open_file`, which uses `OpenFileIntent::Existing`. Starting the app in a fresh directory therefore exits before the command loop because `scratch.txt` does not exist. I verified this by running the built `target/debug/legion-app` from an empty temporary directory; it returned `platform error: not found for .../scratch.txt while metadata` with exit status 1.
- Suggested fix direction: Either default through explicit create intent (`open_new_file`) when the default scratch path is absent, require the user to pass an existing file, or surface an interactive/new-file fallback instead of exiting during app startup.

## `crates/legion-app/src/first_run.rs`

No findings identified in this file. The first-run helpers are small, deterministic, and covered by tests for enabled/disabled crash-reporting consent and settings projection updates.

## `crates/legion-app/src/offline_ai.rs`

### Finding 2
- Category: error
- Severity: high
- Line numbers: 1283-1320
- Description: The offline-build test helper `route_request()` constructs `AssistedAiProviderRouteRequest` without the required `prompt_prefix` field. This breaks the no-default-feature test target even though the no-default-feature library target checks. Verification command: `cargo check -p legion-app --no-default-features --all-targets` failed with `error[E0063]: missing field prompt_prefix in initializer of legion_protocol::AssistedAiProviderRouteRequest` at `offline_ai.rs:1283:9`.
- Suggested fix direction: Add `prompt_prefix: String::new()` (or the intended deterministic prompt prefix) to the test initializer and keep no-default-feature all-targets in CI so offline tests track protocol shape changes.

### Finding 3
- Category: failure-point
- Severity: high
- Line numbers: 410-418, 420-435
- Description: `validate_containment` canonicalizes the sandbox base but only lexically normalizes the candidate target path. A path that remains textually under the sandbox while traversing a symlink inside the sandbox can pass `starts_with` and still resolve outside the sandbox at use time. The same function also calls `std::env::current_dir().unwrap()` on fallback paths, so a process with an unavailable current directory can panic instead of returning `OfflineAiError`.
- Suggested fix direction: Propagate `current_dir` errors instead of unwrapping, and validate containment against a canonical/resolved target parent with symlink-aware filesystem checks. For create-file targets, validate the existing parent directory canonically and reject symlink components or use an openat-style/no-follow creation strategy.

## `crates/legion-app/src/proposal.rs`

### Finding 4
- Category: bug
- Severity: high
- Line numbers: 81-92, 117-125
- Description: `filtered_batch_proposal_for_accepted_targets` keeps every item whose own targets are accepted, then drops dependency edges whenever either endpoint is not retained. If an accepted item depended on a rejected/omitted prerequisite item, the dependency is silently removed and the dependent item can be applied without its required prerequisite. This changes the semantics of the original batch during partial hunk application.
- Suggested fix direction: Preserve dependency safety when filtering. Either reject a filtered batch when any retained item has a prerequisite outside the retained set, recursively include required prerequisites only when their targets were accepted, or mark dependent retained items as not applicable with a diagnostic/partial-failure record.

## `crates/legion-app/src/lib.rs`

### Finding 5
- Category: bug
- Severity: high
- Line numbers: 10525-10537, 18138-18155
- Description: The direct save workflow writes the file through `workspace.save_file_with_proposal` before requiring the applied-proposal audit to persist. If `observe_proposal_response` fails after the workspace save succeeds, `SaveWorkflowService::save_active_buffer` returns `SaveWorkflowFailure`, and `AppComposition::save_buffer` reports `AppSaveOutcome::Rejected` without rolling back the on-disk write and without binding the new saved metadata into `active_documents`. This can leave disk mutated while the editor/app state believes the save failed and remains dirty/stale. The generic `apply_workspace_proposal` path has rollback logic for audit failure, but this direct `save_active_buffer` path does not.
- Suggested fix direction: Treat audit failure after a successful workspace save consistently with proposal apply: either persist audit before committing the workspace mutation, add rollback for the saved file using a pre-save checkpoint, or commit the saved editor metadata while surfacing the audit failure separately so disk/editor state cannot diverge.

### Finding 6
- Category: failure-point
- Severity: medium
- Line numbers: 15656-15659, 15702-15706, 15742-15744
- Description: `execute_delegated_task` allocates a sandbox/worktree, but cleanup is not guaranteed on all error exits. If the ACP host command fails to start, the `?` at the `run(...)` call returns immediately. If the host succeeds but the proposal file cannot be read, the `read_to_string(...)?` path also returns immediately. In both cases the already-initialized sandbox is not cleaned up. The unsuccessful-exit path does clean up explicitly, and the later proposal-generation path also attempts cleanup, so these early ACP paths are inconsistent.
- Suggested fix direction: Use a scope guard/RAII cleanup wrapper or restructure the ACP block so every return after successful `orchestrator.initialize` runs `orchestrator.cleanup(&permission)` and reports cleanup failures alongside the original error.

### Finding 7
- Category: bug
- Severity: medium
- Line numbers: 21705-21763, 2189-2202
- Description: Partial delegated-hunk application creates a filtered batch proposal locally, but the coordinator stores the original proposal before filtering and never replaces it with the filtered one. `proposal_ledger_projection` later renders rows from `self.proposals`, so after a filtered apply the ledger/review projection can still describe the original full batch even though only accepted hunks were applied. That can make rejected hunks appear to be part of the applied proposal metadata.
- Suggested fix direction: When `filtered_apply_required` returns a filtered proposal, update the coordinator's stored proposal (or store an applied-subset record) before observing/applying transitions, and ensure the ledger/review projection distinguishes original proposal coverage from the actually applied subset.
