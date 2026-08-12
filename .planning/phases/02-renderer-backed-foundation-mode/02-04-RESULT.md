# Plan 02-04 Result: App Composition Desktop Workflow

Status: Complete
Wave: 3
Agents: engineering-senior-developer, testing-qa-verification-specialist

## Files Changed

- `crates/devil-desktop/src/workflow.rs`: replaced the inert runtime with `DesktopLaunchConfig`, `DesktopRuntime`, `DesktopWorkflowOutcome`, app-owned open/dispatch/projection refresh wiring, and an eframe app implementation that renders `ProjectionView`.
- `crates/devil-desktop/tests/desktop_workflow.rs`: added non-rendering harness tests for open/edit/save, external overwrite save rejection, quit, replace, and delete paths.

`crates/devil-desktop/src/main.rs` already called `workflow::run_from_env()` and required no functional change.

## Workflow Paths

- Startup opens a trusted workspace through `AppComposition::open_workspace`, optionally opens an initial file through `AppComposition::open_file`, then creates `Shell` from `AppComposition::shell_projection_snapshot`.
- User actions route through `DesktopCommandBridge::translate`.
- Mutation intents route through `AppComposition::dispatch_ui_intent`; quit remains adapter-local.
- After every handled action, the runtime rebuilds the shell snapshot through app projection APIs and preserves app-owned dirty/save state.
- Save success and save rejection are surfaced as adapter status messages without marking rejected dirty text clean.

## Run Command

- `cargo run -p devil-desktop -- . Cargo.toml`

## Verification

| Command | Result |
| --- | --- |
| `rg -q "AppComposition" crates/devil-desktop/src/workflow.rs` | passed |
| `rg -q "dispatch_ui_intent" crates/devil-desktop/src/workflow.rs` | passed |
| `rg -q "run_from_env" crates/devil-desktop/src/main.rs` | passed |
| `cargo test -p devil-desktop desktop_workflow --test desktop_workflow` | passed; 4 passed |
| `cargo check -p devil-desktop --all-targets` | passed |

## Rejection Evidence

`desktop_workflow_external_overwrite_save_rejects_and_preserves_dirty_projection` edits `seed` to `seed!`, externally overwrites disk with `external`, saves through `DesktopRuntime::handle_action(DesktopAction::SaveActive)`, receives `DesktopWorkflowOutcome::SaveRejected(_)`, verifies disk remains `external`, and verifies the active projection remains dirty with `seed!`.

## Issues

Native window launch smoke is deferred to Plan 02-05, which owns renderer timing and platform smoke evidence.
