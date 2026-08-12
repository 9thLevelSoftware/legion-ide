# PKT-RAIL Evidence — Ghost Text and Assistant Rail

Date: 2026-07-06
Branch: `m9/assist-rail`

## Task summary

PKT-RAIL implements the ghost text overlay view model, the assistant rail command system, per-block proposal binding for every code block in an AI response, and un-orphans the telemetry module in legion-ai.

## T1 — Ghost text overlay view model (e5b05e2)

Created `crates/legion-desktop/src/view/ghost_text.rs`:

- `GhostTextState` enum: `Displaying`, `Accepted`, `Dismissed`, `Cancelled`
- `GhostTextOverlayViewModel` struct: text, insert_position, provider_id, request_id, state, stale
- `ghost_text_from_prediction(result, provider_id) -> Option<GhostTextOverlayViewModel>` — returns `None` for stale predictions (`freshness.state != Fresh`) or when `ghost_text` is absent
- 3 new `DesktopAction` variants in `bridge.rs`: `AcceptGhostText { request_id }`, `DismissGhostText { request_id }`, `CancelGhostText { request_id }`, plus corresponding `translate` arms routing each through `CommandDispatchIntent::AcceptAssistInlinePrediction` / `DismissAssistInlinePrediction` / `CancelAssistInlinePrediction` — never direct buffer mutation

5 integration tests in `crates/legion-desktop/tests/ghost_text.rs`:
- `ghost_text_from_valid_prediction_creates_overlay`
- `ghost_text_from_stale_prediction_returns_none`
- `ghost_text_from_missing_text_returns_none`
- `accept_ghost_text_dispatches_proposal`
- `dismiss_ghost_text_clears_overlay`

## T2 — Rail commands (d2839d6)

Added to `crates/legion-protocol/src/lib.rs`:
- `AssistantRailCommand` enum: `Explain`, `Fix`, `Test`, `Doc`, `Refactor`
- `RailCommandCapability` struct: `command`, `capability_id`
- `rail_command_capabilities() -> Vec<RailCommandCapability>` — returns 5 capabilities with IDs: `ai.rail.explain`, `ai.rail.fix`, `ai.rail.test`, `ai.rail.doc`, `ai.rail.refactor`

Added to `crates/legion-desktop/src/view/assistant_rail.rs`:
- `AssistantRailCommandViewModel` struct: `command`, `label`, `available`
- `rail_command_view_models(capabilities) -> Vec<AssistantRailCommandViewModel>` — emits one entry per canonical command; `available = true` only when capability appears in the granted set

Added to `crates/legion-desktop/src/bridge.rs`:
- `DesktopAction::ExecuteRailCommand { command, selection }` → `CommandDispatchIntent::StartAiProposal { instruction_label: "ai.rail.{command}" }`

5 integration tests in `crates/legion-desktop/tests/assistant_rail.rs`:
- `rail_commands_enumerate_all_five`
- `rail_command_dispatches_proposal_not_mutation`
- `rail_command_view_models_reflect_capability_gates`
- `each_rail_command_has_stable_capability_id`
- `rail_command_without_selection_is_valid`

## T3 — Every code block gets apply-as-proposal affordance (75912ab)

Modified `assistant_rail_rows()` in `crates/legion-desktop/src/view/assistant_rail.rs`:

- Changed from consuming a single proposal (`.take()`) to assigning each complete code block its own `ProposalId(base.0.saturating_add(block_index))` with a running counter
- Block 0 gets the base ID, block 1 gets base+1, etc., enabling independent per-block application

Added:
- `bind_proposals_to_blocks(rows, base_proposal_id) -> Vec<AssistantRailRow>` helper that applies base+offset binding to rows already assembled without proposal context
- `streaming_rail_rows(chunks, proposal_id) -> Vec<AssistantRailRow>` — joins partial stream chunks and delegates to `assistant_rail_rows`

5 tests (updated + new) in inline and integration test files:
- `assistant_rail_rows_bind_proposal_to_every_complete_block` (renamed and updated — second block now asserts `ProposalId(8)`)
- `streaming_rail_rows_accumulate_chunks`
- `non_streaming_response_gets_per_block_proposals`
- `incomplete_streaming_block_never_applyable`
- `assistant_rail_rows_without_proposal_are_not_applyable` (existing, still passes)

## T4 — Un-orphan telemetry module (3009170)

Root cause: `crates/legion-observability/src/telemetry.rs` existed on disk but was never declared as `pub mod telemetry` in `lib.rs`, so `legion_observability::telemetry` was unreachable.

Fixes:
- Added `pub mod telemetry;` to `crates/legion-observability/src/lib.rs`
- Added `legion-observability = { workspace = true }` to `crates/legion-ai/Cargo.toml`
- Added `legion-observability` to the `legion-ai` allowed-deps list in `plans/dependency-policy.md`
- `crates/legion-ai/src/lib.rs` already had `pub mod telemetry;` and re-exports from prior commit

Added smoke test `crates/legion-ai/tests/telemetry.rs`:
- `telemetry_module_is_accessible` — verifies consent gate blocks spool writes (`require_explicit_consent=true`, `consent_current=false` → `None`) and that the full `suggestion_telemetry_recorded_event` path produces a `MetadataOnly` envelope when consent is current

## Test results

```
cargo test -p legion-desktop -- --nocapture
running 5 tests (ghost_text.rs) — all ok
running 8 tests (assistant_rail.rs) — all ok

cargo test -p legion-ai --test telemetry
running 1 test
test telemetry_module_is_accessible ... ok

cargo test --workspace
0 failures across all crates
```

## Security / architecture invariants

- Ghost text acceptance routes through `CommandDispatchIntent::AcceptAssistInlinePrediction` (proposal-mediated); no direct buffer mutation
- `ExecuteRailCommand` maps to `CommandDispatchIntent::StartAiProposal` — still proposal-mediated, never direct
- Telemetry is default-deny: `require_explicit_consent=true` gates all spool writes; `metadata_only` is the classification floor
- No private keys, certificates, tokens, or BYOK keys in any committed file
