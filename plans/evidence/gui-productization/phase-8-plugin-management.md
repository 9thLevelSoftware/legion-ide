# GUI Phase 8 plugin management evidence

## Status

- Plugin management GUI: supported.
- Phase 8 acceptance: not final; this artifact covers Plan 08-02 only.

## Scope

The desktop GUI now renders plugin management rows from `PluginContributionProjection` and can invoke projected plugin commands through the existing app-owned command path.

Covered behavior:

- Plugin rows show plugin id, total contribution count, command count, non-command contribution count, status, and metadata-only sandbox/audit notes.
- Command rows show projected command id, display title, required capability id, and dispatch-intent-only audit wording.
- Desktop command routing validates plugin id and command id against the current projection before dispatch.
- Unknown plugin ids, unknown command ids, and empty command ids produce typed bridge errors and no app intent.
- Accepted, denied, quota-refused, and no-runtime plugin command responses become explicit desktop workflow outcomes and status messages.

## Preserved Boundaries

- Plugin command execution remains app-owned and protocol-mediated through `CommandDispatchIntent::InvokePluginCommand`.
- `legion-desktop` imports protocol projection/DTO types only; it does not import `legion-plugin` or call plugin runtime APIs directly.
- `legion-ui` remains projection-only and was not modified for this plan.
- The GUI uses metadata labels, contribution descriptors, status labels, and capability ids already present in projections; it does not persist or render raw plugin payloads, raw manifest bodies, stored plugin data, source text, or sandbox output.
- Runtime sandbox, capability, quota, and metadata-only audit behavior remains covered by the app/plugin runtime tests.

## Verification

| Command | Result |
|---|---|
| `rg -q "InvokePluginCommand" crates/legion-desktop/src/bridge.rs` | passed |
| `rg -q "PluginCommand" crates/legion-desktop/src/workflow.rs` | passed |
| `rg -q "plugin management" crates/legion-desktop/src/view.rs` | passed |
| `rg -n "legion_plugin\|legion-plugin" crates/legion-desktop/src crates/legion-desktop/tests` | passed, no matches |
| `cargo test -p legion-desktop plugin_management_unknown -- --nocapture` | passed, 1 matching test |
| `cargo test -p legion-desktop plugin_management_command_routes_to_app_intent -- --nocapture` | passed, 1 matching test |
| `cargo test -p legion-desktop plugin_management_rows -- --nocapture` | passed, 1 matching test |
| `cargo test -p legion-desktop plugin_management -- --nocapture` | passed, 5 matching tests |
| `cargo test -p legion-app plugin -- --nocapture` | passed, 1 matching app plugin test |
| `cargo run -p xtask -- check-deps` | passed |
| `cargo fmt --all --check` | passed |

## Evidence Notes

- The desktop bridge maps valid projected plugin commands only to `CommandDispatchIntent::InvokePluginCommand`.
- The desktop workflow distinguishes invoked, denied, and no-runtime plugin command states instead of reporting a generic no-op.
- The focused desktop tests cover projected command routing, invalid plugin/command rejection, management row content, invoked and denied command outcomes, and a stale-projection no-runtime response.

## Residual Risk

- This evidence does not mark final GUI Phase 8 accepted. Collaboration, remote workspace, delegated task command center, GA operations, platform parity, final evidence checks, and repository-wide gates still have to pass in later Phase 8 plans.
