# Ticket 63 bounded named MCP peer checkpoint

Status: independently reviewed bounded HTTP core partial delivery; ticket 63 is not resolved.

## Scope and seams agreed in the assignment

Base `f4dbeed`, branch `codex/ide-2026-ticket-063`. Ticket 01 is resolved.
The source-pointer note is navigation, not acceptance authority. This slice uses
ticket 63, the approved spec, architecture authority boundaries and ADR-0039.

- `AppComposition` configure/inspect: named identity, pinned role/version/transport,
  declared authentication scopes and metadata-only privacy. Catches invalid and
  stale configuration; does not prove durable persistence or native input.
- `AppComposition` activation/health against a controlled protocol peer: reuse
  `McpTransport`, `McpClient` and `AppMcpClientToolRuntime`. Catches Manual and
  network/process denial, missing credentials, negotiation mismatch and endpoint
  faults. It does not qualify an independently operated real peer.
- `AppComposition` revoke through `SecretStore`: invalidate the app grant/runtime
  before attempting secret deletion, expose deletion failure separately, and deny
  reuse of stale grants. Does not claim remote token invalidation or concurrent
  cancellation/recovery (ticket 66).

Client HTTP activation is the bounded runtime path. Client stdio and local server
stdio configuration are validated; activation remains unsupported where process
containment or external-client qualification is absent. No inference invocation,
UI, persistence, OS-keyring inspection or real-peer installation is in this slice.

## Planned verification

One public-interface test per red/green cycle, running only that newly introduced
test after its implementation. Controlled loopback HTTP transcripts are the
independent protocol oracle; in-memory credentials contain synthetic values only.
After the core checkpoint: affected existing MCP contract tests once, formatting,
targeted clippy, offline app compilation, dependency and docs gates once. Merge
the current integration tip before handoff; independent review precedes commit.

## External qualification prerequisites

Ticket 64 needs a selected and pinned independently operated MCP server, endpoint
or process/runtime, protocol/tool capabilities, scoped credentials and approved
execution environment. None is selected or provisioned here. Ticket 65 needs a
named external client and compatible server-role host/runtime. Native GUI input,
OS containment, live endpoint qualification and release gates are not proven by
these local contracts. No acceptance checkbox is credited from component tests.

## Results

The worktree was clean at `f4dbeed` before edits. The coordinator reserved the
MCP-only `lib.rs` module/state/initializer and register/invoke guards. Integration
tip `77166f7` was fast-forwarded with `git merge --ff-only --autostash
codex/ide-2026-integration`; the automatic WIP restoration succeeded without
conflicts. No implementation commit was created. Provider-40 configuration and
completion changes were preserved.

Before handoff, the same fast-forward/autostash command merged native-driver-only
integration tip `177caa5`. The changed-file list contained only driver/evidence
files and no MCP overlap. WIP restoration again succeeded without conflicts;
passed runtime checks were not repeated merely because that tip advanced.

Implemented files are the new app and protocol `named_mcp_peer.rs` modules, the
new app integration test, MCP-only changes in the app/provider/protocol `lib.rs`
files, this evidence and the ticket comment. No dependency was added. The runtime
reuses the existing `ai` feature's MCP substrate; no inference provider is selected,
required for negotiation, or invoked by the named-peer module. This does not claim
that the existing provider crate is separated from inference at Cargo-feature level.

Configuration pins `2025-11-25`, Legion client HTTP/stdio or local server stdio,
explicit None/Bearer authentication, declared credential scopes and MetadataOnly
privacy. HTTP requires HTTPS except loopback; userinfo, query credentials,
fragments and redirects are rejected. Named HTTP disables environment proxies.
Grant and configuration revisions bind the exact app-owned endpoint. Secrets use
only `legion-mcp-peers/<peer-id>:<binding-digest>`, never a provider alias or
environment fallback. The binding digest includes the exact endpoint/launch,
authentication route, scopes, role, protocol, transport family and privacy contract;
display labels are excluded. An endpoint/auth/scope change requires its own
explicitly stored credential. The original peer-ID-only prototype key is never
used as a fallback. Revocation deletes only the current reviewed binding; inactive
credentials for prior bindings are not silently migrated or deleted.

The controlled peer transcript independently observed `initialize`,
`notifications/initialized`, `tools/list`, `ping`, then a second health `ping`.
Pinned protocol and synthetic bearer authorization were present on each request.
Manual/permission/credential denial produced no TCP connection. Negotiation
mismatch, wrong response ID and redirects failed closed. Reconfiguration retires
the old runtime/grant. Grant revocation precedes credential deletion; failed
deletion leaves health Revoked and is exposed separately without raw store errors.
An unrelated registered runtime cannot replace a named-peer-owned runtime.
Discovered tool descriptions and peer-selected receipt labels are not retained;
existing app/client permission and proposal paths remain the tool authority.

### Exact TDD commands and logs

All commands ran from `D:/legion-ide-2026-workers/ticket-063`. Every row uses
`cargo test -p legion-app --test named_mcp_peer <test-name> -- --exact` unless
the row explicitly says otherwise. Logs are outside the repository under
`D:/legion-ide-2026-notes/`. Synthetic test tokens are not personal credentials.

| Test name | Red observation/log | Green observation/log |
| --- | --- | --- |
| `configure_named_peer_in_manual_mode_exposes_only_metadata` | Missing app/protocol module and methods; initial execution transcript (not separately saved as a file) | 1 passed, `ticket63-green1.log` |
| `unsupported_or_ambiguous_peer_contract_is_rejected_without_replacing_configuration` | Unknown protocol accepted, `ticket63-red2.log` | 1 passed, `ticket63-green2.log` |
| `activation_requires_nonmanual_mode_current_transport_grant_and_peer_credential` | Missing activation/grant APIs, `ticket63-red3.log` | 1 passed, `ticket63-green3.log` |
| `named_http_peer_negotiates_pinned_protocol_and_scoped_auth_without_inference` | Missing probe API, `ticket63-red4.log` | First green build failed E0507 on moving a borrowed trust value, `ticket63-green4.log`; clone repair then 1 passed, `ticket63-green4-repair1.log` |
| `grant_and_credential_revocation_disable_peer_and_invalidate_old_revision` | Missing revoke APIs, `ticket63-red5.log` | First post-integration run ABORTED `0xc0000409`, `ticket63-green5.log`; diagnostic command below passed, `ticket63-green5-diagnostic.log` |
| `secret_deletion_failure_is_redacted_and_cannot_restore_a_revoked_runtime` | Missing explicit privacy metadata contract, `ticket63-red6.log` | 1 passed, `ticket63-green6.log` |
| `negotiation_mismatch_malformed_health_and_redirect_disable_runtime_without_fallback` | Additional contract coverage against implemented behavior; no manufactured red | 1 passed, `ticket63-fault-contract.log` |
| `reconfiguration_retires_old_grant_and_stdio_roles_require_permission_and_containment` | Additional contract coverage against implemented behavior; no manufactured red | 1 passed, `ticket63-revision-contract.log` |

The affected diagnostic invocation was:

```powershell
$env:RUST_BACKTRACE='1'
cargo test -p legion-app --test named_mcp_peer grant_and_credential_revocation_disable_peer_and_invalidate_old_revision -- --exact --nocapture
```

**Unresolved runtime observation:** the first run aborted before reporting an
assertion with Windows `STATUS_STACK_BUFFER_OVERRUN` (`0xc0000409`). No runtime
repair was made or root cause established. The diagnostic rerun passed after the
independently planned privacy-contract addition; that successful invocation does
not repair, erase or retroactively pass the aborted run. No unrelated retry was
performed. This remains an independent-review/platform concern.

### Planned final checks

| Exact command | Result | Log under notes directory |
| --- | --- | --- |
| `cargo test -p legion-ai-providers --lib mcp` | 9 passed | `ticket63-provider-mcp-regression.log` |
| `cargo test -p legion-app --test legion_workflow_integration mcp` | 3 passed | `ticket63-app-mcp-regression.log` |
| `cargo test -p legion-ai-providers --test mcp_ga_conformance` | 4 passed; controlled fixtures only | `ticket63-mcp-conformance.log` |
| `cargo clippy -p legion-app --lib --test named_mcp_peer -- -D warnings` | first run failed `collapsible_if`; equivalent let-chain repair then passed the same affected command once | `ticket63-clippy.log`, `ticket63-clippy-repair1.log` |
| `cargo check -p legion-app --no-default-features --features offline --lib` | passed, exit 0; 42 offline-build warnings in existing app code, not a warnings-clean offline claim | `ticket63-offline-check.log` |
| `cargo fmt --all --check` | passed, exit 0 before mechanical clippy repair; not repeated | `ticket63-fmt-check.log` (empty on success) |
| `cargo run -p xtask -- check-deps` | passed, exit 0 | `ticket63-check-deps.log` |
| `cargo run -p xtask -- docs-hygiene` | passed, exit 0 | `ticket63-docs-hygiene.log` |
| `git diff --check` | passed, exit 0 before review delta; not repeated | `ticket63-diff-check.log` (empty on success) |

### Independent review delta (Euclid HOLD)

Review found two concrete defects in the checkpoint; the initial green component
tests did not establish either security property. Both were addressed within the
named-peer module/tests, with no general provider or AI completion edits.

- **P1 credential binding:** the peer-ID-only lookup could send endpoint A's bearer
  to newly configured endpoint B after a network grant. `SecretReference` now
  hashes the reviewed route/authentication/scopes contract described above. All
  existing synthetic credential setup sites were updated for this API; they were
  compiled by the new focused commands, not rerun as a full suite. The independent
  two-endpoint transcript sees no request to B until a B-specific token is stored,
  then sees only B's token. Changing B's declared scopes again blocks activation.
- **P2 transport error retention:** raw reqwest endpoint/path text reached the
  existing workflow adapter's failure labels. The named transport now maps every
  underlying send failure to the fixed label `named MCP endpoint unavailable`.
  The regression runs an actual existing MCP workflow: network activation alone
  does not call the tool; explicit app-owned tool permission precedes a real
  `tools/call`; the peer drops that connection. Public decision-feed and blocked
  worker failure metadata then contain the failure category without URL or `/rpc`.

Exact affected commands, run from the same ticket worktree:

```powershell
cargo test -p legion-app --test named_mcp_peer changing_endpoint_or_scopes_cannot_send_a_previous_binding_bearer -- --exact
cargo test -p legion-app --test named_mcp_peer named_tool_transport_failure_retains_only_redacted_workflow_metadata -- --exact
```

| Regression | Red log and result | Green log and result |
| --- | --- | --- |
| Two endpoints / changed scopes | `ticket63-review-p1-red.log`: assertion failed because B activation returned Ready using the old binding | `ticket63-review-p1-green.log`: 1 passed |
| Failing named tool / retained metadata | `ticket63-review-p2-red.log`: assertion failed because the raw endpoint reached failure metadata | `ticket63-review-p2-green.log`: 1 passed |

The two owned Rust files were formatted with
`rustfmt --edition 2024 crates/legion-app/src/named_mcp_peer.rs crates/legion-app/tests/named_mcp_peer.rs`.
Per coordinator instructions, no prior successful runtime/gate check was repeated
after this delta; those gate results apply to the earlier checkpoint. The two new
regressions are the executed verification of this delta. The coordinator subsequently
reported Euclid's delta review PASS and explicitly authorized the partial commit.

### Authorized handoff

After review PASS, the coordinator requested an immediate merge of integration
`928275e` and the reviewed partial commit, with no new tests. The fast-forward
`git merge --ff-only --autostash 928275e` succeeded and restored the MCP WIP without
conflicts. Provider-40 session-settings persistence and the protocol/app shared-file
changes were preserved; no provider-configuration or completion file was edited by
this worker. The partial commit contains only the eight owned MCP source/test/glue,
ticket and evidence files. Prior passed checks were not repeated merely for this
integration merge. The review delta is covered by its two recorded focused commands;
the earlier gate results retain their stated checkpoint scope. The Windows
`0xc0000409` abort remains unresolved. No ticket-63 acceptance checkbox is credited.

### Remaining slice and qualification limits

- Only JSON-response HTTP activation/health/tools discovery is in this checkpoint.
  MCP session-header/SSE operation, resources/prompts qualification, named tool
  execution against a real peer and process containment are not established here.
- Stdio configuration and process grants are observable, but no configured process
  is spawned: activation explicitly reports UnsupportedEnvironment. Legion server
  role activation also remains unsupported pending the existing server contract's
  app wiring/external-client qualification in ticket 65.
- Local revoke disables future runtime use and invalidates old revisions. It does
  not invalidate a token at the remote service, observe out-of-band SecretStore
  changes, or prove termination of an in-flight blocking HTTP request (ticket 66).
- State is app-memory only; no UI or durable configuration migration is included.
  Native input, independent real-peer selection/version/endpoint/runtime, platform
  containment, live authentication-scope enforcement and release gates remain open.
- The eight initial focused tests and two review regressions were run individually;
  no claim of a full-suite,
  three-platform, native, live-provider or ticket-63 acceptance run is made.
