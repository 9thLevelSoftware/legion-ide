# Ticket 40 explicit provider core evidence

Date: 2026-10-09. Worktree: `D:/legion-ide-2026-workers/ticket-040`.
Branch: `codex/ide-2026-ticket-040`; original base `87580fa`.
Integrated `dfcdd42`, `f8b375a` (including smoke repair `07d2187`), then
`f4dbeed`. Status: independently reviewed partial core implementation. The
coordinator reported Euclid's final core review PASS on 2026-10-09 and authorized
committing this bounded slice. Ticket 40 remains open; this is not native or live
acceptance.

## Scope and authority

Read the TDD skill, glossary, architecture authority boundaries, ticket/spec/
execution/reconciliation, and coordinator source pointers before implementation.
The user approved the existing AppComposition seam. App composition selects and
authorizes; existing ModelProvider adapters perform transport; existing SecretStore
stores keys. Provider output supplies no mutation authority. Saves and proposal
gates are unchanged. No parallel provider service, desktop edits, map/execution
edits, paid calls, production credential access, or native acceptance claims.

Changes are limited to app/provider code and tests, the app dependency/lockfile,
ticket 40, and this evidence document. Smoke-worker desktop hunks and worker 63's
MCP module/state/invocation hunks are outside this slice.

## Implemented contracts

- Bounded named metadata profiles: exact adapter/endpoint/model and explicit chat
  wire options; atomic validation/restore; select; JSON encode/decode only.
  Profiles expose adapter capability, transport locality, credential presence and
  last-request health metadata. No configuration or profile selection probes a
  host. Successful transport says `responded (model unqualified)`; adapters echo
  request model identifiers and cannot independently validate the actual model.
- Credentials use the existing secure-store port, keyed to profile name plus
  adapter/endpoint/model identity. Replace/revoke fail closed on store errors.
  Endpoint/model changes do not inherit a different route's key. Captured key
  snapshots are redacted in Debug and wrapped in Zeroizing; config contains no
  key field. Existing adapters internally copy credentials into their own request
  state; this is not a claim that every transient adapter allocation is zeroized.
- Named routes capture one immutable backend/key snapshot for the broker decision
  and request. Unknown cost remains unknown and subject to organization ceilings.
  Manual rejects AI requests before provider selection. Offline builds refuse a
  named real route. Auto now requires explicit selection. Deliberate deterministic
  test helpers remain available; unavailable/denied/busy selected routes produce
  no replacement fixture ghost, Assist edit, or Delegate answer.
- Selected profile edits invalidate completed ghosts. Each inline request captures
  an app-owned provider revision; acceptance checks it before applying editor
  edits, in addition to buffer fingerprints. Selection, credential changes,
  restore and legacy preference changes also invalidate authorization. Restoring
  old profile metadata does not resurrect old acceptance authority; an identical
  metadata update does not invalidate a current prediction.
- Logical cancellation does not claim transport termination. While a captured
  named worker still exists, credential/config mutation and entry into Manual
  remain blocked. The late result cannot become an accepted ghost.
- The existing HTTP client rejects redirects, including 307/308 body replay,
  instead of sending to a destination absent from the original broker decision.
  Profile validation rejects numeric aliases, noncanonical IP spellings and
  ambiguous hosts so transport URL normalization cannot change policy identity.

## Owner-selected subscription candidate

The owner's explicit OpenAI-compatible selection is the Singapore TokenPlan
endpoint below, superseding the earlier direct-service/PAYG candidate and endpoint
pending notes. The separately supplied Anthropic URL is not an automatic alternate.
Quota exhaustion remains unavailable, with no PAYG, gateway or model fallback.
Metadata example only; `selected: null` does not activate a default route or mode:

```json
{"profiles":[{"name":"mimo-sgp","provider_id":"openai-compatible","endpoint":"https://token-plan-sgp.xiaomimimo.com/v1","model":"mimo-v2.6-pro","max_completion_tokens":true,"disable_thinking":true}],"selected":null}
```

Coordinator's official-source notes: `D:/legion-ide-2026-notes/mimo-testing-candidate.md`.
The generic wire option emits documented `max_completion_tokens`, including
reasoning, rather than undocumented legacy `max_tokens`. The protocol oracle
captures a literal inline request with limit 128 and `thinking.type=disabled`.
Product chat/Assist uses the existing 512-token bound. These are requested wire
bounds, not evidence of subscription/model compliance, pricing or quality.
No key was requested or supplied here; secure subscription credential entry and
live qualification remain pending. Thinking-disabled behavior is explicit;
reasoning-enabled context/quality work is not certified by this slice.

## Red-before-green slice checks

Each row used exactly
`cargo test -p legion-app --test explicit_provider_configuration <name> -- --exact`
from this worktree. Each green ran one test. Tests use isolated temporary
workspaces, in-memory synthetic keys, and bounded loopback protocol peers; none
contact Xiaomi or another paid service.

| Exact test name | Observed red | Green oracle |
| --- | --- | --- |
| `explicit_unavailable_provider_refuses_instead_of_returning_fixture_prediction` | Explicit unavailable llama.cpp yielded a ready fixture. | Refuses; no ghost/editor mutation. |
| `named_profile_round_trips_without_credentials_and_rejects_secret_endpoints_atomically` | Public profile APIs absent. | Round-trips metadata through a second app; key absent from JSON; invalid endpoints leave config intact; revoke clears synthetic key. |
| `selected_profile_sends_exact_endpoint_and_model_to_protocol_peer` | Selected endpoint received no request. | Literal POST path/model and resulting peer ghost, without editor mutation. |
| `token_plan_profile_emits_bounded_completion_tokens_and_explicit_disabled_thinking` | Wire option fields absent. | Literal Bearer request, selected model, max_completion_tokens=128, no max_tokens, explicit disabled thinking. |
| `selected_profile_http_refusal_finishes_without_fixture_or_editor_mutation` | HTTP refusal became fixture ghost. | HTTP 429 ends request with no ghost/editor change; unavailable health. |
| `cancelled_request_keeps_credentials_and_manual_transition_blocked_until_worker_drains` | Revoke succeeded while cancelled transport still held its snapshot. | Held peer blocks revoke/Manual; drain permits revoke and Manual; no late ghost. |
| `selected_profile_redirect_cannot_send_prompt_or_credential_to_another_endpoint` | 307 reached a second, unauthorized endpoint. | Second listener observes no request. |
| `noncanonical_numeric_endpoints_cannot_replace_a_valid_provider_profile` | 0x08080808 accepted while transport would normalize to 8.8.8.8. | Hex/integer/octal/short/trailing-dot IPv4 and noncanonical IPv6 refused atomically. |
| `selected_profile_edit_invalidates_completed_ghost_and_rejects_old_acceptance` | Completed ghost remained active after selected endpoint/model edit. | Same-profile resave preserves ghost; edit clears it; changing back cannot accept old ID; bytes and undo history unchanged. |

The initial closed-port command used a partial filter with `--exact` and ran zero
tests; it is not red evidence. The corrected full name produced the observed red.
The redirect oracle initially failed on an inherited nonblocking socket; fixing
the peer to blocking with a read timeout exposed the actual redirect escape before
the production fix. Compiler failures are not presented as behavioral red results.

## Compiler repairs and escalation

Immutable `Configured(Arc<_>)` changed the backend from Copy to Clone. Runtime
wiring initially failed with 16 E0382 move errors. Mechanical clone repairs
reduced these to six and then two; a multiline CRLF replacement missed intended
sites. A Python edit attempt was unavailable on this host, and one command was
unnecessarily retried unchanged; neither is counted as progress or verification.
After two distinct failed repair attempts, repair work stopped and the evidence
was escalated. Independent read-only `sol_reviewer` advice is recorded at
`D:/legion-ide-2026-notes/ticket40-runtime-escalation-review.md`, session
`01a11ed2-118d-7700-8658-ae05f52c3d67`; initial compiler log is
`D:/legion-ide-2026-notes/ticket40-runtime-compile.log`.
The advised two `backend.clone()` fixes preserve the same authorized Arc snapshot,
without resolving a second route or copying the key. The subsequent route test
also exposed an unavailable OS keyring in the fixture; injecting the existing
in-memory SecretStore made the protocol test isolated rather than claiming native
keyring success.

Early independent Euclid review supplied three blockers: redirect escape,
numeric-host policy bypass, and stale completed ghost acceptance. All three now
have public behavioral red/green regressions. The coordinator reported Euclid's
final core review PASS: all three prior blockers fixed, no new material findings.
That final review approves this partial core slice, not the remaining ticket 40
native/live acceptance gates.

The final app unit build exposed two further test-only E0382 diagnostic borrows
(route descriptor and local inline-route loop). One supported clone repair fixed
both. The resulting 42-test run passed 40 and failed two old assertions that
expected canned content/old wording after failure. Those oracles now require no
edit/no reply and preserve the deliberately selected fixture assertion; only those
two failed cases were rerun. Their old Anthropic paths were replaced with an
explicit closed loopback route and synthetic secure-store key before running, so
the tests cannot resolve production credentials or incur paid calls.

## Final focused checks

Crosscutting route/revision/transport changes justified the explicitly requested
one-time recheck of earlier successful profile slices. No unchanged successful
check was repeated as a debugging loop. Commands below are from this worker root.

| Exact command | Result |
| --- | --- |
| `cargo test -p legion-app --test explicit_provider_configuration --test assist_inline_prediction_workflow --test control_trust_surfaces` | Passed: 9 profile, 6 Assist inline, 14 trust/proposal tests. Closed-port case additionally executes its one-test isolated child. |
| `cargo test -p legion-app --lib product_ai` | Initial compile failed on two test-only moves; supported repair compiled. 40 passed, two outdated failure oracles failed as described above. |
| `cargo test -p legion-app --lib product_ai_policy::org_ceiling::a_failed_` | Passed both affected failures after oracle fixes; the other 40 successful cases were not rerun. |
| `cargo test -p legion-ai-providers --lib -- --skip anthropic_tool_calling_live_smoke --skip openai_tool_calling_live_smoke` | Passed: 62 tests; one live Messages smoke ignored, two live tool-calling tests explicitly filtered out. |
| `cargo fmt --all --check` | Passed; targeted app/provider formatting applied first. |
| `cargo run -p xtask -- check-deps` | Passed dependency policy checks. |
| `cargo check -p legion-app --no-default-features --features offline` | Passed compilation, with 42 unused-import/dead-code warnings. These include new configured-provider fields/helpers excluded from live use in this build; no warning-clean claim. No network/AI call or native workflow qualification. |
| `git diff --check` | Passed for tracked changes. |

## Separate remaining gates

- Runtime implementation: core APIs/contracts above are partial. Native profile
  settings/typed intents, exact selection display, durable metadata persistence,
  credential entry/replace/revoke UI and explicit connection-check UI are deferred
  to the coordinator's next bounded slice. JSON round-trip is not reopened disk
  persistence. No new desktop flow has been exercised or shown in a screenshot.
- Secure credentials: OS keyring adapter is reused but native entry, persistence,
  replacement/revocation and unavailable-store UX need attended platform evidence.
  Route edits leave old route-bound secure-store entries unreachable from the
  edited profile; account cleanup/migration policy is not implemented here.
- Adapter gaps: configured Ollama's existing adapter does not transmit the loaded
  key, so an authenticated remote Ollama profile is not qualified. Capabilities
  are adapter metadata, not tested FIM/model capability. Configured completions
  currently publish one chunk rather than qualifying native streaming.
- Live acceptance: the owner-selected endpoint/model is known; a dedicated secure
  subscription key and concrete paid/live-call authority are absent. No claim of
  token-plan auth success, actual model identity, subscription quota behavior,
  cost compliance, native interaction, useful real prediction or live quality.
  These are blocked prerequisites, not failed local tests or passed acceptance.
- Release/native: coordinator's independently frozen candidate packaging/smoke
  evidence does not include this core change and cannot qualify its provider UI/runtime.

Ticket 40 acceptance boxes remain unchecked until its complete native and live
outcome is independently evidenced. No ticket-map or execution status promotion.
