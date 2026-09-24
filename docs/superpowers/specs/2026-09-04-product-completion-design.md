# Legion IDE full product completion design

Status: owner-approved; implementation plan available in `docs/superpowers/plans/2026-09-04-full-product-completion.md`.

Date: 2026-09-04. Source baseline: `1a9264ebced087192f1ef980609181fcf942ad33`.

## 1. Purpose and approved decisions

Deliver the full Legion vision as a usable production IDE. The central risk is that component implementations and passing fixture tests can coexist with inaccessible controls, incomplete workflows, and missing recovery behavior.

The owner approved:

- Full existing Legion vision plus missing everyday IDE essentials. Deferred features remain requirements for final completion.
- Complete editing, navigation, refactoring, debugging, and testing for Rust, TypeScript/JavaScript, and Python. Additional languages use the extension system.
- Workflow-first delivery, with independent work in parallel.
- Separate implementation status from product acceptance.
- Seven delivery stages and three complementary verification layers, defined below.

This is the governing design for the forthcoming completion program, not a claim that the current product satisfies it. It neither promotes existing readiness rows nor treats unsigned previews as final releases. Detailed work packages follow written-spec approval. Lower-level subsystem designs remain separate, bounded specifications where interfaces or authority change.

## 2. Current evidence and its limits

The exploration was source/test/evidence inspection, not a fresh human development session or exhaustive current feature audit. Prior `claim-audit` and `verify-readiness-consistency` runs passed on the source baseline. Those results establish their specified consistency checks, not product completeness.

| Observation | Evidence | Planning consequence |
| --- | --- | --- |
| Real editing/persistence paths exist | `crates/legion-desktop/tests/save_row_2.rs` exercises synthetic clicks, text and save shortcuts, and checks disk contents | Preserve implementation; qualify native input, realistic projects and recovery |
| Real Git integration has stronger tests | `crates/legion-desktop/tests/source_control_reachability.rs` drives rendered controls and checks Git index/history | Credit external effects, then qualify normal native operation and adverse states |
| Native GUI smoke has narrow interaction coverage | `crates/legion-desktop/src/windowed_e2e.rs` opens through launch configuration, invokes edit/save actions directly, and reads disk | Keep smoke, but add native user-input acceptance; do not relabel its direct dispatch as input proof |
| Broad test names sometimes cover only projections | `crates/legion-desktop/tests/user_journey_rendering.rs` constructs snapshots and checks view models | Record the precise layer each test proves |
| Beta acceptance substitutes results | `crates/legion-desktop/tests/beta_acceptance_e2e.rs` constructs a passed verification fixture and enables simulated debugging | Require real test execution and real adapter/debuggee journeys |
| A terminal assertion is weaker than its comment | `clicking_run_cargo_test_sends_the_command_to_the_terminal` in `terminal_reachability.rs` searches for command text | Prove process execution with output that cannot come from input echo, exit status and actual results |
| Live AI paths coexist with deterministic defaults in tests | `ProductAiProviderPreference::from_env` and backend selection in `crates/legion-app/src/lib.rs` | Pin the selected backend and actual execution evidence in acceptance |
| Documentation has drift | User guide describes implicit Rust LSP startup; source tests assert opening a Rust file does not start LSP | Resolve intended behavior and test the ordinary user flow before revising claims |
| Release and broader capability gaps remain explicit | Product-readiness ledger and P0 sequence | Carry signing, replacement/restart updates, remote, extensions and enterprise work into final scope |

Four recorded three-OS native-window runs and owner sign-off are useful historical evidence. They do not replace current-candidate qualification. No assertion here that a feature currently fails is based solely on an old regression-test comment.

## 3. Scope and traceability

Stage 0 must reconcile the master plan, production roadmap, readiness and phase ledgers, all kanban tasks, the installed-product sequence, adaptive IDE/canvas designs, ADRs, retained evidence, and explicit parking-lot requirements. Historical sources are leads for requirements discovery; their old implementation claims are not current truth. Missing historical files must be recorded as unavailable, not assumed reviewed.

The current 163-task kanban is one input. Its `done` values do not close completion requirements. Preserve historical identifiers and evidence while adding a separate program mapping.

For scope, the owner's explicit decisions govern. After written approval, this completion contract governs final acceptance; existing ADRs and authority/dependency rules govern implementation constraints until explicitly amended. Current code and observed results govern implementation facts. Conflicts between older plans must be recorded with both source references, the proposed resolution and the owner's decision when it changes scope. A ledger status or historical milestone cannot overrule a failed observation. The implementation plan must assign one authoritative record per status field and make other documents references or derived summaries.

### Required feature families

| Family | Required completion scope |
| --- | --- |
| Text editing | Selection, Unicode, undo/redo, multi-cursor, indentation, wrapping, clipboard, IME, Vim, large-file behavior and predictable input |
| Workbench | File/folder operations, tabs, splits, multiple windows, layout persistence, settings/profiles, keymaps and discoverable commands |
| Canvas | Promised arrangement, navigation and editing workflows; inventory the direction documents beyond the existing arrangement surface |
| Navigation/search | File/symbol navigation, literal/regex search, workspace replacement, ignores, cancellation and stale-result handling |
| Language tooling | Provisioning/discovery, lifecycle, completion, diagnostics, hover, definition/references, rename, formatting, code actions and refactoring for all three language groups |
| Build/test/debug | Build/task execution, test discovery and targeted/group runs, real adapters, breakpoints, stepping, variables, evaluation, debug console and termination |
| Terminal | Real shell execution, interactive programs, control keys, resize, scrollback, output rendering, working directory and process cleanup |
| Source control | Status, diffs, staging, commits, branches, conflict resolution, remote verbs, planned review integration and local history |
| Work preservation | Dirty recovery, external edits, save conflicts, checkpoints, restart, restore, storage failure and safe migration |
| Provider/model setup | Local and hosted configuration, credentials, managed local runtime, model download/integrity, hardware fit and useful first-run setup |
| Context/retrieval | Index freshness, symbols, memory, provenance, token/context budgets, privacy boundaries and inspectable selection |
| Assist | Completion/ghost text, explanations, inline and multi-file changes, progressive feedback, review/apply/reject and cancellation |
| Delegate | Scoped tasks, isolated execution, real tool loop, verification, reviewable changes, resource limits and restart recovery |
| Multi-agent workflows | Editable plans, dependency execution, worker coordination, conflicts, fleet controls, budgets, interruption and replay |
| External interoperability | Required MCP and ACP roles and actual interoperability with named supported implementations |
| Trust/proposals | Generalized proposal lifecycle, approvals, policy, audit, cancellation, rollback, secret handling and visible egress |
| Extensions | Promised WASM/runtime and VS Code compatibility surfaces, including required webview, notebook, custom-editor and storage capabilities; install/update/disable/remove, permissions, isolation, failure handling and distribution |
| Remote development | SSH/container environments, remote files/tools/terminal/debugging, reconnect, offline transitions and workspace identity |
| Collaboration/enterprise | Shared state/proposals/review, reconciliation, administration, identity, policy distribution, audit/export and retention |
| Training/telemetry | Explicit opt-in capture, redaction, export, deletion, retention and planned feedback mechanisms; Manual privacy remains intact |
| Platform quality | Windows/macOS/Linux, accessibility, focus, DPI, window restore, responsiveness, startup, resource bounds and sustained use |
| Distribution/operations | Signed native packages, separate Manual/offline artifact and OS-level no-egress verification, clean installation, real update replacement/restart/rollback, diagnostics, crash controls, documentation and support |

These families organize discovery; they are not a substitute for enumerating individual requirements. Every retained promise and missing essential gets a stable `COMP-*` identifier. A gap discovered later is added with its dependencies; it cannot disappear because a phase was previously called complete.

### Finite supported-configuration contracts

Final completion means the full agreed product within published compatibility contracts, not universal compatibility with every historical tool, extension or machine.

Before dependent implementation, Stage 0 produces versioned matrices for:

- OS versions, architectures, reference hardware and accessibility tools across Windows/macOS/Linux.
- Rust, TypeScript/JavaScript and Python toolchains, project types, language servers, test runners and debug adapters, including package/environment discovery.
- Extension APIs and contribution types, required VS Code compatibility, runtime categories, named representative extensions and supported versions. An API or runtime promise cannot be replaced by metadata parsing or installation alone.
- Remote hosts/container environments, collaboration topology, identity/admin roles and supported identity integrations.
- Local model/runtime/hardware combinations and hosted provider configurations; required MCP/ACP roles and protocol versions.

Matrix entries must map to product requirements and executable acceptance. Unsupported versions are explicit boundaries; excluding an already promised capability requires an owner scope decision and cannot be used to manufacture full-vision completion. Version selection is an owned Stage 0 deliverable, not an undocumented implementation choice.

The minimum project breadth is Rust binaries, libraries and multi-crate workspaces; TypeScript and JavaScript browser and Node applications; and Python applications and packages in isolated environments. Each group must include multi-file navigation/refactoring, passing and failing tests, real breakpoints/stepping/inspection, dependency setup, tool failure and restart. Stage 0 selects named tool versions and representative repositories for these categories; a single-file fixture cannot replace them.

## 4. Architecture and implementation strategy

Retain projection-only UI, app-owned orchestration, editor/text ownership, workspace write authority, proposal mediation, default-deny capabilities, metadata-first observability and explicit egress consent.

The ordinary product flow remains:

`native input -> desktop/UI intent -> AppComposition -> authoritative service -> external effect -> app snapshot -> rendered feedback`.

Recovery follows the same authority boundaries. Neither tests nor new runtimes may bypass conflict checks, proposal validation, policy or persistence rules to obtain a green result.

Extend existing implementations after tracing and exercising them. Extract a touched region from oversized composition modules when necessary to establish a coherent boundary; do not launch an unrelated rewrite. Interface changes require protocol/dependency-policy updates and the applicable ADR process. Retired freeze language must not silently dictate new scope, but existing security and authority constraints remain binding.

Parallel execution is allowed for independent modules, acceptance assets and platform infrastructure. Integration ownership remains with the primary implementer/reviewer. Workers get explicit file ownership, no overlapping mutations, and bounded tasks. Architectural judgment, security decisions, integration acceptance and final qualification remain primary-owner responsibilities.

## 5. Delivery stages

Stages order acceptance, not every preparatory activity. Distribution, security, accessibility, performance, test infrastructure and three-OS operation start immediately and continue through every stage.

| Stage | Deliverables | Required exit |
| --- | --- | --- |
| 0. Establish baseline | Exhaustive requirements/matrix mapping, UI-to-service traces, evidence classification, test-shortcut findings, blockers and execution dependencies | Every source requirement accounted for; required matrices ratified; no unassigned scope or unexplained `done` promotion |
| 1. Dependable Manual | Editing/workbench/canvas essentials, navigation/search, terminal, Git, work preservation, input/accessibility and workload budgets | Real repository change through ordinary controls; verified save/Git/terminal effects; canvas open/arrange/navigate/edit/save/restore journeys; failure recovery; sustained use |
| 2. Complete language workflows | Setup and complete language/build/test/debug loops for Rust, TS/JS and Python | For each supported project configuration, reproduce a defect, navigate/refactor, debug, fix, test and commit through the product |
| 3. Complete Assist/Delegate | Provider/model setup, context/retrieval, real model operations, multi-file proposals, sandbox task loop, review and recovery | Representative local and hosted tasks complete with real outputs; policy, cancellation and rollback tested |
| 4. Complete multi-agent workflows | Plans, dependencies, coordination, conflicting edits, budgets, approvals, interrupted-work recovery and interoperability | Multiple workers complete a real task; external effects independently checked; controlled failures and recovery demonstrated |
| 5. Complete extensions/teamwork | Full required extension contract, remote environments, collaboration, enterprise governance and opt-in training/telemetry workflows | Supported extensions execute actual capabilities; remote/team/identity workflows work with real endpoints and concurrent participants; consent, capture/export, retention and deletion independently verified |
| 6. Qualify complete product | Whole-inventory regression, sustained use, compatibility coverage, security review, clean-machine install/update/recovery and operational material | All required completion rows accepted on the candidate; qualification rules below satisfied |

Stage 1 supplies real project work rather than waiting for complete language tooling; the complete language loops close in Stage 2. Extension/remote architecture and procurement may begin early, but their full user workflows remain required Stage 5 outcomes. Signing procurement starts in Stage 0; signed artifacts must be available before clean-machine trust and real update acceptance. External blockers stay on the critical path and are never treated as code completion.

Each stage is decomposed into bounded work packages with a user outcome, owned modules, prerequisites, implementation/repair tasks, tests, product acceptance, rollback/recovery, evidence paths and reviewer. Plan the work far enough to establish dependencies across all stages; refine subsystem internals through separate specs before risky interface changes.

Stage 0 must assign every canvas requirement a concrete journey and stage owner: core editing/arrangement in Stage 1, AI/workflow integrations in Stages 3-4, and team integrations in Stage 5 where promised. No requirement can remain assigned only to final qualification; Stage 6 validates completed implementations rather than absorbing unowned feature construction.

## 6. Verification design

### Three layers

1. Component: algorithms, protocol contracts, policy and failure behavior. Mocks and fixtures are valid here.
2. Integrated workflow: real services/processes, real files/repositories and independently observed effects. State clearly where any dependency is substituted.
3. Product acceptance: ordinary packaged executable, user-facing setup, actual keyboard/mouse/accessibility interactions, real supported tools/providers/endpoints and externally checked results.

Direct runtime dispatch, test-only state injection, fabricated verification results, deterministic model replies and display-model assertions cannot close layer 3. External input automation is permitted. Process supervision, log collection and fault injection may be external to the app, but cannot insert successful app state. Test accounts and reproducible projects are allowed when they still exercise real execution.

A final acceptance scenario cannot substitute a dependency whose real behavior is part of the claimed user outcome: a fake adapter cannot certify debugging, a fabricated verification record cannot certify tests, a local transport stand-in cannot certify remote work, and a fixture provider cannot certify AI usefulness. Substitution disclosure does not waive this rule. Such a run remains supporting evidence only.

Keep existing smokes and replay suites. Label their layers accurately and supplement them. New acceptance must check prerequisites, the intended control/focus, loading/error feedback and actual effects, preventing a missed click, absent preview or echoed command from satisfying an assertion.

### Acceptance record

Every scenario records requirement IDs, task/outcome, artifact hash and source SHA, OS/hardware, dependency versions, input route, setup, expected external effects, observed results, applicable recovery cases, logs/captures, test substitutions, defects, reviewer and coverage gaps. Retain failed attempts; reruns do not erase intermittent failures.

Capture metadata and redacted diagnostics by default. Raw source, secrets, private conversations and credentials must not leak through acceptance recordings. Source-bearing artifacts require explicit authorization and controlled retention under the existing policy.

### Required failure classes

Apply the relevant classes to each feature: denied trust/capability, missing or incompatible tool, malformed output, slow/unreachable dependency, timeout, cancellation, process crash, restart, stale response, external edit, merge conflict, low disk, partial write/download, resource exhaustion, permission change and disconnected/reconnected remote service. Irrelevant cases require a written rationale rather than an empty field.

Safety outcomes are mandatory: preserve user work; show accurate status; never report verification/apply success from intent alone; no silent cloud fallback; no unauthorized workspace writes; contain and clean up workers. AI unavailability must be visible in production workflows rather than silently substituted with successful-looking fixture output.

### Real-model evaluation

Separate useful-model evaluation from deterministic regression. Required configurations perform representative held-out tasks across all three language groups, including multi-file edits, tests, failures and recovery. Define per-task external success checks and repeated-trial pass thresholds in Stage 0 before qualification runs; do not move thresholds after seeing results. Publish success rates, failures, resource costs and limitations by model/configuration. Safety invariants cannot be traded for average task success.

Live model and external-service evaluation remains separate from credential-free PR gates. Required release acceptance may be blocked by unavailable credentials or failed live evaluation even while PR gates are green. Provision endpoints through owner-authorized infrastructure; never embed secrets or enable paid/remote execution implicitly.

## 7. Program records and regression policy

Use an authoritative completion register mapped to existing kanban/readiness IDs. Its implementation field distinguishes absent, partial, implemented, and repair-required. Its acceptance field distinguishes unassessed, blocked, failed, and accepted. Compatibility coverage and evidence freshness are separate fields. Record fixture usage explicitly.

For each requirement, record scope/source, user entrypoint, authoritative service, dependencies, work package/owner, acceptance scenarios, platform/configuration coverage, defects and evidence. Existing kanban `done` remains historical; it does not force the new acceptance field to accepted.

The forthcoming implementation plan must introduce validation that rejects unmapped required scope, missing scenarios, invalid evidence references and incomplete matrix coverage. It must not pass a row merely because a file exists. Completion dashboards report separately: implemented requirements, accepted requirements, blocked configurations and failed journeys. No blended percentage is a production verdict.

Mechanically, every required product row must link at least one passing layer-3 scenario covering its user outcome and all required configurations, plus its applicable recovery evidence. Internal-only requirements instead link to the product outcomes they protect and their negative/safety scenarios; they cannot inflate accepted feature counts. The register validator must reject implementation-only or fixture-only evidence offered as final acceptance. Security and enterprise claims use the same stable IDs, named reviewers and explicit pass/fail evidence as functional requirements.

Evidence belongs to immutable builds. A change affecting a requirement invalidates its acceptance until the relevant scenarios are rerun. Final qualification reruns the complete required matrix on the candidate. Reuse unchanged historical evidence only for supporting analysis, not as a substitute for that final run.

Retain the standing gates. Introduce native product gates in phases under the existing hosted-promotion policy. Until promoted, require their recorded results for milestone/release decisions separately. Do not silently redefine existing workflow triggers or make live providers compulsory for every PR.

## 8. Completion and release decision

The following are proposed concrete qualification defaults for written-spec review:

- All required requirements and matrix entries accepted; no unbuilt promised feature hidden behind a final-release deferral.
- Zero unresolved P0/P1 defects and zero known defects of any severity that invalidate a required acceptance outcome. Lesser cosmetic issues may be documented only when all required outcomes remain satisfied.
- A ten-working-day owner dogfood period using Legion for real development, plus repeatable native acceptance on Windows/macOS/Linux. Restart the affected stability observation after a data-loss, authority violation or recurring workflow-blocking fix; rerun the full candidate suite after final changes.
- Independent usability acceptance by at least two people who did not implement the workflows, completing unfamiliar representative development tasks through the published documentation without implementer coaching. Record completion, assistance, errors and abandoned steps; a required journey that needs an undocumented workaround fails acceptance. This complements the owner's sustained dogfood period.
- Real Rust, TS/JS and Python journeys, real local/hosted AI qualification, extension/remote/team contracts, accessibility and measured workload budgets all pass their declared matrices.
- Clean-machine installation, signature/trust validation and real installed-version update replacement/restart/rollback demonstrated on each supported OS, including interruption cases.
- Independent review of evidence and the release decision. Implementation agents may prepare evidence but cannot solely certify their own completion. External audit and security qualification requirements from the existing roadmap remain required before their associated enterprise claims.
- Operating/support documentation matches observed behavior, and diagnostics, consent, retention, repair/uninstall and recovery procedures work.

Intermediate builds may be internal dogfood or limited previews with named limitations. They cannot be called final feature-complete releases. A deadline or budget cannot close a failed requirement; it can only motivate a separately approved change of product scope.

## 9. Risks and controls

| Risk | Control |
| --- | --- |
| Another layer of plans without usable software | Work packages end in native user outcomes; implement and accept a vertical journey before adding breadth |
| Existing docs overstate or understate current behavior | Source traces plus live baseline; preserve uncertainty until exercised |
| Narrow scenarios overfit implementations | Independent scenario review, realistic repositories, holdouts, repeated use and adverse states |
| Broad compatibility becomes unlimited scope | Ratified finite matrices covering every promised capability; track version boundaries explicitly |
| Too many agents modify composition simultaneously | Explicit module ownership, bounded extraction and serialized integration |
| Tool provisioning/certificates/models arrive too late | Stage 0 owner-assigned external work and visible dependency gates |
| Release performance/a11y work is postponed | Platform and workload validation runs from Stage 1 and accompanies each expansion |
| User work or private data is damaged during qualification | Disposable qualification workspaces, explicit consent, redacted evidence and recovery verification |

## 10. Next artifact and approval boundary

After written-spec review, create the dependency-ordered implementation plan with stable requirement/work-package IDs, concrete files, acceptance commands/scenarios, independent owners, external actions and milestone checklists. Include the complete inventory and compatibility-matrix work as explicit deliverables, with their approval gates before dependent scope is implemented. No calendar estimate is inferred from backlog counts.

The planning deliverables must be indexed in `docs/INDEX.md`. Reconcile governing roadmap relationships explicitly rather than creating a second conflicting status source. The historical master plan and readiness rows remain unchanged until the implementation plan specifies and validates their reconciliation.

Written-spec approval authorizes implementation-plan preparation. Product implementation, infrastructure changes, publication and purchases are separate actions and are not performed by this document.
