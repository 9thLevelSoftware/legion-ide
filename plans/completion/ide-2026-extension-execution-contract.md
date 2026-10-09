# IDE 2026 finite extension execution contract draft

Date: 2026-10-09. Ticket: [155](../../.scratch/ide-2026-planning/issues/155-ratify-one-finite-extension-execution-contract.md).
Source baseline: `e671d8c62e012646e5632521ae78c6970a987f9c`.
Status: proposed bounded contract; root independent review PASS for bounded
documentation only. Engineering work and the external prerequisites below remain pending. This document grants
no runtime activation and does not resolve ticket 155 or promote canonical acceptance.

## Scope and existing decisions

The approved [migration contract](ide-2026-migration-contract.md) supplies the
two installed extension pins below. Native Windows/Rust edit, language, build,
debug and Git equivalents remain the initial pilot route; CodeLLDB's extracted
adapter is not a compatible installed extension. Neither named extension is a
prerequisite for Manual pilot acceptance.

The [S5 plan](../../docs/superpowers/plans/2026-09-04-ai-team-completion.md),
[extension scope audit](extensions-scope-audit.md) and
[canonical requirements](requirements.json) retain all eleven
`COMP-SCOPE-FAMILY-17-01` through `-11` outcomes. The finite first scenarios below
bound implementation and qualification; they do not remove required WASM,
Node, web-worker, webview, notebook, custom-editor or storage work. An uncovered
required row stays unavailable with its own prerequisite, rather than becoming
optional or being replaced by a metadata demonstration.

Assigned implementers may make ordinary runtime, IPC, numeric quota, storage and
recovery decisions within the approved scope, document their rationale and verify
them through the existing review/gates. They need no renewed user authorization
for those choices. A version must be selected from verified compatibility facts,
not invented; an actual artifact, credential or host must be available before its
qualification. Only a change to an approved obligation or authority boundary
needs a separate scope decision.

## Observed installed identities

Read-only inspection covered only each installed `package.json`, not personal
settings, credentials, executable code or live container services. Local paths
are under `C:/Users/dasbl/.vscode/extensions/`.

| Approved installed pin / manifest directory | Observed entrypoint and host constraint | Concrete unresolved prerequisite |
| --- | --- | --- |
| `ms-azuretools.vscode-containers` `2.5.2`; `ms-azuretools.vscode-containers-2.5.2` | `main.js`; `engines.vscode = ^1.109.0`; no `browser` entrypoint | Declares `vscode.docker` and `vscode.yaml` extension dependencies. Their exact resolved identities/artifacts and required API closure are not established here. |
| `ms-vscode-remote.remote-containers` `0.469.0`; `ms-vscode-remote.remote-containers-0.469.0` | `./dist/extension/extension`; `extensionKind = [ui]`; `engines.vscode = ^1.101.0`; no `browser` entrypoint | Declares proposed APIs `resolvers`, `tunnels`, `workspaceTrust`, `terminalDimensions`, `contribEditSessions`, `contribViewsRemote`, `contribRemoteHelp`; a stable command facade alone cannot establish compatibility. |

Manifest SHA-256 identities, in the same order:

- Containers: `88393444973f876e54f5ecdb1f36b39056c0db1724163c9d8ae04fcabcf743d9`.
- Remote Containers: `54278e24c20d31a824d65a8af50a23422a4d13d8fa4b99f548d94d2d8f810b90`.

These hashes bind manifest observations only. They are not complete VSIX/module
digests, publisher signatures, dependency locks or redistribution approval.
The engine ranges are declared compatibility constraints, not selected Node,
VS Code facade or browser-engine versions. No host version is invented here.

## Proposed finite scenario and API matrix

Command identifiers below are present in the pinned manifests. Their selection
as first qualification scenarios is proposed, not owner-ratified or implemented.
API clusters describe required adapter boundaries to resolve against the full
artifact; they are not a claim that manifest inspection found every runtime call.

| Row / downstream owner | Finite first outcome | Runtime, contribution and authority boundary | Qualification / unavailable condition |
| --- | --- | --- | --- |
| Signed WASM; 156/157, family 17-02/04/05 | One signed theme contribution through component `activate`; inspect its actual visible registration, disable/remove, reject tamper, quarantine crash, and recover an interrupted update | Existing `legion:plugin/plugin-host` WIT world; bounded `themes.register-theme`. Grammar and LSP-adapter WIT obligations remain separate rows, not inferred from theme success. | Real named signed guest bundle, immutable module digest, trusted signer and declared quotas are missing. Existing WAT/host tests are not that bundle. |
| Containers Node host; 162, family 17-07 | `vscode-containers.containers.refresh`, then `vscode-containers.containers.inspect` for one explicitly selected container; show bounded results and unavailable/denied/crashed-host states | Command registration/dispatch, view/tree contributions, configuration reads, lifecycle/disposal, cancellation and output; container access through a reviewed broker and explicit target identity | Must inventory the complete activation/API/dependency closure. No ambient Docker socket, shell or workspace access; no daemon/CLI version or container identity is selected here. |
| Remote Containers; 162 plus 169/173/174/183/189 | `remote-containers.reopenInContainer`, explicit local return through `remote-containers.reopenLocally`, then reconnect to the same declared workspace | Local UI-side host per manifest; reviewed resolver/tunnel/trust contracts and remote authority handshake. Product mutation remains on the identified local or remote workspace authority. | Requires ticket 169's host/image/workspace identities and policy. Proposed API behavior, provisioning, rebuild and reconnect remain blocked until their contracts and real target are available. |
| Web-worker; 163, family 17-07 | One real named browser-entrypoint contribution, activation, observable result, denied API and restart | Versioned worker bridge; bounded messages, explicit capability broker and no ambient network or main-workspace access | Neither installed pin supplies a browser entrypoint. Representative artifact/version and worker engine are unselected. Do not relabel either Node target as a web-worker qualification. |
| Webview; 164, family 17-08 | One named extension's visible message round-trip plus origin/resource/CSP/message denial and recovery | `ExtensionUiChannel` proposal; UI projects bounded data, resources and messages; no direct app/editor/storage ownership | Representative artifact and renderer/isolation version unselected; existing Tier-3 classification is insufficient. |
| Notebook; 165/166, family 17-09 | Open/edit/save one notebook; run/interrupt one cell; restart with dirty recovery and conflict refusal | Stable document/cell IDs; task/terminal-owned kernel lifecycle; proposal-mediated file changes and bounded output | Extension/kernel/artifact versions and fixture unselected. A generic subprocess test cannot qualify notebook identity or native editing. |
| Custom editor; 167, family 17-10 | Open/edit/save/revert/backup one document; preserve both versions on external conflict and recover after restart | Stable document identity, editor-owned dirty state, app/proposal/workspace persistence and scoped backup | Representative artifact/document fixture unselected. No direct extension write replaces proposal review. |

Default unsupported scope for this first contract: arbitrary Marketplace/VSIX
execution; undeclared API methods and proposed APIs; arbitrary activation events;
unreviewed dependency auto-installation; automatic image pulls/provisioning; raw
filesystem/process/socket access; and provider/AI actions granted by extension
installation. Containers also declares `languageModelTools`; that declaration
grants no inference or tool authority. Missing required activation APIs block the
affected extension before activation, with an explicit diagnostic, rather than
silently discarding contributions or claiming partial activation as success.

## Host protocol and lifetime requirements

Reuse protocol DTOs, `legion-security`, app-owned extension management and
existing proposal/storage services. `VsCodeExtensionHostSession` and
`extension_host_session_for_manifest` are passive planning descriptors at this
baseline. `VsCodeApiCallEnvelope` names an API/capability and audit identity; its
presence is not proof of an executable transport or complete S5 service contract.

The S5 `ExtensionHostSpec`, instance/lifecycle/storage/contribution/UI-channel
contracts remain proposed follow-up work. Before implementation activation, the
versioned request/response contract must bind extension ID and full artifact
digest, host runtime/version, instance generation, workspace identity, grant
revision, call ID, deadline, payload bounds and existing nonzero event metadata.
Reject unknown schema/API, stale generation, revoked grant, duplicate/replayed
mutation, invalid workspace and oversized input before privileged work. On host
restart, old registrations and late responses cannot become current again.

WASM ABI `1` and the current `grammars`, `themes`, `lsp` WIT interfaces are the
existing basis, not authority to add WASI. Preserve manifest-declared ceilings
for fuel, wall time, memory pages, storage bytes, host calls, events and output,
bounded by host policy. Node/worker IPC frame/queue/inflight limits, termination
deadline, crash retry ceiling and storage/key limits need explicit finite values
in the reviewed host specification before launch. Unset bounds mean unavailable,
not unlimited. Selecting and verifying these finite values is authorized
engineering work, not a missing owner-approval prerequisite.

Lifecycle contract: verify artifact and permissions before registration; make
enable/disable/revoke observable; cancel requests and confirm host termination
before reporting stopped; quarantine crashes or failed cleanup. Stage updates
separately, bind migration/rollback to old and new identities, publish atomically,
and preserve the old executable/state on interruption. Expanded permissions need
fresh review; an update cannot inherit them implicitly. Removing an extension
revokes its registrations and execution before reporting storage disposition.

## Storage and UI authority

| Scope / ticket | Required identity and durable effect | Denial / recovery contract |
| --- | --- | --- |
| Workspace / 158 | Extension instance + workspace + namespace + key/schema; get/set/delete/clear through an app-composed storage service; restart persistence | Reject cross-workspace/extension keys, traversal, quota overflow, stale writes and malformed data; migration failure preserves originals. |
| Global / 159 | Extension identity + user/profile + namespace + key/schema, explicitly distinct from workspace state | No cross-extension sharing or implicit workspace-source retention; bounded migration, export/deletion and restart evidence. |
| Secret / 160 | Existing secure-store service, bound to extension identity and approved scope; replace/revoke with observable metadata only | No secret values in ordinary settings, audit, global/workspace storage, diagnostics or backup. Test availability/failure and revoke independently of UI projection. |

Freeze exact quotas, retention, deletion-on-remove versus explicitly retained
state, backup disposition and schema migration before each storage path activates.
Raw source/provider payload retention remains consent-gated. Metadata-only
`PluginStorageRecord` substrate is not full workspace/global/secret persistence.
Webviews have explicit origins, resource roots, CSP and bounded messages;
notebooks use supervised task authority; custom editors retain existing save
fingerprint, buffer-version and workspace-generation checks.

## Policy and activation checklist

- [ADR-0019](../adrs/ADR-0019-wasm-plugin-runtime.md): no ambient authority,
  manifest/capability validation, bounded host calls and proposal-only mutation.
- [ADR-0047](../adrs/ADR-0047-extension-distribution.md): signed curated WASM
  and metadata-only declarative distribution. Classification tier labels do not
  qualify Node/web-worker execution. Runtime VSIX support needs an explicit
  accepted extension-host decision, not reinterpretation of distribution tiers.
- [ADR-0050](../adrs/ADR-0050-wasmtime-runtime-ratification.md) ratifies the
  engine, separately from product activation. Its decision names `46.0.2`, while
  this baseline's `Cargo.toml` and `Cargo.lock` name `48.0.5`. Reconcile the version
  authorization/supply-chain record before using this draft to claim current
  engine qualification; neither downgrade nor a fresh ratification is implied.
- [ADR-0038](../adrs/ADR-0038-os-sandbox-layer.md) is the isolation basis to
  adapt and evidence for each extension host. An application permission check
  alone cannot contain a Node process with ambient OS privileges.
- [ADR-0046](../adrs/ADR-0046-surface-expansion-freeze.md) is retired. Its
  retirement does not activate PR-VSC-002; the ledger and surface-specific gates
  still apply. The full-program requirement for Tier 3 remains retained.
- [Dependency policy](../dependency-policy.md), section 4: accepted ADR,
  explicit dependency entries, phase gate/owner, protocol and runtime contracts,
  ownership tests, sandbox evidence and ledger evidence must accompany activation.
  Keep `check-deps` and `deferred-surfaces` enforcement aligned with that change.
  The historical S5 plan's suggested ADR filename must be checked for numbering
  collisions when the actual ADR is created; this draft allocates none.

## Remaining work and acceptance handoff

Ticket 155 stays partial. Authorized engineering work includes the complete
activation/API/dependency inventory, compatible runtime/facade selection, exact
host/storage limits, lifecycle policy and S5-01 ADR/protocol/policy implementation.
Workers can make those bounded technical decisions without a fresh owner prompt;
they still need independent review and the applicable activation evidence.

Unestablished external prerequisites are full extension/dependency artifacts and
their distribution/trust evidence, a real signed WASM representative, named
web-worker/Tier-3 representatives, and actual remote/container qualification
resources. Installed manifest metadata does not supply them. Independent review
of this documentation checkpoint cannot resolve the entire S5-01 contract or
substitute for its authority-bearing implementation and qualification evidence.

Ticket 169 owns actual remote/container environment selection. Ticket 88 owns
the broader platform matrix. No missing host, engine, daemon, kernel, signer,
account or artifact is inferred from an installed manifest. Each qualification
row must bind immutable Legion and extension artifacts, declared configuration,
exact action and external result, denial/recovery evidence and reviewer identity.
Ticket 168 collects extension qualification; ticket 211 collects remote workflows.
Metadata traces, renderer tests and mock peers remain distinct evidence layers.

## Verification scope

Performed: read-only inspection of ticket 03, S5/canonical requirements, relevant
ADRs/policy, protocol/compatibility/WIT source and the two installed manifests;
SHA-256 capture of those manifests. No extension activation, network request,
installation, native action or runtime test. Only this document and issue 155 are
owned by this worktree. Canonical records, policy, ADRs and product source remain
unchanged. Whitespace is checked once after the draft; docs-hygiene is reserved
once for the composed wave. Root independently reviewed this author-owned draft:
PASS for bounded documentation only, with no runtime compatibility, ratification
or ticket-closure claim.
