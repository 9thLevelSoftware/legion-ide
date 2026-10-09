# IDE 2026 remote workspace identity and authority contract

Date: 2026-10-09. Ticket 169; M5 / S5-05 / A18. Review baseline:
`ee6896f1e340113cfa1cddbd873f2bc7cf0cb8e7`.
Status: accepted planning contract for ticket 169 after root independent review.
No runtime activation, provisioning, credential access, S5-05 implementation,
A18 product acceptance or canonical readiness promotion is established.

## Scope and source authority

The [approved specification](../../.scratch/ide-2026-planning/spec.md), including
all retained full-product families, M5 and A18, governs scope. The
[full-product design](../../docs/superpowers/specs/2026-09-04-product-completion-design.md)
and [S5-05/06/07 plan](../../docs/superpowers/plans/2026-09-04-ai-team-completion.md)
govern the remote implementation boundary. This attachment supplies ticket169's
engineering decisions; it does not declare those larger packages implemented.
[Reconciliation D9](ide-2026-reconciliation.md), the
[remote scope audit](remote-scope-audit.md), [requirements](requirements.json),
[matrix](matrix.json), [scenarios](scenarios.json) and
[dependency register](dependencies.json) retain their IDs and ownership.

| Existing requirements | Contract obligation | Downstream owner / evidence |
| --- | --- | --- |
| COMP-REMOTE-001/002/003/004 | Finite targets, authentication, environment and noninterchangeable workspace/service identities | 170/171/173; SC-REMOTE-SSH-CONNECT, SC-REMOTE-CONTAINER-CONNECT, SC-REMOTE-WORKSPACE-IDENTITY |
| COMP-REMOTE-005/006/007/016 | Files, save, watch/search, dirty preservation, explicit source transfer | 172/174/175/184/185; SC-REMOTE-FILES-EDIT-SAVE, SC-REMOTE-SEARCH, SC-REMOTE-EGRESS-PRIVACY |
| COMP-REMOTE-008/009/010/013/018 | Terminal, language, build/test/debug, Git and real termination | 176–179/186/187/190–210; SC-REMOTE-TERMINAL-TASKS, SC-REMOTE-LSP-SESSIONS, SC-REMOTE-BUILD-TEST-DEBUG, SC-REMOTE-CANCEL-CLEANUP, SC-REMOTE-GIT |
| COMP-REMOTE-011/012/014/015 | Extension/forward lifecycles, disconnect/reconnect, install/rebuild/rollback | 170/171/173/180–183/188/189; SC-REMOTE-EXTENSION-LIFECYCLE, SC-REMOTE-PORT-FORWARDING, SC-REMOTE-DISCONNECT-RECONNECT, SC-REMOTE-AGENT-INSTALL-LIFECYCLE |
| COMP-REMOTE-017 | Packaged native workflows and independent target effects | 211 / S5-15 / A18; no fixture or local-host substitution |

General save/conflict semantics remain COMP-PRES-001/003/004/008; language edits
remain COMP-LANG-004/007; build/test/debug remain COMP-LANG-011/012 and COMP-BTD;
Git remains COMP-SCM-001/002/003/006/009/010. Remote rows add target authority and
remote evidence, not replacement semantics. Ticket05's accepted local recovery
slice is a prerequisite, not remote save evidence.

## Selected qualification environments

These are finite design selections, not claims that hosts or images exist.
The target labels below are document labels, not new canonical CFG IDs. Actual
addresses, host keys, package revisions, image and agent digests are missing
external inputs; no invented fingerprint, credential or placeholder may pass
activation. The operator freezes one complete target manifest before each run.

| Target | Selected environment and access | Workspace/tool location | Qualification prerequisites |
| --- | --- | --- | --- |
| SSH-UBUNTU24-X64 | A separately administered Ubuntu Server 24.04 LTS amd64 VM on an explicitly allowlisted LAN/private endpoint, OpenSSH server from its maintained distro package, unprivileged run-scoped `legion-qual` principal. Outbound client SSH only; no jump host, password fallback, agent forwarding or public service exposure in the initial profile. | Separate disposable repository clone below `/srv/legion-qualification/<run>/<project>` with ownership limited to that principal. Language servers, runners, debuggers and Git execute on this host. Proposed capacity: 8 vCPU, 32 GiB RAM and 100 GiB disposable volume; actual resources and enforced per-job limits enter the manifest. | Assigned VM/operator; actual OS/kernel/package revisions; client SSH implementation identity; host-key fingerprint obtained independently from host administration; scoped signing credential reference; exact remote root/file identities; verified Linux x64 agent and tool artifacts. None supplied by this task. |
| CONTAINER-UBUNTU24-X64 | Linux amd64 OCI dev-container on a rootless Docker Engine 28.x qualification host, using the SSH target's authenticated operator boundary. Ubuntu 24.04 base, built into a dedicated qualification image and frozen by digest. Exact daemon/runtime/API versions must be frozen; `28.x` and an image tag alone are not executable pins. | Container-owned named volume mounted at `/workspaces/qualified`; a separate clone per run/project. No bind mount of the developer's local worktree, home, SSH directory, keyring or engine socket. Non-root UID/GID 1000, no privileged/host PID/network mode, dropped capabilities, no-new-privileges and enforced engine resource limits. Proposed maximum: 4 vCPU/16 GiB RAM/256 PIDs/64 GiB work volume; freeze and enforce actual limits before qualification. | Dedicated rootless daemon/context ID and owner; immutable image/config/build-context digests; scoped engine broker; volume ID and container ID; verified agent/tools; actual mount/user/network/resource inspection. No daemon is provisioned or assumed reachable here. |

The container format is a reviewed `.devcontainer/devcontainer.json` plus a
content-addressed build context where a build is required. Initial qualification
uses a digest-pinned image, explicit remote user/workspace mount, no features,
no automatic lifecycle commands and no implicit ports. Build/setup commands,
features, mounts, environment variables and network destinations introduced later
must appear in a changed configuration digest and a separately reviewed setup
plan. Parsing a configuration grants neither execution nor containment.

The first client is the already selected Windows 11 x64 pilot configuration in
[pilot02](ide-2026-pilot-configuration.md). Required client qualification retains
all four existing PRODUCT-JOURNEY cells: CFG-WIN11-X64-PRODUCT-JOURNEY,
CFG-MAC15-X64-PRODUCT-JOURNEY, CFG-MAC15-ARM64-PRODUCT-JOURNEY and
CFG-LINUX-UBUNTU-X64-PRODUCT-JOURNEY. Both targets must be exercised from each
required client; Windows success cannot qualify Mac or Linux. Remote Windows,
remote macOS and Linux arm64 agents are outside these initial target selections,
not substitutes for the required desktop client platforms.

For **each** SSH and container target retain Rust binary, Rust library, Rust
multi-crate workspace, TypeScript browser, JavaScript browser, TypeScript Node,
JavaScript Node, Python application and Python package journeys. The combined
TSJS-NODE matrix cell still needs separate TS and JS results. Use the exact
project/tool candidates in [ticket88's configuration attachment](ide-2026-language-platform-configurations.md),
then freeze Linux x64 artifacts, absolute tool paths, dependency locks, project
commits, runner/debug commands and expected results on the actual target.
Local Windows tool binaries or a guessed PATH executable cannot satisfy these
remote tool identities. Ticket211 must expand existing scenario/configuration
references with target-manifest digests; this document does not rewrite the matrix.

## Authentication, transport and mode policy

Connection configuration, workspace trust, credential use, tool execution,
proposal approval and source retention are separate decisions. A trusted
workspace or successful SSH login grants none of the others implicitly.

1. The app checks current mode, signed organization ceiling, trust, endpoint and
   per-service capabilities before constructing a connector. Unknown capability,
   denied target, missing manifest or ambiguous authority is refused before I/O.
   Remote defaults remain off. No inference provider or AI credential is required.
2. SSH verifies the independently enrolled host-key fingerprint before user
   authentication, agent bootstrap or workspace access. A changed key fails
   closed; user review creates a new authority enrollment and invalidates prior
   grants. No automatic trust-on-first-use or fallback host is permitted.
3. The local credential service owns an opaque reference scoped to target,
   principal and authentication method. Keys/signatures use an explicit brokered
   credential/agent interface, never credentials or private-key paths in command
   arguments, environment, configuration, logs or audit payloads. No ambient
   default SSH identity, forwarded agent or unscoped Docker socket reaches tools
   or extensions. Revocation stops new dispatch and invalidates resume authority;
   local revocation does not claim remote key deletion.
4. Select framed typed envelopes over an authenticated SSH execution channel for
   the SSH agent. The remote launch is a fixed reviewed agent entrypoint with
   framed input; no user-derived shell string. Container access uses a scoped
   engine broker and exec stream bound to the selected container, reached through
   the authenticated target boundary. Neither route exposes a product listener.
   SSH as ADR-0025's "accepted equivalent" to TLS/mTLS needs the activation ADR's
   explicit cryptographic/peer-binding review; existing mTLS tests do not qualify
   the SSH adapter. No plaintext or unauthenticated fallback is acceptable.
5. Negotiate protocol schema, agent version/digest, authority/root identity,
   service versions, limits and capability intersection before marking Active.
   Unknown versions and missing required services are unavailable, not emulated
   by local tools. SSH host authentication is not hardware attestation of an agent.
6. Manual keeps its zero-egress contract: this remote connection/provisioning
   profile is unavailable in Manual, including automatic reconnect and container
   bootstrap. Existing retained views and unsent edits remain usable offline.
   An explicit permitted non-Manual mode plus remote capability grants enables
   a future implemented connection; it never triggers inference automatically.
   The offline package never activates this profile. An air gap refuses the
   non-loopback target; container locality does not waive the containing route's
   egress decision. Do not extend the existing explicit Git-remote exception to
   SSH workspace sessions.
7. A mode/ceiling downgrade denies new work immediately and requests cancellation
   across connections, tools and forwards. Retain a truthful draining state until
   actual transport/child/listener termination is observed; never label live
   traffic Manual or report cancellation complete from a timer or stale reply.
   Reconnect never restores a grant that current policy has revoked.

Before connection, show the target, authenticated principal, requested root,
agent/environment identities, service permissions and source-transfer scope.
Authentication failures may perform only the authorized authentication exchange;
they must not bootstrap an agent or execute workspace commands. Secrets and raw
network frames stay out of diagnostics. Audit records retain validated nonzero
correlation/sequence, non-nil causality, operation/proposal and redacted identity
metadata, through the existing storage/observability authority.

## Identity and service ownership

An endpoint string, directory spelling, image tag or display label is never an
authority identity. The proposed remote identity contract binds:

```text
Authority: enrolled target + authenticated host/engine identity + principal
Environment: agent/package/schema + OS/architecture + tool/config digests
Workspace: authority + canonical remote root + filesystem/volume identity
Session: workspace + fresh session/connection epoch + policy/grant revision
Handle: session + service kind + opaque object ID + object/content version
```

Agent restart, root replacement or container rebuild changes the relevant epoch.
Paths are explicit RemotePath/RemoteFileHandle values, never implicitly converted
to a local PathBuf, file URI, local workspace ID or command working directory.
Reject absolute/path-traversal/symlink/mount escapes after target-side resolution;
revalidate canonical root and object identity at use, including case and Unicode
semantics on that target. A same-spelled local directory is a separate authority.

| Owner | Owns | Must refuse |
| --- | --- | --- |
| Desktop/UI | Target-labelled projections and typed user decisions | Filesystem writes, credential reads, process launch or ownership of remote session/editor text |
| App composition | Explicit activation, policy/trust, credential references, service routing, proposal lifecycle, authority-labelled editor sessions and reconnect reconciliation | Treating transport results as approval; choosing a local fallback target |
| Local editor | Unsaved buffer text/version associated with its exact remote handle and observed baseline | Claiming remote disk is saved before a matching commit receipt |
| Local WorkspaceActor/save authority | Local files and separately approved local recovery/export destinations | Receiving a remote handle as a local save path or fabricating local effects for remote success |
| Remote workspace/file service | Actual target filesystem, canonical paths, content versions/fingerprints, watch epochs and atomic guarded commits | Direct UI/provider/tool writes; mutation without the app-approved proposal and valid target-side preconditions |
| Remote agent supervisors | Target-owned LSP/debug/task/PTY/Git service instances, process-tree handles, bounded outputs and observed termination | Local execution fallback, arbitrary host scope, orphan continuation claimed stopped, extension ambient authority |
| Transport/connector | Authenticated framed delivery, correlation/order, bounded flow and replay checks | Editor/proposal authority, local disk writes, treating an acknowledgement as a durable file commit |

A remote save/apply carries proposal ID, operation/idempotency ID, principal,
capability decision and policy revision, exact target/session epoch, file identity,
expected fingerprint/content version, workspace generation, buffer version,
snapshot/verification subject and payload digest. Both app review and the remote
workspace authority check their relevant preconditions at commit time. Use an
atomic guarded replacement or refuse; no unsafe write fallback. Changed source,
dirty dependency, root, policy or proposal invalidates prior approval/evidence.
Remote refusal retains dirty text and the original remote baseline. A user may
explicitly create a local recovery copy through local proposal/save authority;
that copy is not a remote save or a silent retargeting.

Language/navigation/search results carry target, root, document version and
request epoch. Remote LSP edits become proposals; stale diagnostics/navigation
are labelled stale or discarded. DAP source paths and breakpoints resolve only
within the same remote authority. Build/test/Git results identify the actual
remote executable, repository/index/HEAD and source subject. A test pass is not
proposal approval; remote Git push/fetch requires its own destination/credential
grant, not the SSH workspace credential by default.

Tools and interactive commands that can mutate reviewed source run in a scoped
disposable target-side worktree/overlay; changes return as proposals. Approved
build outputs/caches have explicit separate roots and limits. An unrestricted
shell with direct canonical-workspace write access does not satisfy the
proposal-only contract. Agent/LSP/extension code cannot infer authority from
container existence or a user-visible terminal. The supervisor must enforce the
declared file/process/network boundary; inability to enforce it means unavailable.
Terminal display is transient and bounded; logs/source are not retained as audit.

Extensions retain [ticket155's execution contract](ide-2026-extension-execution-contract.md).
UI-side resolver code and target-side extension/tool processes have distinct
identities, hosts and capability grants; Remote Containers' manifest does not
authorize engine access. A port forward separately binds one approved target
service to a loopback-only local address/port with explicit lifetime, collision
checks, authorization and close evidence. Wildcard/public binds, reverse forwards
and general `remote.transport.listen` remain denied pending their own gate.

## Disconnect, reconnect, conflict and lifecycle rules

| Event | Required state and reconciliation | Forbidden shortcut |
| --- | --- | --- |
| Connection loss | Mark Disconnected/Offline; stop admitting writes/tools; retain dirty text, baseline and review context. Cached reads are visibly stale/read-only; new unsent text may remain a local draft. | Reporting online/save success from cached state or writing to a same-named local file |
| Commit response lost | Mark operation outcome unknown. Query the target's durable operation receipt using the original identity and payload digest after reauthentication. | Replaying the write because its acknowledgement was lost |
| Same-target reconnect | Revalidate host/principal/root, agent/schema, policy, checkpoint and target versions. Establish a fresh connection epoch and rebind verified handles. Resume reads/watch subscriptions explicitly; show watch gaps and take a new snapshot. | Accepting an old session token as fresh permission or automatically resuming queued execution |
| Changed host key, principal, root, volume or agent environment | Refuse automatic resume. Preserve drafts under their old identity; require explicit new-target enrollment/review and fresh baselines. | Relabelling old buffers/proposals as belonging to the replacement |
| External edit/delete/rename/permission loss | Compare actual remote fingerprints/file identity and workspace generation. Refuse conflicting apply; expose remote versus dirty/proposed content for deliberate merge/recovery. | Last-writer-wins, silent overwrite, treating path reuse as the same object |
| Duplicate request | Bind operation ID to authority, proposal and payload digest; return the durable prior outcome without another effect. Same ID/different payload is denied. If the receipt is missing, expired or unverifiable, remain unknown and reconcile actual state. | Unqualified exactly-once claims or retrying a non-idempotent tool command |
| Cancellation/IDE exit | Revoke new admission, signal the actual child/process tree or forward, and observe exit/close. Report unreachable termination as unknown; the remote supervisor enforces a bounded session lease and cleanup on expiry. Retained sessions restore stopped. | Reporting killed from a local flag or allowing autonomous work after IDE exit |
| Container rebuild | Review changed image/config/setup, close old handles, preserve dirty drafts and selected volume data, then verify new container/volume/root/tool identities before fresh review. | Replaying pre-rebuild writes or trusting a reused container name |
| Upgrade/rollback | Stage exact manifest/digest, validate compatibility and health, atomically activate, retain the previous verified version for rollback. Failed/low-disk install leaves the previous healthy version active or reports unavailable. | Download/launch on restore, partial upgrade claimed healthy, or automatic downgrade to an unverified binary |
| Close/remove | Show target and resources to stop; revoke credentials/grants/forwards; observe session children drained. Removal is separately scoped to that run's agent installation/owned disposable resources and must preserve workspace data and recovery copies. | Broad remote deletion or interpreting disconnect as permission to remove a volume/repository |

Durable operation receipts contain only identity/digests/status/version metadata,
with bounded retention advertised during negotiation. If receipt persistence
fails before an operation can be safely made recoverable, refuse new mutation.
Dirty text uses the existing dedicated work-preservation store, subject to the
remote source's local-cache/retention policy, not audit records. Do not silently
discard already-held dirty text if policy changes; stop new transfer and surface
an explicit permitted recovery/disposal path. Raw traces require separate consent.

## Activation and qualification gates

This contract preserves [ADR-0022](../adrs/ADR-0022-remote-edge-workspace-agent.md),
[ADR-0023](../adrs/ADR-0023-remote-transport-security.md),
[ADR-0024](../adrs/ADR-0024-remote-execution-boundary.md) and
[ADR-0025](../adrs/ADR-0025-production-remote-network-transport.md). Their fixture,
descriptor and production-direction acceptance is not packaged SSH/container
acceptance. The [authority boundaries](../../docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md)
and [dependency policy](../dependency-policy.md) still apply.

Before ticket170's real agent is activated, its owner must supply a separately
reviewed activation ADR (fresh unused number), protocol/service-handle contracts,
threat model, exact capability decisions, feature/default-off policy and matching
dependency enforcement. App composes local and target service ports; protocol
owns shared DTOs. Existing remote crates must not gain forbidden app/UI/editor/
project internals. If target-local file/process services require new edges, update
policy and `xtask check-deps` enforcement together in that implementation slice;
this planning document does not authorize a new backend or crate edge.

The activation slice needs failing-then-passing protocol/security tests for wrong
target/role/schema, swapped handles, revoked grants, replay, stale fingerprints,
canonical-root escapes, lost commit receipts, bounded queues and privacy. Real
agent process tests must observe files, actual child trees, termination, low-disk
and permission failure, restart, failed upgrade/rollback and cleanup. Exact limits
(connect/bootstrap/operation/lease times, frame/queue/output/cache bytes, processes,
CPU/memory/disk) must be populated and enforced; absent limits deny activation.
Negotiation takes the intersection of client/agent policy limits, never widens
them. Stale/oversized/unparseable messages cannot mutate state.

For A18, use the existing SC-REMOTE scenarios for both actual targets and each
required client/project cell. Observe SSH authentication/key identity on the
server, container/image/mount/process state through the host operator, exact
remote file bytes and Git state independently of Legion, and local workspace
bytes to prove no unintended local effect. Capture real LSP/test/debug responses,
wrong-target refusals, external conflicts, dropped responses/reconnect, revoked
forward absence and no surviving children after acknowledged termination.
Native input/accessibility and packaged artifact identity are required separately
from app/protocol tests; a missing rendered control is a failure, and an absent
host/artifact is a named blocked prerequisite. Never score either as passed.

Ticket211 aggregates these runs and unresolved defects. Product promotion uses
the existing [readiness ledger](../product-readiness-ledger.md), including applicable
PR-ENT-002/PR-VSC-002 gates and the required ADR, policy, tests and product evidence.
Root/Pauli own canonical registration, integration and final docs-hygiene. This
worker changes no register, map, index, execution record or status counts.

## Explicit conflicts, drift and disposition

| Record tension at this baseline | Disposition; no silent override |
| --- | --- |
| S5-05 names `ADR-0055-remote-workspace-authority.md`; [ADR-0055](../adrs/ADR-0055-language-artifact-materializer.md) already names the proposed language artifact materializer. | Do not overwrite or reuse its number. The activation owner allocates a free ADR ID and updates the plan/dependency references in the same future slice. No fictional ADR is linked here. |
| S5-05 is described as an ADR/protocol/dependency implementation package, while ticket169 is the approved narrower planning contract. The dependency register lists only COMP-REMOTE-004 and empty package prerequisites for S5-05. | Ticket169 can satisfy its documentation criteria without claiming all of S5-05 implemented/accepted. Root must reconcile the register's edges with S0-02/S0-06/local services/S4-01 and target requirements before activation; keep package and requirement IDs. |
| Existing matrix has client/platform cells but no pinned remote endpoint/image manifests; the September remote audit's "no scenarios" finding is historical, while SC-REMOTE scenarios now exist. | Reuse those scenarios and preserve the four client IDs. Attach the concrete target manifests required here; neither historical absence nor provisional client cells supply target qualification. |
| ADR-0022/0024 and Phase 7 evidence accept fixture files and execution descriptors; S5-06/07 require real remote services. | New service activation must explicitly extend the implementation boundary with an ADR, policy/dependency changes and external effects. Accepted harness evidence remains limited to its original scope. |
| ADR-0025 requires TLS/mTLS or an accepted equivalent; SSH/config planners validate metadata only. | This contract selects authenticated SSH framing as the proposed equivalent, with a required activation review. No claim that the planner or existing TLS carrier already implements it. |
| Manual/privacy records promise zero egress; docs/PRIVACY permits explicitly invoked Git remotes, and docs/MODES lists narrower AI/network exclusions. | Preserve the stricter zero-egress rule for this new remote-workspace profile; require explicit non-Manual activation without requiring inference. A future Manual remote exception requires an explicit mode/privacy ADR, not inference from Git. |
| SC-REMOTE-SSH-CONNECT's OR-CAPTURE-AUTHORIZED-ONLY says no connections during all refusals, while its rejected-credential case requires server authentication evidence. | Preflight policy denial must make no connection. Host-key/authentication refusal may require a bounded exchange with the already-authorized endpoint, but no bootstrap/command/workspace effect. Root must clarify that oracle before qualification; do not claim its literal zero-connection assertion passed. |
| Some index/readiness prose still cites ADR-0046's freeze clauses; [ADR-0046](../adrs/ADR-0046-surface-expansion-freeze.md) is retired in full. | Do not resurrect the freeze or infer activation from retirement. Current evidence-based surface gates remain; root owns the stale prose correction. |
| ADR-0024/proposal-only writes conflict with an unrestricted terminal or source-writable host bind mount. | Select target-side disposable execution lanes and container-owned volumes. Canonical source mutations require proposal commits. Any future unrestricted interactive-write profile needs an explicit authority decision and independent tests. |

## Planning acceptance and remaining external facts

No additional owner preference is needed to finish this bounded design: target
families, initial topology, authentication method, authority split, mode behavior,
reconnect rules and activation gates are selected above under the existing
engineering authorization. Root independently reviewed the complete contract
and accepted its narrow planning scope; Pauli records ticket169's resolution in
the documentation integration gated by the single final docs-hygiene check.
Unknown deployment facts do not make the design decision vague.

Actual VM/address/host key/user mapping, scoped credentials, rootless engine and
volume identities, OS/tool patch inventory, image/agent digests and signer/trust
material, frozen project fixtures and cleanup observer remain **missing external
prerequisites**. Target operator supplies them to 170/171/173; language owners
provide target artifacts to 177/178/190–210; extension owner supplies 181/189's
runtime contract/artifacts; 211's qualification owner captures native and external
evidence. A manifest with any missing security identity or limit cannot activate.
Root still must resolve the named canonical drift during integration/activation;
this is a recorded handoff, not permission to edit those records in this lane.

Integration verification: the single final composed `cargo run -p xtask -j 2
--target-dir D:/legion-ide-2026-tools/qualification-target -- docs-hygiene` passed
with exit 0. Log: `D:/legion-ide-2026-notes/wave1-final-docs-hygiene-ddba84fe.log`.
Root's planning-only review and this documentation gate support ticket169's narrow
resolution; no remote runtime or product acceptance follows.
