# Legion IDE 2026 comprehensive program specification

Status: ready-for-agent
Type: specification
Date: 2026-10-08
Scope: approved product design; planning only

The product scope, shared understanding and testing boundaries are confirmed.
This specification is published to the local tracker as `ready-for-agent`.
That label enables implementation planning, not provisioning, product acceptance,
or shipping.

## Problem Statement

Developers need to complete real editing, navigation, build, test, debug and Git
work reliably before they can adopt a new IDE. AI assistance adds value only when
its context, changes, cost and execution are understandable and controllable.
Today Legion has substantial native and policy infrastructure, but component
tests and deterministic demonstrations do not establish complete daily workflows,
useful live assistance, interoperable agents, or safe recovery after interruption.

The existing full-product and frontier plans overlap with newer research. Without
one dependency-aware specification, the team risks rebuilding existing services,
prioritizing advanced experiments over adoption blockers, losing earlier product
obligations, or presenting stale evidence as qualification. Developers also need
truthful compatibility and unavailable states instead of inferred parity.

## Solution

Deliver the existing full product through independently qualified milestones.
First qualify a Windows/Rust Manual daily-driver pilot using Legion development
itself. Separately qualify assisted workflows with named external agents, useful
real-provider next-edit assistance, exact proposal review, observable cancellation
and explicitly resumed recovery. Continue all required language, platform,
extension, remote, enterprise and distribution work. Complete bounded frontier
experiments without making their success a prerequisite for ordinary editing.

The primary assisted journey is: inspect a proposed change, understand its affected
code and uncertainty, run authorized existing checks against the proposed editor
state, see fresh or stale evidence, then review and apply or reject through normal
workspace authority. Formal tools and learned retrieval extend this journey later.

Required delivery means a demonstrated workflow within a named compatibility
contract. Experimental work must produce a reviewed result, including a negative
result when appropriate; it cannot claim supported capability without independent
qualification. Partial delivery never means whole-program completion.

### Scope and candidate disposition

All 22 existing full-product feature families remain required. The following
dispositions add sequencing and research decisions, rather than deleting those
families or replacing their atomic requirements.

| Candidate | Disposition | Delivery rationale and boundary |
| --- | --- | --- |
| Native editing, navigation, terminal, tests, debugger, Git, recovery and accessibility | Required; first milestone | Adoption depends on real ordinary workflows without AI credentials. |
| Exact proposal review, impact, executable evidence and rollback | Required; first complete change-review journey | Make changes understandable and preserve developer authority. |
| ACP agent choice, retaining existing MCP roles | Required | Qualify Codex and Claude Code separately; speaking ACP alone is insufficient. |
| Private/local/self-hosted setup, permissions, cost and cancellation | Required | Preserve existing provider/model, trust and enterprise scope. Locality is not an egress guarantee. |
| Resolved repository context and incremental indexing | Required | Use LSP/Tree-sitter and existing index/scheduler; disclose incomplete impact. |
| Useful next-edit predictions with an existing provider | Required | Demonstrate benefit against ordinary completion; custom training is not a dependency. |
| Supervised agent sessions and restart recovery | Required | Restore permitted context/progress with execution stopped. |
| Reproducible remote/container environments and reconnect | Required after local pilot | Extend existing remote scope; environment formats do not establish containment. |
| Source-linked failures and browser trace inspection | Required in relevant language/web milestone | Show actual test/process evidence; browser traces do not qualify native Legion input. |
| Portable instructions/skills with lightweight planning | Required | Visible provenance and explicit execution trust; small changes need no elaborate document ceremony. |
| Selective settings/keybinding migration and named extensions | Required | Report compatibility precisely; native equivalents can satisfy pilot workflows only. |
| Shared review, CI/security/deployment feedback and enterprise controls | Required through existing team scope | Integrate existing Git/CI review and evidence; no replacement collaboration backend is implied. |
| Structured tool generation and serving-side XGrammar evaluation | Required profile/cancellation work; bounded backend evaluation | Syntax validity never supplies tool or write authority. |
| Learned retrieval, formal verification/repair, verified refactoring | Complete F4/F5/F7 experiment and implementation packages | Supported opt-in delivery requires independent positive qualification; negative conclusions remain explicit. |
| Custom next-edit model training | Bounded experiment | Compare against the qualified existing-provider baseline before investing in training. |
| Interactive agent stepping and trajectory editing | Bounded experiment | Assess diagnosis value and replay safety before promoting support. |
| AHP/shared multi-client session hosts | Deferred | Study when multi-client hosting is required; restart recovery does not need a new shared host. |
| Agent execution after IDE exit | Deferred | Distinct service ownership and operational contract. |
| Replacing Git/PR collaboration or autonomous deployment | Deferred | Outside the accepted authority and initial adoption goals. |
| SCIP producer, Salsa migration, vector/search database replacement | Deferred implementation alternatives | Native contracts meet the current need; require a measured crossover and ADR before replacement. |

## User Stories

1. As a developer, I want to open an existing Rust workspace in Manual mode, so that I can work without configuring AI.
2. As a developer, I want predictable Unicode, selection, clipboard, IME, Vim and undo/redo behavior, so that normal editing preserves my intent.
3. As a developer, I want tabs, splits, windows and layout recovery, so that navigation fits sustained work.
4. As a developer, I want responsive large-file editing with bounded memory, so that background intelligence does not interrupt input.
5. As a developer, I want file and symbol search with cancellation and truthful freshness, so that I can navigate a changing repository.
6. As a developer, I want reviewable workspace replacement and language edits, so that broad changes cannot bypass conflict protection.
7. As a developer, I want live completion, diagnostics, hover, references and call hierarchy, so that language assistance reflects my actual toolchain.
8. As a developer, I want real build and test execution with meaningful output and exit results, so that I can diagnose failures rather than inspect simulated success.
9. As a developer, I want debugger launch/attach, breakpoints, stepping, variables and termination, so that I can investigate actual program behavior.
10. As a developer, I want a usable terminal with control keys, resizing and process cleanup, so that interactive tools work normally.
11. As a developer, I want Git status, diffs, staging, commits, branches, conflicts and history, so that source-control work stays in my development flow.
12. As a developer, I want save conflicts and external edits to preserve dirty text, so that concurrent changes cannot silently destroy my work.
13. As a developer, I want interrupted sessions and failed storage migrations to preserve originals, so that recovery does not compound a failure.
14. As a developer, I want keyboard and assistive-technology access to the same workflows, so that visible controls are usable without a pointer.
15. As a migrating developer, I want to preview selected settings and keybinding imports, so that I can adopt familiar behavior without replacing unrelated preferences.
16. As a migrating developer, I want a report of supported, unsupported and conflicting imports, so that omissions are visible and reversible.
17. As a migrating developer, I want each essential extension workflow labeled as native, compatible or unsupported, so that I can judge whether Legion covers my work.
18. As a developer, I want extension install/update/disable/remove and recovery within declared APIs, so that extension support means more than manifest classification.
19. As a developer, I want to choose a named agent and see its qualified capabilities, so that I understand the integration I am activating.
20. As a developer, I want external agent tools confined to an isolated working copy, so that their output cannot directly alter my authoritative workspace.
21. As a developer, I want context, permissions, endpoint and execution location visible before activation, so that consent covers the actual work.
22. As a developer, I want explicit scope, cost and resource limits, so that one activation cannot authorize unbounded retries.
23. As a developer, I want to stop an agent and distinguish stopping from confirmed termination, so that cancellation is observable and honest.
24. As a developer, I want late or superseded output rejected, so that it cannot create actions after cancellation or context changes.
25. As a developer, I want retained agent progress restored after restart with execution stopped, so that I can inspect it before resuming.
26. As a developer, I want uncertain external action outcomes reconciled before retry, so that recovery does not duplicate a push, migration or publication.
27. As a developer, I want to select a real prediction provider explicitly, so that the IDE never silently changes where my code goes.
28. As a developer, I want timely, current next-edit suggestions that I can accept, reject or undo, so that suggestions help rather than disrupt editing.
29. As a developer, I want provider outages to leave ordinary completion and editing usable, so that an AI dependency cannot disable Manual workflows.
30. As a reviewer, I want exact diffs and affected files/tests with navigable provenance, so that I can understand a change before approving it.
31. As a reviewer, I want heuristic and resolved relationships distinguished, so that matching names do not appear to prove dependency.
32. As a reviewer, I want unknown impact to widen checking or report incomplete coverage, so that missing knowledge does not look like no risk.
33. As a reviewer, I want relevant dirty buffers included in the verification subject, so that evidence describes the state I actually intend to use.
34. As a reviewer, I want an explanation when that combined state cannot be represented, so that disk-only checks cannot silently substitute for it.
35. As a reviewer, I want required and optional checks with their authority shown separately, so that advice cannot silently become a gate or waive one.
36. As a reviewer, I want freshness, coverage, outcome and artifact availability shown independently, so that successful but stale results are not misleading.
37. As a reviewer, I want approval invalidated by meaningful proposal or requirement changes, so that approval never transfers to an unreviewed change.
38. As a developer, I want apply to preserve unrelated dirty buffers and recheck mutation guards, so that a valid check does not become unconditional write permission.
39. As a developer, I want test/build execution in an authorized isolated staged state, so that repository code cannot escape through verification.
40. As a developer, I want failed tests linked to source and relevant browser traces, so that diagnosis uses reproducible evidence.
41. As a developer, I want loaded instructions and skills to show source and version, so that I can inspect which guidance affects an agent.
42. As a developer, I want executable skill permissions reviewed when introduced or expanded, so that discovering or updating instructions does not grant execution authority.
43. As a developer, I want structured tool output validated for both syntax and permission, so that well-formed output cannot bypass policy.
44. As a developer, I want optional learned retrieval to retain citations, privacy and context budgets, so that improved ranking does not conceal provenance or egress.
45. As a developer, I want declared formal properties, assumptions and tool bounds displayed, so that a proof is limited to what was actually checked.
46. As a developer, I want counterexamples validated before being described as bugs, so that tool failure and unproved obligations are not misclassified.
47. As a developer, I want bounded proof repair to preserve properties and return an ordinary proposal, so that repair cannot weaken the specification or apply itself.
48. As a developer, I want verified refactoring restricted to its proven semantic domain, so that unsupported code cannot receive a universal safety claim.
49. As a developer, I want unsupported formal platforms to explain unavailability without launching a fallback, so that changing execution environments remains explicit.
50. As a developer, I want Rust, TypeScript, JavaScript and Python workflows qualified on declared systems, so that one successful Rust demo does not imply universal coverage.
51. As a remote developer, I want environment identity, reconnect, offline state and remote tool authority visible, so that connection failures cannot target the wrong workspace.
52. As a teammate, I want shared proposals, review and reconciliation tied to current evidence, so that collaboration preserves the same authority rules.
53. As an administrator, I want provider/extension policy, identity, audit export and retention controls, so that team adoption can meet existing governance requirements.
54. As a developer, I want raw traces and source retention to require separate consent, so that enabling inference does not silently enable storage or training.
55. As an evaluator, I want manual and assisted milestones scored separately, so that missing credentials do not prevent evaluating ordinary editing.
56. As an evaluator, I want real workflow attempts and fallback to other IDEs logged for five working days, so that pilot acceptance reflects sustained use.
57. As an evaluator, I want performance and usefulness targets frozen before candidate scoring, so that a weak candidate cannot redefine success.
58. As an evaluator, I want failed experiments and blocked prerequisites reported independently, so that unsuccessful work is not relabeled as support.
59. As an operator, I want signed installation, update/restart/rollback and no-egress artifact evidence separately qualified, so that local tests do not imply release readiness.
60. As a contributor, I want dependency-linked, demoable implementation slices with explicit checks, so that new work builds on accepted contracts and existing implementation.

## Implementation Decisions

### Authority and integration

Retain the native Rust workspace and its existing service ownership. Shared
contracts belong to protocol; UI renders app-produced projections and emits typed
intents; app composition coordinates; editor owns buffers; workspace owns guarded
filesystem mutation. Save and apply retain fingerprint, snapshot, buffer-version
and workspace-generation checks and fail closed on non-atomic or conflicting
writes. Do not introduce an editor-to-project dependency or UI/provider writes.

Use existing supervised services, including app-owned thread workers; do not
assume every service uses Tokio. Background graph, verification, inference and
retrieval work must not occupy paint/input execution. Preserve bounded text chunk
APIs and the 5 MiB text snapshot cache. Observability identifiers remain nonzero
and valid; source bodies cannot be smuggled into metadata fields.

New or materially expanded surfaces require a bounded ADR, explicit contracts,
dependency-policy enforcement, a phase gate and contract tests before activation.
The old blanket ADR-0046 freeze is retired, but PR-VSC-002, PR-ENT-001 and
PR-ENT-002 remain unqualified/deferred until their current gates are met. Product
scope approval does not itself activate those runtime surfaces. Extend ADR-0043's
local ACP bridge through a reviewed protocol/containment contract rather than
treating the existing command adapter as already interoperable.

### Current-state reconciliation and schema ownership

Reuse the existing completion requirements, configuration matrix, scenarios,
dependency register, defects and decisions, together with the completion/native
acceptance tooling. F0's claim that these are all absent is superseded. The matrix
exists but inspected rows still contain provisional approval and pending tool
versions. Candidate nomination is a separate operation; no candidate artifact or
accepted result is fabricated by this specification.

Preserve canonical requirement IDs and attach new atomic outcomes only where
existing rows do not already own them. Register frontier additions using the
approved COMP-FRONTIER namespace and connect existing S0/S1/S2/S3/extension/remote/
enterprise owners. This document owns design, not acceptance status. No parallel
readiness database or duplicate completion system is introduced.

Existing prediction projections and deterministic workflows, ACP command-host
proposals, workflow reconstruction from supplied data, and extension manifest
classification are reusable substrate. They do not prove live prediction quality,
ACP protocol compatibility, durable agent recovery or executable extension support.
Record that distinction per requirement before estimating remaining work.

### Migration and ordinary workflows

The first acceptance configuration is Windows/Rust on Legion's own multi-crate
repository. Ratify exact OS/toolchain/server/debugger/hardware and artifact identity
before qualification. Preserve broader required Rust binary/library/workspace,
TS and JS browser/Node, and Python application/package configurations across the
required operating systems. JavaScript requires its own evidence, not a TypeScript
result renamed as coverage.

Migration imports are previewed, selective and reversible. Map supported settings
and keybinding semantics explicitly, preserve unrelated configuration, and report
unsupported keys, collisions, precedence and malformed inputs. Inventory the
pilot's essential workflows and named extension IDs/versions before freezing its
acceptance matrix. A qualified native equivalent can satisfy a pilot workflow but
cannot be recorded as extension compatibility. Required WASM, VS Code runtime/API,
webview, notebook, custom-editor and storage outcomes remain in the full program.

### Proposal intelligence and evidence

Extend the existing semantic index with resolved identities, provenance, dependency
versions, forward/reverse relationships and scoped impact coverage. Lexical name
matches remain heuristic. Late server/parser results cannot replace newer facts.
Unknown dependencies conservatively widen the declared subject; budget exhaustion
produces incomplete coverage instead of silently reducing scope.

Incremental parsing consumes authoritative editor transactions with exact byte and
point lineage. Preserve existing changed-range semantics for other consumers.
Missing lineage, opaque undo/redo or unsupported input triggers a full/degraded
parse. Bound tree and metadata retention independently; avoid duplicate source
storage. The native LSP/Tree-sitter/index/scheduler design remains selected.

A versioned VerificationEnvelope binds proposal and overlay digests, relevant
buffer/snapshot/workspace versions, dependency manifest, properties/assumptions,
tool/configuration identity, coverage, outcome, resource/cancellation metadata and
artifact references. Run lifecycle, evidence freshness, coverage, semantic outcome,
artifact availability and termination state are independent dimensions. TestsPassed
is not Proved; NotProved is not Disproved; a witness is not validated by exit code.

The app constructs immutable verification subjects from proposed changes and
relevant dirty buffers. Unrepresentable combined state blocks checking with an
explanation. Never save implicitly or substitute disk. Tests/build scripts need
workspace trust and execution permission; writes remain in approved staged/scratch
locations under enforced isolation. Optional failures are advisory. Required checks
come only from authorized policy or the developer, and unmet requirements block
only the affected proposal. AI output cannot weaken them.

Approval binds the exact proposal, requirements and relevant state. Meaningful
changes invalidate it; proven-independent changes may be ignored, but unknown
independence invalidates conservatively. Apply rechecks workspace authority and
preserves unrelated dirty buffers. Evidence labels the combined editor state and
does not certify a differing resulting disk state.

Persist metadata through existing repositories/events with transactional migrations.
Older records lacking evidence or resolution become unknown/unavailable, never
historical green proof. Retain originals on migration failure. Raw source, output,
witnesses and traces require the existing separate consent, redaction, vault,
deletion and retention controls. Missing artifacts after restart are shown honestly.

### Agents, predictions and session recovery

ACP negotiation and session updates map into app-owned permissions, context,
cancel, proposal and audit contracts. Independently qualify Codex and Claude Code
with named adapter/agent/protocol versions, supported methods, authentication route
and enforcement environment. A protocol-compatible agent without enforceable
containment is unsupported. Its own filesystem and terminal tools run in an
isolated working copy; only reviewed proposals reach the main workspace. Explicitly
allowed networking and credentials are bounded by the selected profile; copied
workspace placement alone is not proof of isolation.

Prediction requests bind selected endpoint/model/profile revision, current buffer
and context versions. Reject stale outputs and preserve normal accept/reject/undo.
No silent switch among hosted, local and deterministic providers. Ordinary language
completion remains usable when predictions are unavailable. Select and pin an
existing real provider during the qualification preparation slice using supported
capabilities and credential availability, then freeze latency/usefulness targets
before evaluating the candidate. Do not assert an untested model is the best one.

Durable agent-session records contain schema version, task/workspace identity,
profile/capability identity, permitted retained context references, progress,
proposal/evidence references, action IDs and known/uncertain outcomes. Persist
checkpoints atomically through existing storage services, not a second database.
On restart, validate versions, permissions and artifact availability; interrupted
execution is stopped until explicit resume. Reconcile uncertain external outcomes
against resulting state before any retry. If reconciliation is unavailable, require
inspection rather than blind replay. Revoked consent or unavailable source cannot
be bypassed to reconstruct context. Continuing execution after IDE exit is excluded.

Every activation binds scope, permissions and aggregate attempts/time/resource
limits. Extensions require a new activation. Cancellation immediately disqualifies
subsequent output, while UI shows stopping until actual transport/process termination
is confirmed. Failed termination remains an execution fault, with unsafe reuse or
cleanup blocked. Provider cancellation includes connect and streaming-body reads,
not merely suppression of a returned result.

### Skills, artifacts and advanced capabilities

Portable instruction/skill discovery records source, version/content identity and
loaded scope. Discovery does not authorize scripts, tools or network access. New
or expanded execution grants require explicit approval; updates cannot inherit
expanded grants. Source-bearing traces are consented artifacts with provenance and
retention controls. Link real test failures and relevant browser traces to exact
source/evidence; untrusted artifacts cannot issue authoritative IDE commands.

Retain F4's approved formal worker isolation, supported-domain and provisioning
contract, F5's pure-u32 wrapping refactoring domain and independent checker, F6's
explicit NativeTools/JsonSchema profiles and transport cancellation, and F7's
bounded learned retrieval contracts. Their detailed domain exclusions, wire limits,
resource bounds, cache identities and qualification conditions remain normative
through the frontier input. Do not silently weaken them during ticket decomposition.

In particular: formal execution targets Linux x64 and explicitly provisioned
Windows WSL2; native macOS formal execution is Unsupported. Formal jobs are limited
to one active job, 120 seconds, 2 GiB aggregate memory, 64 processes, 1 GiB scratch
and bounded output, with confirmed process-tree cleanup. Proof repair is bounded
to three candidates/180 seconds and cannot weaken properties, add trusted bypasses
or apply itself. Verified rewriting admits only the approved pure primitive domain,
requires independent equivalence checking, and returns an ordinary proposal.

Structured profiles bind endpoint/model/schema identity and retain all semantic,
permission and proposal checks. XGrammar remains serving-side; no IDE decoder is
introduced. Provider transport limits and actual cancellation targets remain those
of F6. Learned retrieval remains explicit Assist/Delegate opt-in with distinct model
spaces, source-correct citations, bounded batches/cache and no cloud fallback.
Manual forbids AI inference even through loopback; lexical/structural retrieval is
still available. Deterministic retrieval degradation must be explicitly labeled and
must not be confused with forbidden silent prediction-provider fallback.

### Milestones, dependency graph and development approach

| Milestone | Outcome and work | Prerequisites | Exit evidence |
| --- | --- | --- | --- |
| M0 — Reconcile and freeze contracts | Deduplicate canonical requirements; ratify pilot matrix, migration inventory and candidate workloads; assign architecture/authority amendments and prerequisite owners | Approved specification and current source inspection | No unowned required outcome; pending configuration fields explicit; baseline and target-freeze protocol recorded |
| M1 — Manual daily-driver | Close ordinary Windows/Rust edit/navigation/build/test/debug/terminal/Git/preservation/accessibility blockers and selective migration | M0 pilot contracts; real language/debug tools | Native and external-effect evidence plus five working days of declared real development; no unresolved data-loss, unauthorized-write or workflow-blocking defects |
| M2 — Complete change review | F1/F2/F3 slice: impact, staged existing tests, dirty-state evidence, freshness, requirements, inspect/recheck/cancel/review/apply | M0 contracts; usable M1 workflow paths; enforceable staged test execution | End-to-end native journey and adverse-state evidence without Kani, Verus or learned-model prerequisites |
| M3 — Assisted workflows | Qualified ACP agents, real selected prediction provider, bounded execution, profile/cost/privacy UX, durable stopped recovery, portable skills | M2 review authority; containment/storage/provider contracts and external credentials | Independent named-agent evidence, useful real predictions, cancellation/termination and restart/reconciliation; no fixture fallback credited |
| M4 — Required compatibility breadth | Complete Rust/TS/JS/Python and three-OS matrices, native migration coverage, source-linked web/test traces and required extension lifecycle/runtime/API work | Relevant M0 contracts and M1 foundation; extension ADR/policy/isolation gates | Per-configuration native journeys and external artifacts; declared unsupported items; actual named extension workflows |
| M5 — Remote and enterprise | Existing remote/container, shared review, collaboration, identity/admin/audit/retention and Git/CI feedback obligations | Accepted local workflow contracts; remote/enterprise authority gates; provisioned target environments | Reconnect/offline/conflict/role-denial and review evidence using real services; no mock connection credited |
| M6 — Frontier experiments and qualification | Complete F4/F5/F6/F7 implementations/evaluations, custom prediction-training and agent-debugging experiments | M2; each tool/isolation/model contract; held-out workloads and frozen targets | Independent positive qualification or reviewed negative conclusion per experiment; unavailable work remains blocked, not completed |
| M7 — Full-program and release acceptance | Close remaining required families and distribution/operations promises, supported configurations and independent reviews | All required milestones and experimental work outcomes; real external release prerequisites | Canonical evidence for required outcomes; signed clean-install/update/restart/rollback and platform/privacy evidence where required |

Dependencies are capability-specific, not a forced serial queue. M2 engineering can
overlap M1 once its inputs are stable; M4 preparation can overlap M3; M6 research can
run after its prerequisites without blocking ordinary adoption. Manual acceptance
does not wait for provider credentials. Assisted acceptance remains separately
required. Ship qualified capabilities independently under applicable authorization;
do not call M7 complete while required work is blocked.

Decompose each milestone into demoable vertical slices with explicit blockers,
owned modules, input/output contracts, existing requirement IDs, acceptance scenario,
verification commands and external prerequisites. Inspect before assigning work.
Use one coordinator for shared protocol/app/UI contract changes and normally one
or two independent workers for disjoint implementations. Persistence, containment
and authority changes receive independent review. Estimate staffing and schedule
only after this dependency/prerequisite inventory; five pilot days is observation,
not a delivery estimate. The separate ticket-writing step produces executable work
items; this specification does not invent dates or claim those items are complete.

## Testing Decisions

### Confirmed behavioral boundaries

Use AppComposition workflow integration as the primary automated behavioral seam:
dispatch real app intents, observe projections/results and inspect external effects.
Exercise the existing editor/workspace services, substituting only external providers
or tools where deterministic contract testing is intended. Prefer extending current
workflow tests over introducing a separate orchestration harness.

Use native packaged desktop acceptance for keyboard/pointer/focus/assistive-technology
paths and visible controls; app integration tests cannot prove native input or paint.
Use focused process and storage contracts where only a real subprocess or reopened
store can demonstrate containment, cancellation, resource limits, atomic recovery
and migration failure. Parser/index differential and property tests are justified
for incremental equivalence and conservative impact, rather than UI-shaped tests
of internal implementation. These are complementary boundaries, not competing
whole-product test systems.

Tests assert observable outcomes: preserved dirty text, exact applied bytes, rejected
stale outputs, actual process termination, unavailable states and absence of forbidden
egress/retention. Avoid tests that simply rebuild expected implementation structures.
Deterministic provider replay proves contracts; real-agent/provider acceptance is
separately opt-in and never replaced by replay or paid mandatory PR calls.

### Existing test precedents

- [Workspace VFS integration](../../crates/legion-app/tests/workspace_vfs_integration.rs) supplies conflict, dirty-text preservation, trust and path-escape cases at the app/workspace boundary.
- [Git workflows](../../crates/legion-app/tests/git_workflow.rs), [delegated tasks](../../crates/legion-app/tests/delegated_task_integration.rs) and [terminal workflows](../../crates/legion-app/tests/terminal_workflow.rs) supply app-level external-effect, cancellation and cleanup patterns.
- [Desktop session restoration](../../crates/legion-desktop/tests/session_restore.rs) is a recovery precedent; extend its observable style without confusing editor restoration with durable agent-session recovery.
- [Sandbox escape attempts](../../crates/legion-sandbox/tests/escape_attempts.rs) supplies real-process hostile probes. Record skipped platform prerequisites explicitly; a skipped symlink/junction probe is not passed containment evidence.
- [Language restart policy](../../crates/legion-app/tests/language_restart_policy.rs) and [DAP handshake](../../crates/legion-debug/tests/live_dap_handshake.rs) supply controlled-adapter contracts, not real language/debugger qualification despite test names.
- [Completion evidence](../../xtask/tests/completion_evidence.rs) and [artifact containment](../../xtask/tests/completion_artifact_files.rs) protect qualification records and references; reuse their schema and path-integrity checks.
- [Windowed smoke](../../crates/legion-desktop/src/windowed_e2e.rs) opens a native window but directly dispatches actions. Keep that smoke layer distinct from real user-input acceptance.

The inspected native acceptance infrastructure has harness evidence, not a newly
observed product run. The [recorded host observations](../../plans/evidence/completion/native-manual-open-type-save-r05-win11-x64/host-observations.md)
identify missing driver/package evidence for that recorded attempt. Provision and
observe the selected current candidate before claiming native-input acceptance.

### Required acceptance scenarios

| Scenario | Observable acceptance | Primary boundary |
| --- | --- | --- |
| A01 Manual pilot | Five working days record build identity/configuration, every declared workflow, successes/failures/fallback and evidence; zero unresolved data-loss, unauthorized-write or workflow-blocking defects | Native desktop plus real work journal |
| A02 Preservation | External overwrite, dirty dependency and migration/storage failure preserve user text/originals and produce conflict or recovery | App workflow plus storage reopen |
| A03 Review journey | Inspect cross-file proposal, navigate impact, run actual staged tests, observe evidence, apply/reject; no advanced tool prerequisite | App integration plus native controls and disk oracle |
| A04 Subject integrity | Relevant dirty buffers included; unrepresentable state blocks; unknown impact expands scope or reports incomplete coverage; disk is not substituted | App integration plus staged process |
| A05 Approval integrity | Required tool missing/timeout/partial blocks only applicable proposal; optional failure remains advisory; weakening requirements or relevant edits invalidates approval | App integration |
| A06 Isolation | Hostile file traversal, child processes and denied network cannot escape declared enforcement; main workspace unchanged until reviewed apply | Real process contract plus app apply |
| A07 ACP qualification | Each named agent negotiates supported capabilities and passes context/permission/deny/proposal/cancel/unsupported tests with actual integration | Adapter contract plus live native qualification |
| A08 Cancellation | Late output cannot create proposals/tools/vectors; transport and process-tree death are observed; failed death is a visible fault | App integration plus real endpoint/process |
| A09 Recovery | Kill/restart restores permitted progress stopped; stale identity/consent handled; uncertain consequential outcome cannot replay blindly | Reopened storage/process plus native resume |
| A10 Prediction usefulness | Selected real profile, frozen baseline/targets, retained edits and undo/rejection/latency measured; stale/outage/Manual paths truthful; ordinary completion baseline retained | App integration plus live qualification |
| A11 Migration | Preview/select/import/revert supported settings; conflicts and unsupported entries listed; named extension/native outcomes distinguished | App integration plus native workflow |
| A12 Skill trust | Open/discover/update cannot grant expanded script/tool/network access; loaded source/version shown; injected instructions cannot override authority | App policy/workflow plus native inspection |
| A13 Graph and incremental text | Aliases/name collisions resolved correctly; late epochs rejected; cold/incremental parse agree for Unicode, newline, multiedit, undo and dropped events | Index/parser contracts plus app projection |
| A14 Formal correctness | Exact properties/configuration; real proof and validated false witness; repair cannot weaken properties; forged rewrite rejected; unsupported Mac launches nothing | Worker contract, actual tool qualification and native inspection |
| A15 Retrieval/structured output | Held-out ranking and citations; malformed vectors/schema/tool calls, profile drift and denied egress rejected; no unauthorized persistence | App/provider/index contracts plus qualified endpoint |
| A16 Language/platform breadth | Actual multi-file LSP edits, passing/failing tests and debugger breakpoints/steps per required project/OS; unavailable/restart observed | Native desktop plus real server/debuggee |
| A17 Trace evidence | Failing test navigates exact source and consented browser artifact; stale/missing artifact disclosed; browser evidence not credited as native UI proof | App/artifact contract plus web/native acceptance |
| A18 Extensions/remote/team | Named real extensions, remote reconnect and shared proposal/role/conflict behavior meet their separate runtime contracts | Native integration plus real external services |
| A19 Privacy and release | Manual no-egress including loopback inference; metadata-only storage/support bundle; explicit trace deletion; real installer/update/rollback and signing evidence | OS/network/storage probes and release qualification |
| A20 Experimental honesty | Disabled by default; unchanged protection gates; held-out baseline/targets frozen; independent negative or positive conclusion; blocked resources cannot count as negative completion | Canonical evaluation record plus review |

Use the existing completion EvidenceRun schema for candidate qualification; runtime
VerificationEnvelope artifacts remain separate and may be attached by digest.
Record candidate revision/artifact, configuration, scenario, actual tools, external
oracles, result, defects and reviewer. Screenshots support visible desktop changes
but do not alone prove behavior. Guard-only unsupported-platform results cannot
count toward positive capability coverage.

For performance follow the frontier method: five warmups, thirty samples, named
hardware, nearest-rank p95 and no outlier deletion, retaining stricter existing
budgets. Freeze usefulness/retrieval targets against representative baselines before
candidate scoring. Measure time to a correct reviewed change, rework, retained
edits, rejection/undo, cancellation latency and cost, not generated text or activity.
No numerical improvement is asserted by this specification.

Plan relevant checks per slice and run each once; rerun affected failures only
after supported fixes. Two distinct failed repair attempts require returning
evidence and escalating. Broader repository/OS gates and external qualification
remain distinct from targeted local verification.

## Out of Scope

- Product implementation, ticket publication, commits, provisioning and release execution during this specification-writing task.
- Universal compatibility with arbitrary tools, extensions, languages or platforms outside published contracts.
- Silent provider/environment fallback, weaker mutation guards, AI-approved requirement removal, or autonomous main-workspace application.
- Agent execution after IDE exit, AHP multi-client hosting, replacing Git/PR collaboration, or autonomous deployment.
- General-program proofs, arbitrary-language verified rewriting, native macOS formal execution, or guarantees inferred from schema-valid output.
- A new graph/vector database, embedded decoder, or mandatory custom-model training as a shortcut to the accepted workflows.
- Production/readiness promotion from document completion, deterministic replay, historical evidence or narrow green tests.

These exclusions do not cancel the pre-existing required feature families or the
approved F0–F8 work packages.

## Further Notes

### Inputs and precedence

- [Confirmed interview decisions](decisions.md) — Q19–Q36 and scope confirmation; Q1–Q18 incorporated from the frontier plan.
- [Domain glossary](../../GLOSSARY.md) — vocabulary used throughout this specification.
- [2026 research](../../plans/evidence/2026-ide-demand-and-technology-research.md) — evidence, limitations and original external sources; research is not qualification.
- [Frontier plan](../../.omp/plans/FRONTIER_INTEGRATION_PLAN.md) — normative F1–F8 technical constraints and Q1–Q18; its obsolete F0 absence claims are corrected above.
- [Full-product design](../../docs/superpowers/specs/2026-09-04-product-completion-design.md) — retained feature families and finite supported-configuration contracts.
- [Full-product implementation plan](../../docs/superpowers/plans/2026-09-04-full-product-completion.md) — existing stage owners and work packages to reuse.
- [Manual/language plan](../../docs/superpowers/plans/2026-09-04-manual-language-completion.md) and [AI/team plan](../../docs/superpowers/plans/2026-09-04-ai-team-completion.md) — detailed existing obligations.
- [Readiness ledger](../../plans/product-readiness-ledger.md), [completion requirements](../../plans/completion/requirements.json), [matrix](../../plans/completion/matrix.json), [scenarios](../../plans/completion/scenarios.json), [dependencies](../../plans/completion/dependencies.json), [defects](../../plans/completion/defects.json) and [decisions](../../plans/completion/decisions.md) — canonical status/evidence records; not modified or promoted here.
- [Authority boundaries](../../docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md), [dependency policy](../../plans/dependency-policy.md), [ACP bridge ADR](../../plans/adrs/ADR-0043-acp-host-local-adapter-bridge.md) — implementation constraints.
- [Current-state inspection](current-state-notes.md) — bounded source findings; not runtime acceptance.

Explicit owner decisions govern scope. Current code and observed evidence govern
implementation facts. Current authority/dependency policy remains binding until
properly amended. In conflicts, record both sources and the resolution instead of
silently selecting a convenient older claim. This spec supersedes older sequencing
where necessary to separate Manual and assisted acceptance; it preserves technical
limits and required outcomes unless an explicit decision changes them.

### Prerequisites and remaining technical selection

M0 owns exact version/hardware ratification, essential extension/import inventory,
real prediction provider selection, agent adapter/protocol versions, held-out
workload freezing and registration of new atomic outcomes. These are bounded
deliverables with explicit acceptance, not permission to omit hard capabilities.
Inspect current APIs and verify mutable third-party versions when selecting them;
the research snapshot is not a package lockfile. Obtain a scope decision only when
feasibility would change a confirmed obligation or authority boundary.

External prerequisites include named native/AT hosts, real debug/language tools,
provider accounts or provisioned local endpoints, supported isolation facilities,
formal tool bundles/offline dependencies, remote/team test services, and approved
release signing/update infrastructure. Missing prerequisites block dependent
qualification only. Signing descriptors remain dry-run/no-production-signer until
real signing support is approved; no credentials or private keys enter the tree.

### Interview traceability

| Decisions | Specification ownership |
| --- | --- |
| Q1, Q9, Q14 | Candidate disposition, M6/M7, experimental honesty A20 |
| Q2, Q5 | Solution and M2 first complete change-review journey, A03 |
| Q3, Q7, Q13, Q16 | Required/optional authority and approval integrity, A05 |
| Q4, Q8 | Bounded activation and explicit advanced work, A08/A15 |
| Q6, Q10, Q11, Q12, Q15 | Proposal subject, isolation and stale approval, A02–A06 |
| Q17 | Cancellation/termination states, A08 |
| Q18 | Frozen baselines and evaluation method, A10/A15/A20 |
| Q19, Q20, Q21, Q22 | Unified scope, milestone graph, preserved external gates |
| Q23, Q27, Q30 | Pilot matrix, migration/native-equivalent boundaries, M1/M4/A11 |
| Q24, Q28, Q34 | ACP integration targets and containment, A06/A07 |
| Q25, Q29 | Explicit real-provider predictions and no silent fallback, A10 |
| Q26, Q31 | Stopped durable recovery and external-action reconciliation, A09 |
| Q32, Q35 | Artifacts/skills/remote/team, experiments and deferrals, A12/A17/A18 |
| Q33, Q36 | Five-day Manual observation and separate assisted acceptance, M1/M3/A01 |

## Comments

- 2026-10-08: User confirmed the shared understanding and authorized `to-spec` synthesis. No product implementation or readiness promotion is recorded.
- 2026-10-08: Proposed primary AppComposition boundary, complementary native desktop acceptance, and focused process/storage contracts submitted for confirmation as required by `to-spec`.
- 2026-10-08: User answered “yes” to those testing boundaries. Specification published locally as `ready-for-agent`; specification synthesis is complete.
