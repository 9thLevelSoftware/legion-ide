# Ticket 40: explicit provider connection check

Status: review-ready implementation; component evidence only. Ticket 40 and native/live
acceptance remain open. Base: `e671d8c62e012646e5632521ae78c6970a987f9c`.

## Bounded contract

An explicit Check action uses the selected named profile and its route-bound stored
credential through app authority and the existing provider adapter/completion path.
The diagnostic contains fixed text, no editor/workspace content, and requests at
most eight output tokens. A response establishes connection behavior only; it does
not qualify model identity, prediction usefulness, or the provider service.

Manual and offline builds refuse before invocation. Selection/configuration/status
refresh remain non-networking. The app's policy ceiling and existing provider lane
remain authoritative. There is no retry, alternate route or deterministic fallback.
The existing blocking transport has a 10-second connection and 120-second overall
timeout. Cancellation discards the outcome and remains visibly pending while the
transport drains; it must not imply that a socket has already stopped. Route/key
changes and Manual transition remain guarded while a named request is live.

The named pilot remains owner-selected Token Plan OpenAI-compatible endpoint
`https://token-plan-sgp.xiaomimimo.com/v1`, model `mimo-v2.6-pro`. Local controlled
protocol peers substitute only this external service for component verification.
No personal credential, remote call, installation, or native GUI run is authorized
or required by this implementation slice.

## Planned verification

- Public AppComposition plus controlled loopback protocol peer: exact route/model,
  fixed bounded payload, checking/success/failure, cancellation/drain, stale route,
  Manual/policy denial, and absence of credential/response/editor-content leakage.
- Existing desktop action and headless renderer seams: Check/Cancel controls,
  app-produced status refresh and stale-route refusal; preserve key/editor isolation.
- Offline compile, changed-file formatting, diff check and evidence docs hygiene.

All Cargo commands use `-j 2` and
`--target-dir D:/legion-ide-2026-workers/wave1-provider-health/target`.
Checks are recorded below after execution; no planned item is a reported pass.

## Results and remaining gates

The public app API, provider-specific desktop actions and headless controls are
implemented. The worker reuses `complete_product_chat`, existing configured adapters,
the policy broker and the shared product request lane. It retains only a boolean
outcome; provider response/error bodies do not enter UI status, config, or the AI
rail. Typed states distinguish idle, checking, cancellation requested, cancelled,
succeeded and failed. Success remains explicitly model-unqualified.

### Red-to-green evidence

- `connection_check_uses_selected_route_fixed_prompt_and_truthful_lifecycle`:
  initial compile red for the absent public API/state; green 1/1 after the first
  app slice. The real loopback peer observes the exact request path, selected
  model, synthetic bearer key, fixed messages, eight-token limit and disabled
  thinking. Manual sends nothing and editor bytes remain unchanged.
- `connection_check_cancellation_drains_then_discards_late_success`: initial
  compile red for the absent cancellation API; green 1/1. While the peer holds
  the request, cancellation remains pending, route/key changes and Manual are
  refused, and a late success becomes Cancelled without exposing response text.
- `connection_check_controls_refuse_manual_and_show_cancellation_through_frame_polling`:
  runtime red `missing control: Check connection`; green in the 9/9
  `provider_settings` suite after provider-only UI wiring. Actual headless
  pointer controls exercise Check, Manual refusal, Cancel and completion polling.
- `connection_check_policy_downgrade_waits_for_actual_request_drain`: runtime red
  expected Assist but observed Manual over a live request; green 1/1 after the
  provider-specific policy cancellation/drain guard. A subsequent source review
  identified the result-handoff lane gap; the same public regression now withholds
  the app pump after the peer responds to exercise that boundary. It reproduced
  red `hold the shared lane until policy reconciliation`, then passed 1/1 after
  retaining the existing reservation in the app job until reconciliation. This
  new supported failure was the reason to rerun that affected check; the earlier
  successful suites were not repeated.

Additional new app cases passed 2/2:
`connection_check_refuses_stale_missing_credentials_and_policy_without_requests`
and `connection_check_http_or_malformed_failure_never_falls_back_or_exposes_payloads`.
They exercise unselected/stale routes, missing credentials, signed policy denial,
HTTP 401, malformed JSON, empty completion, no retry and no payload leakage.

Existing selected-profile regressions passed 4/4 (exact endpoint/model, HTTP refusal,
redirect refusal, stale ghost invalidation). The existing cancelled-request
credential/Manual drain regression passed 1/1. The desktop suite's other eight
tests preserve metadata persistence, stale credential forms, masked entry/editor
isolation, explicit status refresh without per-paint keyring reads, and errors.

### Exact check commands

Executed from `D:/legion-ide-2026-workers/wave1-provider-health`:

```powershell
cargo test -j 2 -p legion-app --test explicit_provider_configuration connection_check_uses_selected_route_fixed_prompt_and_truthful_lifecycle --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
cargo test -j 2 -p legion-app --test explicit_provider_configuration connection_check_cancellation_drains_then_discards_late_success --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
cargo test -j 2 -p legion-desktop --test provider_settings connection_check_controls_refuse_manual_and_show_cancellation_through_frame_polling --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
cargo test -j 2 -p legion-app --test explicit_provider_configuration connection_check_policy_downgrade_waits_for_actual_request_drain --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
cargo test -j 2 -p legion-desktop --test provider_settings --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
cargo test -j 2 -p legion-app --test explicit_provider_configuration connection_check_ --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target -- --skip connection_check_uses_selected_route_fixed_prompt_and_truthful_lifecycle --skip connection_check_cancellation_drains_then_discards_late_success --skip connection_check_policy_downgrade_waits_for_actual_request_drain
cargo test -j 2 -p legion-app --test explicit_provider_configuration selected_profile_ --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
cargo test -j 2 -p legion-app --test explicit_provider_configuration cancelled_request_keeps_credentials_and_manual_transition_blocked_until_worker_drains --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
rustfmt --check --edition 2024 --config skip_children=true crates/legion-app/src/lib.rs crates/legion-app/src/provider_configuration.rs crates/legion-app/tests/explicit_provider_configuration.rs crates/legion-desktop/src/bridge.rs crates/legion-desktop/src/workflow.rs crates/legion-desktop/src/view/interactive_fields.rs crates/legion-desktop/tests/provider_settings.rs
cargo check -j 2 -p legion-desktop --no-default-features --features offline --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
rustfmt --check --edition 2024 --config skip_children=true crates/legion-app/src/provider_configuration.rs crates/legion-app/tests/explicit_provider_configuration.rs
cargo run -j 2 -p xtask --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target -- docs-hygiene
git diff --check
```

Nineteen distinct tests passed: five new app contracts, five existing app contracts,
and nine desktop provider settings tests. Scoped formatting passed, with only the
two affected files rechecked after the handoff repair. Offline desktop compilation
passed before that final reservation-lifetime refinement (the final refinement
compiled and passed through the targeted app regression); that successful offline
check was not repeated. The offline build reported 42 app unused/dead-code warnings;
the desktop build also emits the existing `vendor/epaint/src/tessellator.rs:2326`
float literal fallback warning. No warning cleanup was attempted.
Documentation hygiene passed (exit 0, `documentation hygiene checks passed`);
tracked diff whitespace check passed (exit 0). This final result-only receipt
update does not change the frozen source/test hashes. No workspace-wide tests,
live calls, native launches, commits or merges were performed in this lane.

### Changed paths and ownership

- `crates/legion-app/src/provider_configuration.rs`: app-owned check lifecycle,
  policy authorization, cancellation, route revision and metadata projection.
- `crates/legion-app/src/lib.rs`: provider state export/field/init and a narrow
  provider-check branch in policy-ceiling installation, plus the reviewed-policy
  admission repair described below.
- `crates/legion-app/tests/explicit_provider_configuration.rs`: public protocol
  and lifecycle contracts above.
- `crates/legion-desktop/src/bridge.rs`: two provider action variants and Noop arms.
- `crates/legion-desktop/src/workflow.rs`: provider action handling/status and
  nonblocking frame polling only.
- `crates/legion-desktop/src/view/interactive_fields.rs`: explicit Check/Cancel
  controls and app-produced health state.
- `crates/legion-desktop/tests/provider_settings.rs`: public headless UI contract.
- `plans/evidence/ticket-040-provider-connection-check.md`: this receipt.

No `view.rs`, MCP, save workflow, provider transport, dependency, shared tracker or
integration edits. Pauli composes the separately advancing integration work.

Frozen source/test SHA-256 values (2026-10-09):

| Path | SHA-256 |
| --- | --- |
| `crates/legion-app/src/lib.rs` | `dd547985ca71198b8b676cbdc711f6564957c768a9bece18a0901cac30423cbf` |
| `crates/legion-app/src/provider_configuration.rs` | `321d0f37b031c1e4c4d6f835b5331fd9410b203fe6ee735a5354832366a57488` |
| `crates/legion-app/tests/explicit_provider_configuration.rs` | `9bb9c281d70df4e5ecf1fc9c1cdb7fdbbca9c34cf0a8db6246ffbde89f3534d4` |
| `crates/legion-desktop/src/bridge.rs` | `21b9a99eb804461aa20f21622b6471f464b77fdf5b8501fe7739fee1463a0ba4` |
| `crates/legion-desktop/src/workflow.rs` | `cea908be30ea3424e4ac40c8bdfb8873f96c0835037154b57210125947f3bada` |
| `crates/legion-desktop/src/view/interactive_fields.rs` | `cdf562499b5818ae678655e682a0654588f7e2c0e01c20a80a27ba9af27c6643` |
| `crates/legion-desktop/tests/provider_settings.rs` | `80c045c162e25c63ffbbb241481917899fb54dee62e1991d8185a950c8ac5eb3` |

### Remaining gates

Pauli independently reviewed the final policy repair and related admission guards:
PASS for this bounded implementation. The reviewed worker commit was
`56fb55f2b55ea3cee5cf8454fe149c36534e99e8`, integrated as
`ee6896f1e340113cfa1cddbd873f2bc7cf0cb8e7`. Native profile input/screenshots, route-bound
credential persistence through the actual UI, authenticated Token Plan behavior,
endpoint/model compatibility and provider usefulness remain unqualified. No
personal credential, live remote call or native GUI launch was used. This patch
does not close ticket 40 or promote any acceptance. Transport cancellation is
discard-and-drain, not socket abort; a hung peer remains bounded by the existing
transport timeout. Only Pauli may commit or merge coordinated work.

## Pauli policy-admission HOLD and bounded repair

The initial independent review held one blocker: installing a Manual-only ceiling
while a provider check drains honestly retains Delegate/Automate display mode,
but the mode-only Delegate admission gate still admitted a new background worker.
Displayed drain state must not authorize new work above the installed ceiling.

`connection_check_drain_cannot_authorize_new_work_above_installed_ceiling` reproduced
the public failure before the repair: `Delegate: background Delegate must be denied
at policy admission, got None` (the admission returned `Ok`). The intentional red
case used the existing scripted tool provider and drained its accidentally admitted
worker before asserting, keeping all effects inside the disposable test workspace.

The repair adds a private app admission check against the installed ceiling to new
Delegate background/synchronous/plan/chat work, Assist explain/proposal/inline
request/acceptance, Automate execution, cloud enable/submit and explicit connection
checks. Automatic after-edit predictions skip admission when the ceiling denies
the retained mode. Existing mode-only lifecycle gates remain unchanged so Cancel,
Dismiss, human denial and kill-switch paths remain available. Provider-check
transport ownership, deferred mode reconciliation and result handoff are unchanged.

The new regression passed 1/1 (both retained Delegate and Automate configurations).
It observes policy-specific refusal at public admissions, no delegated sandbox
allocation, successful cancellation/dismissal, the retained mode during the held
request, Manual only after drain, Cancelled result, and unchanged editor/disk bytes.
No previous successful test suite was repeated; aggregate distinct passing tests
are now 20, with the earlier 19 results retained at their recorded revisions.

Exact new/affected checks from the same worker root:

```powershell
cargo test -j 2 -p legion-app --test explicit_provider_configuration connection_check_drain_cannot_authorize_new_work_above_installed_ceiling --target-dir D:/legion-ide-2026-workers/wave1-provider-health/target
rustfmt --check --edition 2024 --config skip_children=true crates/legion-app/src/lib.rs crates/legion-app/src/provider_configuration.rs crates/legion-app/tests/explicit_provider_configuration.rs
git diff --check -- crates/legion-app/src/lib.rs crates/legion-app/src/provider_configuration.rs crates/legion-app/tests/explicit_provider_configuration.rs
git diff --no-index --check -- /dev/null plans/evidence/ticket-040-provider-connection-check.md
```

Only the failed new regression was rerun after the supported repair: red exit 1,
green exit 0, 1 passed / 14 filtered. Formatting passed for the three changed Rust
files. The delta changes those three files plus this receipt; all four desktop
files remain frozen at their previous hashes. Final composed offline/default
all-target builds are reserved for the coordinator/Pauli, not repeated here.

Source inspection also found direct retained-mode checks in named MCP
activation/probe/runtime admission. This was reported to the coordinator; Socrates
owns that separately changing subsystem. This repair adds no MCP references or
MCP edits. Integration owns composition of both drain guards/reconciliation and
MCP admission enforcement. The later [composed-wave checkpoint](ide-2026-provider-mcp-integration.md)
records independent composition review, the four-branch shared-drain regression,
and final default/offline compilation. No commit or merge was made by this worker.
