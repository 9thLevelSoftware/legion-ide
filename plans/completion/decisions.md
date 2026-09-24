# S0-01 inventory decisions

## Authority and status

The approved product completion design (`docs/superpowers/specs/2026-09-04-product-completion-design.md`) governs scope and acceptance. The kanban and historical ledgers remain source history and implementation leads. `acceptance` is `unassessed` for every inventory row; kanban `done` values and retained evidence do not promote acceptance.

The initial inventory baseline contained 197 rows: 165 kanban task outcome rows from 163 task IDs plus 32 additional source-scope outcomes (22 approved feature families and GAP-01..GAP-10). That count is historical for the initial reconciliation and is not the current register total. Legacy task IDs are retained in `legacy_ids`; source references carry path and stable task/scope identity. S0-02 owns population of scenario/configuration IDs, so those arrays are intentionally empty here.

## Conflict resolutions

- **LSP startup wording:** the user guide's implicit Rust-LSP startup wording conflicts with current source tests and the language evidence showing capability-gated startup. The current source behavior and approved completion design govern implementation facts; the ordinary user workflow must be qualified explicitly before acceptance.
- **Fixture defaults:** historical fixture/default claims are treated as test setup evidence only. They do not establish a product default or acceptance result; a native scenario must verify the default.
- **Canvas scope:** existing canvas arrangement evidence is not the full `docs/ui/canvas-workspace-direction.md` promise. The full direction is retained as a separate scope requirement and remains unassessed.
- **Release capability:** historical release/readiness claims do not establish signed, update-safe product capability. GAP-01 through GAP-10 and the release/escrow sources remain required outcomes. The current dry-run/unsigned limits are implementation facts until qualifying evidence exists.
- **VSIX metadata restriction:** the full approved completion specification supersedes the historical metadata-only VSIX restriction. The promised extension surface includes the required webview, notebook, custom-editor and storage capabilities. A future authority amendment is still required before Node activation; metadata parsing or installation alone cannot satisfy that promise.
- **Stale surface-freeze citations:** ADR-0046 freeze citations are retained as historical/deferred-source references. The current project instructions state that the freeze is retired; deferred rows remain requirements only where their own source/evidence requires them.

## Unavailable sources

The backlog documents that `.hermes/plans/2026-06-13_173122-legion-current-to-ga-kanban-plan.md` was removed; it is unavailable and was not inferred. `ENGINEERING_STATUS.md`, `ENGINEERING_AUDIT.yaml`, and `ENGINEERING_PLAN.yaml` are also unavailable after cleanup. Historical `plans/legion-production-master-plan-v0.1.md` and the e2e source package remain supporting leads only; their historical implementation claims were not treated as current truth. The accessible replacement/source trail is the current kanban, completion traceability register, approved completion design, v0.2 master plan, current roadmap/ledgers, installed-product sequence, canvas direction, ADRs, and retained evidence.

## Inventory method

Implementation classifications are conservatively `partial` pending direct current code/evidence review. No classification was copied from kanban status or inferred from filename existence; no row is marked `implemented` or `absent` without a verified trace. This is an inventory signal, not acceptance, and the remaining classification slice is explicitly open for follow-on review.

## S0-01a lossless acceptance extraction (2026-09-04)

This bounded repair used Python 3.12.14 with stdlib `tomllib` to parse `plans/kanban/legion-ga-backlog.toml`. It found 163 task IDs and 165 acceptance strings. Task rows in `requirements.json` now retain their existing fields and stable first IDs, use the exact acceptance string as `title`, and carry an additional `source_refs` entry identifying the exact task and one-based acceptance ordinal (`<task-id> acceptance[<ordinal>]`). The two multi-outcome tasks, `P1.F3.T2` and `P6.F5.T1`, retain their original first requirement ID and receive an additional `-02` row ID. The 32 non-kanban rows were preserved unchanged.

Validation confirmed the historical 197-row baseline, 165 task outcome rows, 32 non-kanban rows, exact `(legacy id, ordinal, text)` mapping, unique requirement IDs, zero missing task IDs, and `acceptance = "unassessed"` for all rows. Later scope expansions own the current register total; this increment does not mark S0-01 complete.

The other independent S0-01 review items remain explicitly open for separate increments: concrete expansion of family/GAP placeholder rows, implementation classification, stable package definitions/routing, internal-to-product `protected_product_ids`, and the existing internal self-link concern. Duplicating rows did not repair those links; that is outside S0-01a.

## S0-01b package and internal-outcome mapping repair (2026-09-04)

This bounded mapping repair assigns each initial 197-row baseline row to a package defined by the Stage 0, Manual/language, AI/team, or production-qualification plan headings. Invented aggregate owners (`S0-BASELINE`, `S1-MANUAL`, `S2-LANGUAGE`, `S3-AI`, `S4-ORCHESTRATION`, and `S5-EXTENSIONS-TEAM`) were replaced with concrete package IDs such as `S1-04`, `S2-02`, `S3-05`, `S4-02`, `S5-09`, and `XQ-01`-family IDs where the qualification plan owns the outcome. Stage values now follow the package's declared implementation stage, including the XQ exceptions (`XQ-02` stage 1, `XQ-03`/`XQ-07`/`XQ-08` stage 0, and `XQ-04`/`XQ-05`/`XQ-06` stage 1).

The Assist family placeholder (`COMP-SCOPE-FAMILY-12`) was expanded into 9 Assist-specific integration/workflow rows. Provider setup/runtime/context/usage remain owned by canonical `COMP-PROV-*`/`COMP-CTX-*` rows; exact dependencies and protected owner IDs are recorded on each Assist row. Existing P3/P4 legacy IDs remain on their original rows and are not duplicated in the family expansion. Implementation classifications distinguish partial substrate from absent workflow; every new row remains `acceptance: unassessed`.

Routing rationale by legacy family: P0 documentation, governance, and licensing rows route to S0-06; Kanban inventory rows route to S0-01; baseline/build and gate observations route to S0-05. P1 routes by Manual authority, workbench, editing, and evidence outcomes to S1-02/03/04/08. P2 routes language lifecycle/refactoring/debug/test outcomes to S2-02/03/04/05, while terminal, search, and Git outcomes route to S1-06/05/07. P3-P5 route proposal, provider, context, Assist, Delegate, sandbox, and agent-loop outcomes to S3-01/02/04/05/06. P6 routes workflow orchestration to S4-02/03/04/05, with the Canvas outcome owned by S1-03C. P7 routes extension outcomes to S5-01/02/03. P8 routes directly to XQ qualification packages. P9 routes evaluation to S3-07, security audit to XQ-08, collaboration to S5-09/10/11, and training/telemetry to S5-12/13. Family and GAP placeholders receive dominant implementation owners using the same semantic routing; their concrete expansion remains open for a later increment.

Internal rows now reference an existing relevant product requirement through `protected_product_ids`; product rows have empty protected lists. No internal row self-links, no source-extracted title/ID/source-ref fields, and no `acceptance` or `implementation` values were changed. This remains provisional routing metadata and is not implementation or acceptance evidence; no S0-01 completion claim is made.

### S0-01b review-round-1 fixes (2026-09-05)

The review correction keeps `owner_role = "luna_worker"` because the current user instruction explicitly selects Luna execution; this is execution ownership for this bounded inventory repair and does not override the selected package's future implementation/reviewer model.

`P9.F2.T2` now routes to `S3-04`/stage S3 for secret-rule implementation and negative coverage, while `P9.F2.T3` routes to `S5-11`/stage S5 for signed policy bundles, ceilings, retention, and export enforcement. `P9.F2.T1` is an internal security-model requirement owned by `S5-11`; `P9.F2.T4` remains an internal external-audit requirement owned by `XQ-08`. Neither audit nor qualification is treated as construction of the product outcomes.

Internal protection relationships were re-evaluated against the final `kind` assignments. P0.F4.T1 protects concrete distribution/docs, installed-product, and signing outcomes (DIST-009, GAP-01, GAP-02); P0.F4.T2 protects GAP-01; P0.F4.T3/T4 protect Manual distribution/package outcomes (FAMILY-22 and GAP-02); P0.F4.T5 protects the GP-1/2/3 product journeys; P0.F4.T6 protects the Rust diagnostics product outcome; P0.F5 protects the recovered SmallCode corpus outcome; P8.F1 protects release/install outcomes (GAP-02 and FAMILY-22); P9.F2 internal rows protect enterprise security policy (FAMILY-19). No internal row points to an internal row or to itself.

`COMP-SCOPE-GAP-06` remains a provisional dominant owner for the Manual/no-egress family only. Its source expansion remains open across S0-05, XQ-01, and later qualification; `XQ-06` alone is not claimed to construct the full artifact or no-egress controls.

### S0-01b review-round-2 fix (2026-09-05)

`P9.F2.T2` is classified as internal because its exact acceptance is fixture/test proof for secret-detection rules. It remains owned by `S3-04` at stage S3 and protects the existing product redaction outcome `COMP-P4-F2-T3-1` (“The bytes sent equal the manifest minus redacted items, with no other delta.”).

## S0-01c legacy implementation classification (2026-09-05)

The current-source implementation inventory is recorded in [implementation-audit.md](implementation-audit.md). It covers all 165 legacy acceptance outcomes exactly once, including separate rows for the two outcome pairs `P1.F3.T2` and `P6.F5.T1`; all `acceptance` values remain `unassessed`. Classifications are implementation facts only: native, hosted, renderer, external-tool, and product acceptance remain separate follow-on evidence.

The audit promotes only behavior established by current source and test bodies. `P1.F4.T5` is `implemented` because the standard performance harness wires the renderer workload, while measurement remains unassessed. `P2.F2.T5` remains `partial` because its ConPTY evidence is metadata parity rather than runtime equivalence. `P2.F4.T4` remains `partial` pending proof of the exact workload semantics and measurement. Missing run evidence alone does not lower an otherwise concrete internal implementation.

The native exploratory lead for `P1.F2.T3` exposed a current-source gap: the desktop keyboard path has no Home/End editor mapping. That row therefore remains `partial`; the observation does not promote or invalidate unrelated native acceptance rows, and the Ctrl+S ingress hypothesis remains separate from the proven command-palette save path.

The independent S0-01c review corrected three classifications against the exact legacy caveats: `P7.F2.T4` is implemented metadata-only VSIX reporting (`NodeSidecar` is classification data, not execution), `P8.F1.T2` is implemented through explicit unsigned-beta/dry-run release descriptors and tests, and `P8.F3.T2` is implemented for the amended v1 consented Rust-panic capture contract. `P8.F1.T3` and `P9.F2.T4` remain absent because the fresh-VM result and archived external audit report are missing.

Round-two source review keeps `P0.F1.T1` and `P0.F1.T2` partial. The canonical mode table and labels exist, but no gate proves every mode-mentioning page links `docs/MODES.md`, and stale labels remain in `mockups/design.md`, `mockups/src/imports/design.md`, and `docs/LEGION_PIVOT.md`; current docs-hygiene checks only exact stale headings.

## S0-01d text-editing scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-01` row is replaced by thirteen atomic rows
`COMP-EDIT-001..013` in package `S1-04`. Each new
row retains the approved `family-01` source identity and remains
`acceptance = unassessed` with empty scenario/configuration coverage. Exact
existing outcomes are mapped in `plans/completion/text-editing-scope-audit.md`
instead of duplicated: multi-cursor is `COMP-P1-F3-T2-1-02`, Vim is
`COMP-P1-F3-T5-1` for generic key-feed safety while named motions, operators,
register, and insert-entry capabilities are explicit `COMP-EDIT-010..013`
companions, and large-file behavior is covered by
`COMP-P1-F4-T2-1` through `COMP-P1-F4-T5-1`.

The approved S1-04 deliverable is the governing text-editing expansion. The
full-vision rectangular editor selection is retained as `COMP-EDIT-009` and
is distinguished from spatial Canvas selection. Historical implementation/status
claims do not promote any new row; the audit records baseline `1895fda2` code
traces and explicit evidence limits. New source paths are literal files with
section locators in identities. This increment does not claim full S0-01 or
full S0 completion.

The large-file mapping preserves `P1.F4.T5` as the committed
`implementation = implemented` classification: the standard harness wiring is
implemented, while renderer measurement and product acceptance remain
unassessed. The audit does not weaken that existing legacy row.

## S0-01e terminal scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-07` row is replaced by fifteen atomic rows `COMP-TERM-001`, `COMP-TERM-002`, and `COMP-TERM-004..016` (with stable TERM-003 removed as an exact duplicate) in package `S1-06`. The approved family and S1-06 terminal deliverable are expanded into explicit shell execution, shell profiles, PTY/input/resize, scrollback/search/rendering, TUI/control keys, OSC cwd/boundaries, task/output references, trust/redaction/env, lifecycle recovery, agent proposal, platform parity, and independent process-evidence outcomes, and multi-session terminal tabs. Exact legacy outcomes P2.F2.T1 through T5 are mapped in `plans/completion/terminal-scope-audit.md` rather than duplicated. The retained P2.F2.T2 row owns the exact trust-denial outcome; all remaining new and retained product rows remain `acceptance = unassessed`; implementation values are source traces only. Source paths are literal files with section locators, and committed baseline `8d151d5` was used for implementation evidence. This increment does not claim full S0-01 or S0 completion.

## S0-01f Workbench scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-02` row is replaced by thirteen atomic rows `COMP-WB-001..013`. Outcomes are routed to their actual packages: S1-03 for tabs/splits/dock/focus/windows/tree/open/reveal/commands, S1-05 for Explorer create/rename/move/delete, S1-04 for settings profiles and keymaps, and S1-08 for native window/focus/DPI/accessibility qualification. Exact broad legacy outcomes remain mapped rather than duplicated (`COMP-P1-F2-T2-1`, `COMP-P1-F2-T3-1`, `COMP-P1-F2-T4-1`, and retained P6.F5 canvas rows). The approved S1-05 create/rename/move/delete/open/reveal promise is current required scope: WB006 explicitly covers both file/folder open/reveal, while WB010-WB013 explicitly cover both file/folder create/rename/move/delete; historical deferrals do not omit it. No inspected source promises multi-root switching, so it is not invented. All new acceptance remains `unassessed`; this increment does not claim S0 completion.

## S0-01g Canvas scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-03` row is replaced by ten atomic Canvas outcomes `COMP-CAN-001..010` in package `S1-03C`. Existing exact P6.F5 arrangement and person-edge outcomes remain canonical and are mapped rather than duplicated. New rows cover spatial pan/zoom, accessible movement, connection lifecycle, full promised node kinds, groups/scope/minimap, navigation/accessibility, workflow-node execution, provenance-labeled derived edges, and native per-configuration EvidenceRun/recovery/performance evidence. Editor text controls remain in `COMP-EDIT-*`; no editor mutation is claimed for Canvas. Current ADR-0051 restrictions and unavailable `docs/superpowers/specs/2026-09-04-canvas-full-vision-spec.md` are recorded as limits/design gates, not as omissions of approved Canvas scope. All new acceptance remains `unassessed`; this increment does not claim S0 completion.

## S0-01h Navigation/search scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-04` row is replaced by thirteen atomic outcomes `COMP-NAV-001..013` in package `S1-05`. Exact retained P2.F4 broad outcomes remain canonical for repo-search speed, proposal-only replacement, and large-fixture bounds; Workbench command/CRUD rows and broad P2.F1 language rows remain mapped rather than duplicated. New rows cover quick file/symbol/definition/references navigation, recent buffers and local history, literal/regex search options and ignore/binary rules, cancellation/stale/degraded/error handling, reviewable replacement preview/cancel/apply, stale/conflict/path denial, and the required real-tree native workflow/evidence. All new acceptance remains `unassessed`; this increment does not claim S0 completion.

## S0-01i Language tooling scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-05` row is replaced by thirteen atomic outcomes `COMP-LANG-001..013` routed to S2-01 through S2-06. The rows preserve the approved Rust, TypeScript/JavaScript, and Python matrix, provisioning/version/hash/prerequisite configuration, real-server LSP projections, proposal-mediated edits, build/test, debug, lifecycle/recovery, and native EvidenceRun promises. Exact retained P2.F1 registry/lifecycle/LSP/call-hierarchy rows and P2.F3 test/debug rows are mapped in `plans/completion/language-tooling-scope-audit.md`, not duplicated. Committed evidence is Rust-focused: TS/JS registry declarations do not prove a desktop live route, and the Python registry URL is an invalid example URL with no materializer; these are recorded limits, never native acceptance claims. All new acceptance remains `unassessed`; this increment does not claim S0 completion.

S0-01i fix1 preserves the explicitly named S2-01 fixture/project identity and fixture hashes/provenance in LANG-001, names organize-imports in LANG-007, and names targeted/grouped build-test execution and output in LANG-011; all remain acceptance-unassessed.

## S0-01j Build/test/debug scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-06` row is replaced by nine atomic outcomes `COMP-BTD-001..009` routed to S2-04, S2-05, and S2-06. The rows preserve approved four-language configuration/prerequisite, build/task/test execution, targeted/group result output, failure/stop/rerun/recovery, real DAP launch/attach/inspection/terminate, and native EvidenceRun promises. Exact retained P2.F3 rows and broad LANG-011/LANG-012 outcomes are mapped in `plans/completion/build-test-debug-scope-audit.md` rather than duplicated. Protocol, fixture, mock, and projection traces remain implementation evidence only; all acceptance remains `unassessed`. This increment does not claim S0 completion.

S0-01j fix1 removes redundant `COMP-BTD-010`: the broad four-language real-runner/adapter promise is already owned by `COMP-LANG-011` and `COMP-LANG-012`. Stable `COMP-BTD-001..009` remain for narrower configuration, execution/result, recovery, inspection, and native-journey outcomes.

## S0-01k Source control scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-08` row is replaced by ten atomic outcomes `COMP-SCM-001..010` in S1-07. The rows preserve status/diff, file and partial-hunk staging, commit, branch safety, conflict resolution, policy-visible remote verbs, local history/checkpoint restoration, worktree management, dirty/external/restart/storage recovery, and host-observed native Git effects. Exact retained P2.F5 outcomes are mapped in `plans/completion/source-control-scope-audit.md`, not duplicated. Read/projection contracts are not treated as actual Git operation proof; no remote/auth mutation was performed. All acceptance remains `unassessed`; this increment does not claim S0 completion.

## S0-01l Work preservation scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-09` row is replaced by thirteen atomic outcomes `COMP-PRES-001..013`. `COMP-PRES-012` is owned by S1-06 and covers terminal cancellation/process loss preserving active editor text, selection, and undo/redo history; `COMP-PRES-013` is owned by S2-05 and covers the equivalent debug-adapter cancellation/loss outcome. `COMP-PRES-011` in S1-08 qualifies the native host-observed recovery evidence for both. Existing EDIT, WB, and SCM rows are exact deduplication targets; narrower cross-surface recovery promises remain explicit. Protocol/read-model/harness traces are not native proof. All acceptance remains `unassessed`; this increment does not claim S0 completion.

## S0-01m Provider/model setup scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-10` row is replaced by eleven atomic outcomes `COMP-PROV-001..011` routed to S3-01, S3-02, S3-03, and S3-07. The rows preserve the full-vision local/hosted setup matrix, credential lifecycle, endpoint/model selection, capability and health metadata, policy/egress controls, cost/usage ceilings, managed runtime, model acquisition/integrity, hardware fit, runtime recovery, and native first-run/provider acceptance. Exact retained P4.F1 provider rows are mapped to `COMP-PROV-002`, `COMP-PROV-003`, `COMP-PROV-005`, and `COMP-PROV-011` where their narrower promises overlap; no retained row is treated as proof of the missing managed catalog, hardware-fit, or native acceptance outcomes. Current source classifications remain conservative (`partial` for existing provider/policy substrate, `absent` for managed catalog/hardware/native qualification); all acceptance remains `unassessed`.

## S0-01n Context, index, and memory scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-11` row is replaced by ten atomic rows `COMP-CTX-001..010` in S3-04/S3-07. They cover the approved context manifest, freshness-aware index selection, semantic retrieval, opt-in memory, provenance, deterministic budgets, privacy boundaries, projection-only inspection, metadata replay/recovery, and packaged native acceptance. Existing legacy outcomes remain canonical and are mapped rather than duplicated. Current source traces support conservative `partial` classifications for CTX-001..009; CTX-010 is `absent` because no required native EvidenceRun records were found. All acceptance remains `unassessed`; this increment does not claim S0 completion. See [context-memory-scope-audit.md](context-memory-scope-audit.md).

The exact retained mappings are `COMP-P4-F2-T1-1` → CTX-001/005 (structured manifest and item metadata), `COMP-P4-F2-T2-1` → CTX-007/008 (before-run privacy and planned egress inspection), `COMP-P4-F2-T3-1` → CTX-006/007 (reviewed payload binding and redaction), `COMP-P4-F2-T4-1` → CTX-008/009 (after-run inspection and replay), and `COMP-P3-F2-T3-1` → CTX-008/009 (structured evidence projection and replay metadata). The retained rows remain canonical owners; these mappings document overlap and boundary without duplicating their IDs.
## S0-01p Delegate scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-13` row is replaced by eleven atomic rows
`COMP-SCOPE-FAMILY-13-01..11` in S3-06/S3-07. They cover the approved
user-authored task/scope/plan, isolated worktree and sandbox execution, real
tool turns, budgets, verification, proposal review, cancellation/kill,
durable checkpoint resume, cleanup, truthful command-center projection, and
packaged native Delegate acceptance. Shared provider, context, proposal,
checkpoint, terminal, build/test, and sandbox outcomes are dependencies or
canonical retained owners rather than duplicated Delegate rows. Existing
Assist/PROV/CTX/P3/P4 owners remain unchanged; exact overlap is mapped in
`plans/completion/delegate-scope-audit.md`. Current source traces support
`partial` for the existing plan, sandbox, loop, budget, verification,
proposal, cancellation, cleanup, and projection substrate; durable resume is
`absent`; packaged native acceptance is `absent`. All acceptance remains
`unassessed`, and this increment does not claim S0 completion.


## S0-01q multi-agent and interoperability scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-14` row from baseline `327bf69` is replaced by twelve atomic rows `COMP-SCOPE-FAMILY-14-01..12`. The rows cover editable plans and dependencies, scheduling and concurrent workers, isolation, budgets, conflict resolution, proposal-only merge readiness, fleet controls, interruption/recovery, truthful command-center projection, production MCP client interoperability, external ACP-agent containment, and packaged native qualification. Shared Delegate, provider, context, proposal, checkpoint, terminal, and build/test promises remain canonical dependencies; no duplicate Assist/Delegate/PROV/CTX/shared rows are introduced. Existing editor routing remains untouched. All acceptance values are `unassessed`; current source classifications are conservative and native qualification is `absent`. Literal source paths and stable identities are recorded in `orchestration-scope-audit.md`.

Family-14-10 classification correction: current `crates/legion-ai-providers/src/lib.rs` contains bounded `McpClient<T>` behavior for registry validation, list/reload, resource/prompt/tool request construction, and permission-gated tool calls. This supports `implementation = partial`; the complete named-peer supervision, reconnect, and packaged qualification promise remains unproven, and acceptance remains `unassessed`.

## S0-01r external interoperability scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-15` row from baseline `7a7cf0a` is replaced by five atomic rows `COMP-SCOPE-FAMILY-15-01..05` in S0-02/S4-04/S4-05. The rows cover the missing Stage 0 named-peer compatibility contract, Legion's MCP client role, Legion's MCP server role, named-peer ACP host interoperability, and equivalent external containment. Family 14 remains the canonical owner for orchestration integration and packaged multi-agent/MCP/ACP qualification; provider/model, context/index/memory, trust/proposal, Delegate, and workflow outcomes are dependencies or existing canonical rows rather than duplicates.

Current code is classified conservatively. `McpClient<T>` plus stdio and Streamable HTTP conformance fixtures support `partial` MCP client evidence; `mcp_server.rs` is a local server substrate without named external-client qualification; and `AcpHostCommand`/`legion-agent::external` implement a supervised env-var adapter and proposal/evidence conversion rather than ACP session/initialize interoperability. The Stage 0 peer matrix, named real-peer runs, full ACP transport, and packaged qualification remain unassessed or absent as recorded in `newinterop-scope-audit.md`. All replacement acceptance values remain `unassessed`; no S0 completion claim is made.

The old `plans/adrs/ADR-0039-agent-interop.md` local-stdio/post-GA server limitation remains historical context and is not changed by this inventory. The approved full-product design and `docs/superpowers/plans/2026-09-04-ai-team-completion.md` supersede that limitation as target scope; required server role, named peer, and ratified-transport promises therefore remain in family 15. No transport is selected here, and the full real-peer contract is owned by S0-02.

## S0-01s trust/proposal scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-16` row is replaced by six atomic outcomes
`COMP-TRUST-001..006` in S3-01/S3-07. The rows cover the generalized
app-owned proposal lifecycle, authenticated deny-by-default validation, batch
atomicity and rollback with dirty-work preservation, audit-before-success and
metadata-only redaction, projection-only trust/privacy/egress controls with
Manual zero-egress, and packaged native trust/proposal qualification. Exact
retained P3 lifecycle/review/risk/checkpoint rows, provider/context/manual
rows, work-preservation rows, and family-13/14/15 outcomes remain canonical
dependencies or protected owners rather than duplicates. Current source
classifications are `partial` for the authority/projection substrate and
`absent` for packaged qualification; all acceptance remains `unassessed`.
Source paths in the replacement rows are literal existing files without
fragment references. This increment does not claim S0-01 or S0 completion.

## S0-01t extension scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-17` from baseline `ddfb166` is replaced by
eleven atomic outcomes `COMP-SCOPE-FAMILY-17-01..11` in S5-01/S5-02/S5-03/
S5-04/S5-15. The rows cover the full host/API/contribution contract, signed
WASM execution, extension services and proposal mediation, install/update/
disable/remove/rollback lifecycle, deny-by-default security and isolation,
workspace/global/secret storage, the supported VS Code Node/web-worker subset,
webviews, notebooks, custom editors, and packaged Stage 5 qualification.

The approved completion design supersedes the historical metadata-only VSIX
restriction for required webview, notebook, custom-editor, and storage
capabilities. ADR-0047 still limits Open VSX to declarative metadata and the
current compatibility crate remains classification-only; neither metadata nor
the existing `NodeSidecar` descriptor is treated as runtime acceptance.
Existing P7.F2 install/permission/tamper/metadata rows, generalized trust and
proposal rows, save-conflict preservation, and the family-15 Stage 0 matrix
remain canonical owners and are referenced rather than duplicated. All new
rows remain `acceptance: unassessed`; implementation classifications are
conservative current-source facts. See `extensions-scope-audit.md`.

## S0-01u remote development scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-18` row is replaced by eighteen atomic
outcomes `COMP-REMOTE-001..018` in S0-02/S5-05/S5-06/S5-07/S5-15. The rows enumerate
the approved SSH and dev-container matrix; host authentication, host-key and
secret handling; remote workspace identity and negotiation; file browsing,
watching and proposal-mediated save; remote search/tools; terminal/task,
LSP, build/test/debug, Git, extension/tooling, and port forwarding; cancellation
and cleanup; disconnect/offline/reconnect; agent upgrade/rollback and recovery;
privacy/egress and dirty-work preservation; and packaged real-remote
qualification across Rust, TypeScript/JavaScript, and Python.

Remote-specific rows do not duplicate canonical local outcomes. `COMP-PRES-*`
owns general save, conflict, restart, and dirty-text preservation; `COMP-LANG-*`
owns language lifecycle, LSP, build/test, and debug semantics; `COMP-TERM-*`
owns local terminal behavior; and `COMP-TRUST-*` owns generalized proposal,
audit, capability, and egress controls. Remote rows cover the remote binding,
authority, endpoint, and product integration of those outcomes and depend on
the canonical IDs where needed.

Current source supports only a deterministic, default-off metadata-first
remote harness. The connection planners parse SSH/dev-container metadata;
`RemoteSessionRuntime` validates identity, trust, bounded filesystem
operations, descriptor-only process/PTY/LSP/semantic requests, proposal
preconditions, cancellation metadata, reconnect/offline transitions, and
metadata-only audit. `RemoteTransportStateMachine` and the forced-drop tests
cover transport state, replay, resume, flow control, and mTLS policy helpers.
There is no current SSH connector, dev-container engine, remote file watcher,
real remote process/LSP/debug/extension service, port-forwarding product path,
agent package installer/rollback workflow, desktop remote UX, or packaged
real-endpoint qualification. Remote Git is a separate `COMP-REMOTE-018` row
that depends on canonical `COMP-SCM-001/002/003/006/009/010` outcomes and adds
only the remote repository boundary and evidence. Those rows are therefore
`partial` only where a
current source trace exists and otherwise `absent`; every new acceptance value
remains `unassessed`.

The accepted Phase 7 ADRs and retained reconnect evidence are treated as
current substrate evidence, not as permanent deferrals. ADR-0025 records the
production transport direction and its evidence gates; the approved Stage 5
plan remains the scope authority. See `remote-scope-audit.md`.

The finite remote host/container matrix is explicitly a Stage 0/S0-02
prerequisite. The review request to replace the selected `luna_worker`
`owner_role` with generic package implementation owners is rejected for this
inventory increment: the user instruction selects Luna-only execution. The
owner field records this bounded inventory owner and does not claim future
implementation ownership or acceptance authority.

## S0-01v collaboration/enterprise scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-19` from baseline `07f068d` is replaced by
twelve atomic outcomes `COMP-COLLAB-001..006` and `COMP-ENT-001..006` in
S0-02/S5-08/S5-09/S5-10/S5-11/S5-14/S5-15. The rows cover the finite
collaboration and enterprise matrix, tenant/workspace/document identity and
roles, authenticated durable collaboration transport/control plane, replay and
recovery, concurrent reconciliation/presence, shared proposal review/quorum,
OIDC SSO, SCIM provisioning, signed policy distribution/enforcement, audit
export and retention/deletion, deployable service operations, and packaged
Stage-5 qualification.

Current source is classified conservatively: the deterministic in-process
collaboration runtime, protocol/session DTOs, app-owned shared-proposal gates,
signed org-policy substrate, metadata-only audit paths, and retention vault
support `partial` rows; the Stage-0 matrix, OIDC/SCIM, durable network service,
service packaging, and real packaged qualification are `absent`. Every new
acceptance value remains `unassessed`.

Canonical proposal, dirty-buffer, audit, projection, language, terminal, Git,
provider, context, extension, remote, and training/telemetry outcomes remain
deduplicated. `plans/completion/collaboration-enterprise-scope-audit.md`
records literal current source paths, evidence limits, and the exact ownership
boundaries. The approved Stage-5 plans remain required scope; historical
deferred-surface language is not converted into a permanent deferral. The
bounded inventory owner is `luna_worker` per the user override.

The aggregate replacement also rewires the only two retained protection links
that would otherwise dangle: `COMP-P9-F2-T1-1` now protects `COMP-ENT-003`
(policy distribution/enforcement), and `COMP-P9-F2-T4-1` now protects
`COMP-ENT-006` (packaged Stage-5 enterprise qualification). All other retained
fields remain byte-decoded equal to baseline; validation covers both
`depends_on` and `protected_product_ids` referential integrity.

## S0-01w training/telemetry scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-20` from baseline `48ec816` is replaced by
ten finite product outcomes `COMP-TRAIN-001..010`. The rows cover the required
S0/S0-02 compatibility matrix; separate crash/product/raw-training consent;
preview, metadata-first redaction, provenance, spool/upload and revocation;
export, deletion, retention and tombstones; packaged Manual/offline no-egress
regardless of stored consent; the S5-12 candidate-feedback workflow; consented corpus export and trainer-boundary checks; reproducible adapter
training with held-out and Legion-Bench comparisons; and packaged Stage-5
qualification through external effect oracles.

The approved product-completion design supplies the family promise and finite
matrix rule. The AI-team plan supplies S5-12/S5-13 workflows, dependencies,
failure cases, and provenance requirements. The production-qualification plan
supplies the EvidenceRun and OS-level no-egress/deletion boundaries. Current
source and retained evidence are classified conservatively: consent service,
spool/upload, retention, corpus export, and training primitives are `partial`
where traces exist; finite matrices, packaged Stage-5 qualification, and the
missing end-to-end consent UI are `absent`. Every new row remains
`acceptance: unassessed`; substrate or historical evidence is not treated as
product acceptance.

The new rows preserve canonical ownership instead of duplicating it. Every
S5-12/S5-13 row depends on `COMP-TRAIN-001`, making the S0/S0-02 matrix an
explicit prerequisite. Their dependencies retain `COMP-ENT-003`/`COMP-ENT-004` for enterprise policy and
retention, `COMP-CTX-005` for provenance, `COMP-TRUST-004`/`COMP-TRUST-005`
for audit and egress, `COMP-P4-F3-T4-1` for metadata-only telemetry,
`COMP-P8-F3-T3-1` for metadata-only export, and `COMP-P9-F4-T1-1..T3-1` for
the retained training-flywheel outcomes. `COMP-TRAIN-006` adds the packaged
Manual/offline network-capture requirement without changing the existing
Manual owner. No retained row referenced the removed family placeholder, so no
authorized rewiring was required.

The exact UTF-8 Git blob at `48ec816` was compared object-by-object: all 391
non-family objects are unchanged, and the register is now 401 rows. Validation
found unique IDs, zero dangling `depends_on` or `protected_product_ids` edges,
an acyclic graph, and existing literal source paths for every new row. Existing
directory-valued historical evidence references remain unchanged as retained
baseline data; the new inventory uses literal files only.

## S0-01x platform quality scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-21` from baseline `f4630b4` is replaced by
nine finite product outcomes `COMP-PLAT-001..009`. They cover the required
S0/S0-02 native OS/version/architecture/reference-hardware/accessibility matrix;
packaged native input and IME/clipboard behavior; UIA/AX/AT-SPI accessibility
and keyboard focus; window/layout/DPI/multi-monitor restore; startup and
renderer responsiveness; calibrated performance and memory/resource bounds;
platform adapters for dialogs/keyring/PTY/watchers/menus; and packaged native
qualification with external oracles.

The completion design governs the family and finite matrix. XQ-02 governs
packaged native input/accessibility EvidenceRuns and blocked outcomes. XQ-03
governs calibrated per-OS renderer/product workloads and sustained use. Current
desktop source, tests, and historical evidence support only conservative
`partial` classifications; the matrix and complete packaged qualification are
`absent`. No headless fixture or historical three-OS record promotes current
product acceptance, and native GUI was not run because macOS/Linux access is
unavailable locally.

The new rows preserve canonical ownership through dependencies. Text semantics
remain `COMP-EDIT-*`; workbench/layout and persistence remain `COMP-WB-*` and
`COMP-PRES-*`; terminal parity remains `COMP-TERM-014`; existing performance
and accessibility contracts remain `COMP-P8-F4-T1-1..T3-1` and
`COMP-P8-F5-T1-1..T3-1`; distribution/operations remain family22/XQ-owned.
No existing row referenced the removed family21 placeholder, so no authorized
rewiring was required. All new rows use `owner_role: luna_worker` and
`acceptance: unassessed`.

The exact UTF-8 Git blob at `f4630b4` was compared object-by-object: all 400
non-family objects are unchanged, and the register is now 409 rows. Validation
found unique IDs, zero dangling `depends_on` or `protected_product_ids` edges,
an acyclic graph, and existing literal source paths for every new row.

## S0-01y distribution/operations scope expansion (2026-09-05)

The aggregate `COMP-SCOPE-FAMILY-22` from baseline `4452c97` is replaced by
ten finite product outcomes `COMP-DIST-001..010`. They cover the required
S0/S0-02 distribution matrix; reproducible artifacts and provenance; production
signing and trust verification; a separate Manual/offline artifact with
OS-level zero-egress; stable/preview descriptors and feeds; update replacement,
restart acknowledgement, interruption, and rollback; clean-machine install and
repair; opt-in crash/support diagnostics; documentation; and final distribution
qualification with immutable hashes and external oracles.

The approved completion design, production-qualification XQ-01/XQ-04/XQ-05/XQ-06/XQ-07,
master release plan WS-REL-01, operator runbook, and current release/update,
signing, diagnostics, packaging, and evidence files define this finite scope.
Current evidence is classified conservatively: release and update paths are
`partial` because they are dry-run or unsigned-beta paths; production signer,
clean-machine/native release evidence, the finite matrix, and final qualification
are `absent`. The register does not claim live credentials, native three-OS
proof, or acceptance; every new row remains `acceptance: unassessed`.

Canonical ownership is retained through dependencies: platform qualification is
`COMP-PLAT-009`, updater behavior remains the P8-F2 outcomes, crash/support
controls remain P8-F3, and the distribution rows consume those outcomes without
duplicating them. The seven authorized protected-field rewires are exact:
`COMP-P0-F4-T3-1` -> `COMP-DIST-002`; `COMP-P0-F4-T4-1` ->
`COMP-DIST-002`; `COMP-P8-F1-T1-1` -> `COMP-DIST-003`;
`COMP-P8-F1-T2-1` -> `COMP-DIST-002,COMP-DIST-003`;
`COMP-P8-F1-T3-1` -> `COMP-DIST-007`; `COMP-P8-F1-T4-1` ->
`COMP-DIST-003,COMP-DIST-007`; and `COMP-P8-F1-T5-1` ->
`COMP-DIST-003,COMP-DIST-005,COMP-DIST-007`. Existing `COMP-SCOPE-GAP-02`
protection is retained on the rows that had it. These mappings preserve build,
artifact verification, signer policy, descriptor integrity, fresh-machine, and
external signing/feed coverage without removing protection.

The exact UTF-8 Git blob at `4452c97` was compared object-by-object: 408
non-family objects remain, with only the seven authorized
`protected_product_ids` arrays changed; every other field is byte-decoded equal
to baseline. The register is now 418 rows. Validation found unique IDs, no
remaining family22 references, zero dangling `depends_on` or
`protected_product_ids` edges, an acyclic combined graph, and existing literal
source paths for every new row.

## S0-01z GAP refinement (2026-09-05)

The stable `COMP-SCOPE-GAP-01..10` identifiers remain the P0 installed-product
outcomes, but their former generic rows were not finite acceptance contracts.
They now have explicit cross-cutting titles, bounded source references, and
dependencies on the finite product rows that provide their prerequisites. The
dependencies deliberately do not point through an internal row that already
protects the GAP, so the combined dependency/protection graph remains acyclic.

One finite row was added: `COMP-GAP-001` covers the missing packaged native GUI
install/edit/save journey, real window input, external disk effects, blocked
native configurations, the four-green promotion clock, and immutable
installed-preview journal. It is `stage: S1`, `package_id: XQ-02`,
`implementation: absent`, `owner_role: luna_worker`, and
`acceptance: unassessed`. The source boundary is P0 GAP-01.1..1.3, XQ-02,
`windowed_e2e.rs`, native-package verification, the T0-D promotion criteria,
and the installed-preview journal.

The retained GAP outcomes now aggregate finite coverage for signing/trust,
update/rollback, work preservation, accessibility, Manual/offline zero-egress,
governance, documentation truth, performance, and support/privacy/legal
distribution. GAP-10 explicitly retains Help/About support-bundle behavior and
legal/package notices as acceptance boundaries; existing diagnostics and notice
rows are prerequisites, not fabricated proof of the desktop path.

The exact protected fields were preserved except for the authorized
`COMP-P0-F4-T1-1` rewire. GAP-01 remains protected by `COMP-P0-F4-T2-1`, GAP-02
by the existing P0/P8 signing rows and `COMP-SCOPE-FAMILY-17-04`, and internal
GAP-07/GAP-08 protect their explicit concrete product targets; future
replacement requires explicit source-mapped rewiring. Baseline `65381ed` had 418 rows; the
current register has 419, with all non-GAP objects unchanged and only the ten
GAP objects plus the new finite row changed. Validation found unique IDs,
resolved references, existing literal source paths, and an acyclic combined
graph.

## S0-01z governance classification correction (2026-09-05)

Traceability source line 24 treats governance and documentation obligations as
internal implementation records. Stable `COMP-SCOPE-GAP-07` and
`COMP-SCOPE-GAP-08` are therefore `kind: internal`, while retaining required,
`luna_worker`, and `acceptance: unassessed`. GAP-07 protects installed-product
truth and signing/trust (`GAP-01`, `GAP-02`). GAP-08 protects distribution
support/docs, installed-product truth, and signing/trust (`DIST-009`, `GAP-01`,
`GAP-02`).

The retained `COMP-P0-F4-T1-1` protection was explicitly rewired from internal
GAP-08 to `COMP-DIST-009`, `COMP-SCOPE-GAP-01`, and `COMP-SCOPE-GAP-02`.
Dependencies do not point through protected edges, and combined validation
remains acyclic; all other non-GAP objects remain exact.

## Owner-blocked prerequisites (2026-09-08, round r01)

This section is the canonical list of prerequisites only the owner can supply. A
blocker never promotes or demotes an `acceptance` value by itself; it records
exactly what is missing. An unavailable host, tool, or credential is recorded
here as blocked with an exact prerequisite string, never as passed and never as
skipped. A result from an available host is never substituted for a row that
names an unavailable one.

Standing authority as of this date: native GUI automation is resumed on this
Windows host by owner instruction. macOS and Linux hosts remain unavailable and
their rows stay blocked.

### BLK-2026-09-08-01 — Unix bounded-process stdin branch

Prerequisite, exact: a Linux or macOS host with this workspace checked out and a
Rust toolchain able to run `cargo test -p legion-platform --test bounded_process`.

Needed to assess the Unix nonblocking `fcntl` stdin branch of packet
`s2-03a-bounded-stdin`. That branch is compiled out on this Windows host, so it
stays unassessed. It is never passed by substitution from the Windows
anonymous-pipe `PIPE_NOWAIT` branch, which exercises different code. The packet
itself was rejected at review in round r01 and reverted; the bounded stdin
specification `docs/superpowers/specs/2026-09-08-bounded-process-stdin.md`
remains planning only.

### BLK-2026-09-08-02 — macOS and Linux packaged native GUI rows

Prerequisite, exact: a macOS host and a Linux host with the packaged native
product installed and a real display session, able to run the windowed GUI e2e
suite.

Needed for every macOS and Linux native-GUI and packaged-journey row:
`COMP-LANG-013`, `COMP-BTD-009`, `COMP-SCM-010`, `COMP-PRES-011`, and the
packaged-native rows of packages `S3-07` and `S4-06`. Those rows stay blocked.
Native GUI automation is resumed on the Windows host only, so a Windows result
never stands in for a macOS or Linux row, and no packaged, cross-OS, or release
readiness may be claimed from Windows evidence.

### Register ratification status on this date

`plans/completion/requirements.json` currently carries no `owner_approval_ref`
values at all, so there is no ratified cell in the register on this date,
provisional or otherwise. Should any be added, they must point at a dated entry
in this file stating that the ratification is provisional and agent-made, and
release mode must reject provisional cells. All 419 rows remain
`acceptance: unassessed`.

### Round r01 register effect

Round r01 applied one reviewer-proposed implementation transition,
`COMP-LANG-007` to `partial`, for the passed pure-move packet
`s2-03b-language-extract`. The row already read `partial`, so the file is
byte-identical before and after: 419 rows, 143 implemented / 233 partial / 43
absent, all 419 `acceptance` values `unassessed`. The round's fifteen green logs
are component or crate-level integrated evidence and are recorded as such in
`plans/evidence/full-product-resume-2026-09-08/README.md`; none of them is
packaged evidence, native GUI evidence, or product acceptance, and no acceptance
value was set from any of them.

### Round r05 additions (2026-09-08)

Five prerequisites were added in round r05. `BLK-2026-09-08-01` and
`BLK-2026-09-08-02` above are unchanged and are not re-numbered. The standing
authority is unchanged: native GUI automation is resumed on this Windows host by
owner instruction; macOS and Linux hosts remain unavailable and their rows stay
blocked.

#### BLK-2026-09-08-03 — external native input driver

Prerequisite, exact: a Windows 11 x64 host with an interactive logged-in desktop
session and the external native input driver installed at
`tools/native-input-driver/legion-input-driver.exe`, able to inject OS-level
keyboard, pointer, text, clipboard and IME/CJK input into another process and to
read that process's UI Automation text and clipboard state from outside it.

That string is quoted from `PREREQUISITE_DRIVER_MISSING` at
`xtask/src/native_product_acceptance.rs:75-82` with its `concat!` parts joined.
It is source text, not a harness observation: nothing reports that the harness
emitted it, because the harness never ran.

`tools/` does not exist in this worktree at all. Discovery gates before the
harness's own session handshake, so `run_native_product_acceptance` never
reached the `--probe-session` step that answers the interactive-session question
by reading `interactive_session = true` out of `driver_session.toml`. No stub
driver was written and no `run.json` was produced. Affects packet
`s1-02b-native-acceptance-run`.

#### BLK-2026-09-08-04 — packaged native product on this Windows host

Prerequisite, exact: a Windows 11 x64 host with the packaged native Legion
product installed into the package directory this command was given, containing
the product executable, so the harness can launch the packaged product as a
subprocess rather than a development build.

`target/native-input-acceptance/package/legion-desktop.exe` is absent.
`target/debug/legion-desktop.exe` exists and was deliberately not copied there:
ADR-0056's subject is an owner-installed package the harness must not build, and
staging a development build in the package directory would make a run blocked on
a missing driver look as though it had a product to drive. Affects packet
`s1-02b-native-acceptance-run`.

#### BLK-2026-09-08-05 — native Node runtime and retained TypeScript fixtures

Prerequisite, exact: a native Node runtime at or above 22.22.2 approved on this
host plus the retained TypeScript fixtures, so that
`cargo test -p legion-app --test typescript_app_startup -- --ignored` can
execute.

All six tests in `crates/legion-app/tests/typescript_app_startup.rs` carry
`#[ignore = "opt-in native Node + retained TypeScript fixtures"]`, and the four
in `python_app_startup.rs` are likewise ignored. `#[ignore]` is a compile-time
attribute; both targets compiled and executed nothing in
`plans/evidence/full-product-resume-2026-09-08/round-r05-test-s2-01b-blast-radius-app.log`.
The TypeScript descriptor pinned by packet `s2-01b-typescript-registry-pin` has
therefore launched no server, and the TypeScript/JavaScript live workflow has no
execution evidence on this host. This is an absence of evidence, not a pass and
not a skip. Affects `COMP-LANG-001`, `COMP-LANG-002` and `COMP-LANG-009`, whose
`implementation` values stay `partial` and whose `acceptance` values stay
`unassessed`.

#### BLK-2026-09-08-06 — owner-selected TypeScript and JavaScript releases

Prerequisite, exact: a real TypeScript and JavaScript language-server, browser
and debug-adapter setup with owner-selected releases.

The tier-two registry can now name a pinned artifact, but which releases are
approved is an owner selection, not an agent one. Until the owner selects them,
the TypeScript and JavaScript live workflow, browser session and debug-adapter
rows have no configuration to run against. Affects packet
`s2-01b-typescript-registry-pin`.

#### BLK-2026-09-08-07 — approved tailwindcss-language-server artifact

Prerequisite, exact: an approved `tailwindcss-language-server` release artifact
with a published version and SHA-256, so registry entry 103 can be pinned
instead of resolving by bare name from PATH.

Entry 103 was left resolving by bare name because no approved release artifact
with a published version and digest exists to pin it to. Inventing a version or
a digest would be a fabricated pin, so the entry stays unpinned and visibly so.
Affects packet `s2-01b-typescript-registry-pin`.

### Round r05 register effect

Round r05 applied three reviewer-proposed implementation transitions for the one
passed packet `s2-01b-typescript-registry-pin`: `COMP-LANG-001`, `COMP-LANG-002`
and `COMP-LANG-009` to `partial`. All three rows already read `partial`, so
`plans/completion/requirements.json` is byte-identical before and after: 419
rows, 143 implemented / 233 partial / 43 absent, all 419 `acceptance` values
`unassessed`. No `owner_approval_ref` value exists anywhere in the register, so
there is still no ratified cell on this date, provisional or otherwise.

The round's nineteen logs are recorded in
`plans/evidence/full-product-resume-2026-09-08/README.md` with one line each
giving the exact command, the exit code and the classification. Eighteen exit 0.
The one non-zero is `round-r05-test-s0-05c-verify-completion-register.log` at
exit 1, which belongs to the rejected and reverted packet `s0-05c-residue-coverage`
and describes a working tree that no longer exists; the committed tree's last
measured value remains the 78 structural issues of round r04. The whole set is
component evidence with four crate-level integrated test targets. None of it is
packaged evidence, native GUI evidence, or product acceptance, and no
`acceptance` value was set from any of it.

`plans/completion/dependencies.json` still describes registry entry 104 as
pointing at `registry.example.invalid`. That has been stale since packet
`s2-01a`, was correctly reported by this round's reviewer and was not fixed. It
is record-owned and is carried forward as an open repair, not as a resolved item.

## 2026-09-08 provisional register ratification

### What this section ratifies, and what it does not

On 2026-09-08, in round r02 of the full-product-resume workspace, an agent
authored the four canonical register files that had never existed:
`plans/completion/matrix.json`, `plans/completion/scenarios.json`,
`plans/completion/dependencies.json` and `plans/completion/defects.json`.
Until they existed, `validate_register_structure` could not load the register at
all, so the register lane had no signal of any kind.

The ratification recorded here is **provisional and agent-made**. No owner has
approved any cell in `matrix.json`, any scenario in `scenarios.json`, any
milestone in `dependencies.json`, or either defect. Every configuration in
`matrix.json` carries the single `owner_approval_ref` string

```
plans/completion/decisions.md#2026-09-08-provisional-register-ratification (PROVISIONAL: agent-made, not owner-ratified)
```

verbatim, in all 42 rows, and that string points at this section. The literal
token `PROVISIONAL:` is the marker a release-mode check must look for: **release
mode must reject every cell carrying it.** A cell becomes owner-ratified only
when the owner replaces that string with an approval reference of their own, in
a separate step this packet does not perform.

Nothing written here is evidence. No `EvidenceRun` exists for anything in these
four files, no row's `implementation` value moved, and no row's `acceptance`
value moved.

### Unselected values are visible placeholders

Every `tool_versions` value beginning `PENDING:S0-02-` is an unselected
placeholder awaiting an S0-02 owner decision, not an observation and not a
support promise. The measured values on this laptop — Rust/cargo/rust-analyzer
1.97.1, Node 24.19.0, npm 11.17.0, bundled Python 3.12.14, Windows 11
`10.0.26200`, Git 2.55.0, PowerShell 7.6.5, Ollama 0.33.3 — were deliberately
**not** promoted into any target pin. Availability on one developer laptop is
not an approved support commitment, and the schema draft rationale says so
explicitly. The one real version label kept from the plan and workflow material
is macOS 15, which appears as `macOS-15 (plan label; exact patch pending)`
because the exact patch level is still unresolved.

The owner must still supply, before any of these cells can stop being
provisional: exact Windows and Linux OS versions and reference hardware; the
exact macOS patch level; Rust, rust-analyzer, TypeScript, Node, language-server,
browser, Python, test-runner, build-backend and debug-adapter releases;
representative project repositories with commits or tags and a lockfile and
network policy; package formats per OS; the accessibility provider session
configuration per OS; the Manual/offline artifact flavor and the OS-level
per-process network-capture tool per OS; and the signer, notarizer and trust
verifier per OS. No credential, certificate, key, keychain entry or secret name
appears in any cell, and none may be added to one.

### Matrix shape: 42 configurations

- 32 language/project tuples, copied verbatim from the S0-05 schema draft: four
  OS/architecture targets (Windows x64, macOS x64, macOS arm64, native Linux
  x64) crossed with the eight required project categories. Only
  `owner_approval_ref` was changed.
- 4 `packaged-product-journey` cells, one per OS/architecture target, from the
  proposal's platform cells `OS-WIN11-X64-REF1`, `OS-MAC15-X64-REF1`,
  `OS-MAC15-ARM-REF1` and `OS-LINUX-UBUNTU-X64-REF1`. These exist because the
  accessibility, workbench and Manual journey scenarios run against an OS cell,
  not against a language tuple.
- 3 `manual-offline-artifact` cells from `DIST-MANUAL-OFFLINE-{WIN,MAC,LINUX}`.
- 3 `signed-stable-artifact` cells from `DIST-STABLE-SIGNED-{WIN,MAC,LINUX}`.

**macOS arm64 for the distribution cells.** The Manual/offline and signed-stable
cells use macOS arm64 rather than macOS x64 because the release workflow's
primary macOS runner label is `macos-15` (arm64) and `macos-15-intel` is the
secondary. Both macOS architectures remain present in the language tuples and in
the packaged-product-journey cells; only the two distribution families are
narrowed to the primary architecture, and widening them is an owner decision.

`hardware` is non-blank on every row because the validator rejects a blank one.
For macOS and Linux it names *target reference hardware pending owner
selection*, and for the signed-stable cells it says explicitly that no clean
machine is available in this workspace. No row is worded as though a macOS host,
a Linux host, a clean VM or a signing credential existed.

### Scenario shape: 37 scenarios and their requirement selection

`scenarios.json` is exactly the native scenario catalog of
`matrix-contract-draft.md`, with the brace suffixes expanded: 6 `SC-MANUAL-*`,
16 `SC-LANG-*` (four families across Rust, TypeScript, JavaScript, Python), 3
`SC-PLATFORM-A11Y-*`, 3 `SC-MANUAL-OFFLINE-30M-*` and 9 `SC-RELEASE-*`.

Configuration binding: the six `SC-MANUAL-*` scenarios use all four
packaged-product-journey cells; `SC-PLATFORM-A11Y-*` uses the
packaged-product-journey cell(s) for its OS (both macOS cells for `-MAC`);
`SC-MANUAL-OFFLINE-30M-*` uses the Manual/offline cell for its OS;
`SC-RELEASE-*` uses the signed-stable cell for its OS; and each `SC-LANG-*`
family uses the language tuples matching its language across all four
OS/architecture targets.

Each scenario carries at most six `requirement_ids`, chosen because the row's own
title text names the outcome the scenario exercises. **Coverage of the 419 rows
is explicitly not attempted here**, and no scenario was widened to raise a
coverage number. One line per scenario:

- `SC-MANUAL-OPEN-TYPE-SAVE` — PRES-001/002 are the save and Save-As/Save-All
  write path, PRES-003 is the restart-with-dirty-buffer recovery, PRES-008 is
  the external-overwrite conflict, WB-001 is the dirty tab and dirty-close
  prompt, WB-007 is the palette save command the run falls back to.
- `SC-MANUAL-WORKBENCH-RESTORE` — WB-002 splits and tab groups, WB-003 dock
  geometry persistence and the corrupt-layout path, WB-005 restart restoration,
  WB-001 tab order and dirty markers, CAN-001 the Canvas viewport that the
  arrangement includes, PRES-003 the dirty buffer carried across the restart.
- `SC-MANUAL-EDIT-INPUT-RECOVERY` — PLAT-002 is the packaged native input path
  itself and is the row both native defects sit on; EDIT-006 clipboard, EDIT-007
  IME commit after focus change, EDIT-008 one authoritative route per input
  kind, EDIT-010 Vim motions through the key feed, PRES-007 grouped undo/redo.
  The accessibility tree is used here only as an oracle; the accessibility
  outcome rows belong to `SC-PLATFORM-A11Y-*`.
- `SC-MANUAL-REFACTOR-SEARCH-REVIEW` — NAV-002 symbol navigation, NAV-007
  literal and regex workspace search, NAV-009 the complete reviewable preview,
  NAV-010 cancel without mutation, NAV-011 apply with preconditions and
  rollback, NAV-012 stale/out-of-workspace rejection.
- `SC-MANUAL-TERMINAL-TUI` — TERM-001 command execution from the workspace root,
  TERM-006 interactive TUI control keys and resize, TERM-008 task exit code and
  duration metadata, TERM-010 cancellation, timeout, kill escalation and orphan
  cleanup, TERM-015 the child PID / launch time / exit status evidence contract,
  PRES-012 the editor buffer surviving terminal loss.
- `SC-MANUAL-GIT-HISTORY-RECOVERY` — SCM-002 stage/unstage including a partial
  hunk, SCM-003 commit, SCM-004 branch workflows, SCM-005 merge conflict and
  resolution, SCM-009 external-change and restart recovery, PRES-006 local
  history restore through proposal authority.
- `SC-LANG-LSP-LIFECYCLE-RUST` — LANG-002 provisioning and provenance, LANG-003
  toolchain and workspace discovery, LANG-004 the lifecycle and its failure
  modes, LANG-005 completion/diagnostics/hover, LANG-006 definition and
  references, LANG-008 the real rust-analyzer process.
- `SC-LANG-LSP-LIFECYCLE-TS` — same five lifecycle rows plus LANG-009, the row
  that names the real TypeScript/JavaScript servers.
- `SC-LANG-LSP-LIFECYCLE-JS` — same five lifecycle rows plus LANG-009, which
  covers the JavaScript half of the same outcome.
- `SC-LANG-LSP-LIFECYCLE-PY` — same five lifecycle rows plus LANG-010, the
  Python server and isolated environment row.
- `SC-LANG-REFACTOR-RUST` — LANG-007 is the reviewable rename/format/code-action
  proposal outcome, LANG-006 the cross-file navigation the rename depends on,
  LANG-005 the post-edit diagnostics, LANG-008 the Rust server.
- `SC-LANG-REFACTOR-TS` — LANG-007, LANG-006, LANG-005 and LANG-009.
- `SC-LANG-REFACTOR-JS` — LANG-007, LANG-006, LANG-005 and LANG-009.
- `SC-LANG-REFACTOR-PY` — LANG-007, LANG-006, LANG-005 and LANG-010.
- `SC-LANG-BUILD-TEST-RUST` — LANG-011 build/test discovery and execution,
  BTD-002 prerequisite validation, BTD-003 run/stop/rerun with real exit status,
  BTD-004 targeted and grouped tests with genuine results, BTD-005 the
  cancellation, missing-runner and stale-result recovery states.
- `SC-LANG-BUILD-TEST-TS` — the same five rows; they are language-general by
  their own titles, which name all four languages.
- `SC-LANG-BUILD-TEST-JS` — the same five rows.
- `SC-LANG-BUILD-TEST-PY` — the same five rows.
- `SC-LANG-DEBUG-RUST` — LANG-012 adapter resolution and session control,
  BTD-006 adapter discovery and startup failure, BTD-007 breakpoints and
  stepping, BTD-008 frames, variables and console evaluation, PRES-013 the
  editor buffer surviving adapter loss.
- `SC-LANG-DEBUG-TS` — the same five rows.
- `SC-LANG-DEBUG-JS` — the same five rows.
- `SC-LANG-DEBUG-PY` — the same five rows.
- `SC-PLATFORM-A11Y-WIN` — PLAT-003 the accessibility tree and focus order,
  PLAT-009 the packaged platform qualification that externally checks
  accessibility, WB-004 keyboard reachability of every workbench surface,
  WB-009 focus and DPI restoration across the matrix, SCOPE-GAP-05 the native
  accessibility gap row.
- `SC-PLATFORM-A11Y-MAC` — the same five rows, bound to both macOS cells.
- `SC-PLATFORM-A11Y-LINUX` — the same five rows.
- `SC-MANUAL-OFFLINE-30M-WIN` — DIST-004 the separately labelled Manual/offline
  artifact with zero egress, TRAIN-006 the OS-level capture proving no egress
  regardless of stored consent, SCOPE-GAP-06 the Manual/offline zero-egress gap
  row. The editing, search and Git work inside the thirty minutes is the vehicle,
  not the outcome under test, so those rows are not claimed here.
- `SC-MANUAL-OFFLINE-30M-MAC` — the same three rows.
- `SC-MANUAL-OFFLINE-30M-LINUX` — the same three rows.
- `SC-RELEASE-CLEAN-INSTALL-WIN` — DIST-003 real signer and OS-verifier
  evidence, DIST-007 clean-machine install and trust qualification,
  SCOPE-GAP-02 the release signing and trust gap row.
- `SC-RELEASE-CLEAN-INSTALL-MAC` — the same three rows.
- `SC-RELEASE-CLEAN-INSTALL-LINUX` — the same three rows.
- `SC-RELEASE-UPDATE-ROLLBACK-WIN` — DIST-005 channel descriptors, hashes and
  feed provenance, DIST-006 the helper-driven atomic swap, restart
  acknowledgement and rollback, SCOPE-GAP-03 the signed update and rollback gap
  row.
- `SC-RELEASE-UPDATE-ROLLBACK-MAC` — the same three rows.
- `SC-RELEASE-UPDATE-ROLLBACK-LINUX` — the same three rows.
- `SC-RELEASE-CRASH-PRIVACY-WIN` — DIST-008 opt-in, metadata-only, redacted,
  exportable and deletable diagnostics with a deletion receipt, PRES-004 crash
  and restart recovery of dirty text, SCOPE-GAP-04 the packaged
  work-preservation gap row, SCOPE-GAP-10 the support, privacy and legal
  distribution gap row.
- `SC-RELEASE-CRASH-PRIVACY-MAC` — the same four rows.
- `SC-RELEASE-CRASH-PRIVACY-LINUX` — the same four rows.

Deliberately **not** bound in this packet: the S2-06 umbrella evidence rows
`COMP-LANG-013` and `COMP-BTD-009`, and the other packaged-journey umbrella rows
(`COMP-SCM-010`, `COMP-PRES-011`, `COMP-CAN-009`, `COMP-DIST-010`,
`COMP-GAP-001`). They describe the existence of an EvidenceRun per configuration
rather than a behaviour a single scenario exercises, and binding them would
inflate coverage without adding a checkable outcome. Their binding is left open.

Each `sensitive_artifact_policy` is written for its own scenario; the
crash/privacy, release and Manual/offline policies are strict about payload
inspection, capture filtering and sealed retention, while the editor and
workbench policies are ordinary. No single policy string is pasted across all
37 scenarios.

### Package register: 54 packages, 419 rows

`dependencies.json` contains exactly the 54 distinct `package_id` values present
in `requirements.json` — no invented aggregate owner — and their
`requirement_ids` partition all 419 rows exactly once, derived mechanically from
`requirements.json` rather than hand-typed.

**`owner_role` rule.** A package takes the `owner_role` carried by the most of
its own rows; a tie is broken in favour of `luna_worker`. Eight packages have
rows with more than one role: `S0-02`, `S3-01`, `S3-05`, `S3-06`, `S4-02`,
`S4-03`, `S4-04` and `S4-05`. Under the rule they resolve to `luna_worker`,
`luna_worker`, `luna_worker`, `luna_worker`, `sol_engineer`, `luna_worker`,
`sol_engineer` and `sol_engineer` respectively. No tie actually occurred, so the
tie-break was not exercised. This is routing metadata, not an assignment of work
to a person.

**`implementation_stage`.** For 52 packages every row declares the same stage and
that stage is used. `XQ-03` and `XQ-07` each have rows at both S0 and S1; the
declared value is `S0` for both, per the S0-01b section above ("`XQ-02` stage 1,
`XQ-03`/`XQ-07`/`XQ-08` stage 0, `XQ-04`/`XQ-05`/`XQ-06` stage 1"). The
divergence is recorded here rather than repaired: repairing the individual rows
would mean editing `requirements.json`, which this packet may not do.

**`acceptance_stage`.** No source plan declares an acceptance stage separate from
the implementation stage for any of the 54 packages. Every `acceptance_stage`
therefore repeats its package's `implementation_stage`. **This is a provisional
default, not a plan fact**, and it must be revisited when the owner ratifies the
stage model.

**`deliverable_refs`.** Each milestone cites the plan file and the heading that
defines the package, using the heading's GitHub anchor slug so the reference
resolves rather than merely naming the package, for example
`docs/superpowers/plans/2026-09-04-manual-language-completion.md#s1-04-finish-editing-semantics-input-methods-keymaps-and-settings`.
All 54 packages have such a heading: `S0-*` in the full-product-completion plan,
`S1-*` and `S2-*` in the manual-language plan, `S3-*`, `S4-*` and `S5-*` in the
AI/team plan, and `XQ-*` and `S6-01` in the production-qualification plan. Every
path and anchor was verified against the file before it was written. Each package
has `<pkg>:implemented` and `<pkg>:accepted`, with `accepted` depending on
`implemented` and no other edges, so the milestone graph is acyclic by
construction.

**`external_prerequisites`.** Only real holds are listed; a package with no
external prerequisite has an empty list, and none was padded. The two open
blockers are reused verbatim: `BLK-2026-09-08-01` is attached to `S1-06`, the
package that owns the terminal and bounded-process execution rows whose Unix
stdin branch it concerns; `BLK-2026-09-08-02` is attached to `S3-07` and `S4-06`,
which it names explicitly; to `S1-07`, `S1-08` and `S2-06`, the packages that own
the four requirement rows it names (`COMP-SCM-010` in `S1-07`, `COMP-PRES-011` in
`S1-08`, `COMP-LANG-013` and `COMP-BTD-009` in `S2-06`); to `XQ-02`, whose whole
task is ingesting native input and accessibility evidence on Windows, macOS and
Linux; and to `S6-01`, whose final journey spans all three OSes. The remaining
entries are the holds the
matrix-contract-draft lists under "Observed, declared, and missing inputs",
attached to the packages that need them: version and repository selection to
`S0-02` and `S2-01`; reference hardware and images to `S0-02`; the
TypeScript/JavaScript server, browser and debug setup, the usable Pyright
artifact and the Python environment to `S2-02` and `S2-05`; the debug-adapter
binaries to `S2-05`; the accessibility provider session to `XQ-02`; the signing,
notarization and feed infrastructure to `XQ-04`, `XQ-07` and `S6-01`; clean
machines to `XQ-07` and `S6-01`; the OS-level per-process network capture to
`XQ-06`; the external endpoints and credentials to `S3-02`, `S5-07`, `S5-09`,
`S5-11` and `S5-14`; and the independent external auditor and archived report to
`XQ-08`.

### Defects: two native Windows observations, and nothing invented

`defects.json` contains exactly the two observations from the native Windows
baseline session of 2026-09-05. Both sit on `COMP-PLAT-002`, the packaged native
input path row, on scenario `SC-MANUAL-EDIT-INPUT-RECOVERY` and configuration
`CFG-WIN11-X64-PRODUCT-JOURNEY`.

- **`DEF-2026-09-05-01` — Home and End have no editor mapping.** Severity `P1`,
  `invalidates_required_outcome: true`. P1 rather than P0 because the product
  still starts, still edits and still saves, and no data is lost or silently
  corrupted; P1 rather than P2 because line-start and line-end motion is part of
  the ordinary keyboard contract every editor user relies on many times an hour,
  and `COMP-PLAT-002` is a required row that cannot be accepted while a standard
  key produces no effect at all. Status is `fixed-awaiting-verification`:
  `crates/legion-desktop/src/workflow.rs` now maps `Home`/`End` to
  `EditorBoundaryKind::LineStart`/`LineEnd` and, with the command modifier, to
  `DocumentStart`/`DocumentEnd`, but **no native run has re-observed the key
  since that change**, so `verification_run_ids` is empty and the defect is not
  closed. Closing it requires a native EvidenceRun; the validator only demands
  verification runs for `closed`, so this honesty is a discipline, not a
  mechanism.
- **`DEF-2026-09-05-02` — native Ctrl+S did not persist.** Severity `P1`,
  `invalidates_required_outcome: true`. P1 rather than P0 because the dirty
  indicator stayed set, so the user was told the truth about the unsaved state,
  a working save route exists through the command palette, and no data was lost;
  P1 rather than P2 because the platform save chord is the single most-used
  command in an editor and `COMP-PLAT-002` cannot be accepted while it does
  nothing. Status is `open`. The cause is **not** established: the observed field
  records the leading explanation — a frame-level `egui::Modifiers.command`
  mismatch for an injected `Control_L+s` — explicitly as
  `HYPOTHESIS, UNCONFIRMED`, because no native event log capturing the actual
  `egui::InputState.modifiers` or `Event::Key` fields for that chord exists. The
  automation-tool and desktop-focus interference events observed in the same
  session are deliberately excluded and are not recorded as product behaviour.

`repair_package_id` is `S1-04` for the first (editing semantics, input methods
and keymaps) and `S1-02` for the second (the native input acceptance boundary).
`owner` names a role, `luna_worker`, not a person.

### What this packet did not touch, and what remains open

`plans/completion/requirements.json` was not read for values to change, not
written and not staged; it is byte-identical to HEAD. `plans/completion/candidate.json`
was not created; nominating a candidate is a separate, owner-authorised step.
No `acceptance` value moved anywhere: all 419 rows remain `unassessed`.

Binding the 419 rows back to scenarios, configurations and defects — filling
`scenario_ids`, `configuration_ids` and `defect_ids` in `requirements.json` —
remains open and is owned by the record role. Because those arrays are all still
empty, `validate_register_structure` is expected to exit nonzero on four issue
classes attributable to that missing binding: required rows with empty coverage
(419), scenario-to-requirement links that are not yet bidirectional (170),
requirement/scenario/configuration triples not yet linked by the requirement
(923), and the two defect links that are not yet bidirectional (2). That nonzero
exit is the correct, truthful result for this repository today; a clean run would
have meant scenarios or defects had been omitted, or links invented. Any further
issue in those four classes is a defect in these four files.

A fifth group of 65 issues is expected as well, and none of it is reachable from
the four files this packet owns. It comes entirely from `requirements.json` and
the workspace paths it points at: 34 `source_refs` entries that name a directory
rather than a file (`plans/evidence/production/M9/`, `crates/legion-app/tests`,
`crates/legion-desktop/tests`); 26 product rows that still carry
`protected_product_ids`, plus one whose protected target is not a product row;
three `legacy_ids` values (`COMP-P4-F1-T1-1`, `COMP-P4-F1-T2-1`,
`COMP-P4-F1-T3-1`) that are requirement ids rather than Kanban task ids; and
`COMP-DIST-010`, a product row whose `package_id` is `S6-01`. These are recorded
here as observations for the record role. They are not repaired by this packet,
they say nothing about the four new files, and they must not be counted against
them.

## Owner-blocked prerequisites — round r06 additions and changes (2026-09-08)

Six owner-blocked prerequisites were surfaced in round r06 and **all six are
restatements of prerequisites already recorded above**. No new prerequisite was
found, none was invented, and no `BLK-2026-09-08-08` exists:

- `BLK-2026-09-08-02` — a macOS host and a Linux host with the packaged native
  product installed and a real display session, able to run the windowed GUI
  e2e suite.
- `BLK-2026-09-08-04` — a Windows 11 x64 host with the packaged native Legion
  product installed into the package directory this command was given,
  containing the product executable, so the harness can launch the packaged
  product as a subprocess rather than a development build.
- `BLK-2026-09-08-01` — a Linux or macOS host with this workspace checked out
  and a Rust toolchain able to run
  `cargo test -p legion-platform --test bounded_process`.
- `BLK-2026-09-08-05` — a native Node runtime at or above 22.22.2 approved on
  this host plus the retained TypeScript fixtures, so that
  `cargo test -p legion-app --test typescript_app_startup -- --ignored` can
  execute.
- `BLK-2026-09-08-06` — a real TypeScript and JavaScript language-server,
  browser and debug-adapter setup with owner-selected releases.
- `BLK-2026-09-08-07` — an approved `tailwindcss-language-server` release
  artifact with a published version and SHA-256, so registry entry 103 can be
  pinned instead of resolving by bare name from PATH.

The standing authority is unchanged: native GUI automation is resumed on this
Windows host by owner instruction; macOS and Linux hosts remain unavailable and
their rows stay blocked, and a Windows result never substitutes for a macOS or
Linux row.

### BLK-2026-09-08-03 is retired by construction

`BLK-2026-09-08-03` asked the owner to install an external native input driver
at `tools/native-input-driver/legion-input-driver.exe`. Round r06's packet
`s1-02c-native-input-driver` **builds that driver from source in this
repository**, as the workspace crate
[`crates/legion-input-driver`](../../crates/legion-input-driver/src/main.rs).
`PREREQUISITE_DRIVER_MISSING` at
`xtask/src/native_product_acceptance.rs:80-90` now names
`cargo build -p legion-input-driver --release` instead of an installation, and
the retained staged path survives only as the last of three discovery
candidates (`target/release`, then `target/debug`, then
`tools/native-input-driver/`), so an owner-staged binary still works. The
retirement is recorded in the ADR-0056 amendment dated 2026-09-08 and in
`blockers.json`, where the blocker's `status` is now `retired` and its original
prerequisite string is left unedited as the true record of what the harness said
on its own date.

**The r05 entry for `BLK-2026-09-08-03` earlier in this file is not rewritten.**
It recorded the state truthfully on 2026-09-08 in round r05, including the exact
string the harness carried then. This section supersedes it; it does not correct
it. The same applies to
`plans/evidence/completion/native-manual-open-type-save-r05-win11-x64/host-observations.md`,
which still quotes the old prerequisite and is left as the record of what was
observed when it was written.

**What this retirement does not do.** It produces no acceptance evidence and it
unblocks no run. `xtask native-product-acceptance` was not invoked at all in
round r06: no product window was opened, no OS-level input was injected into any
process, and no `run.json` was written. On this host
`target/debug/legion-input-driver.exe` exists only as a side effect of the cargo
lane's own test builds, no `target/release/legion-input-driver.exe` exists, and
`target/native-input-acceptance/package/` does not exist.
**`BLK-2026-09-08-04` is not retired and still blocks every run.**
`COMP-PLAT-002` stays `implementation: partial`, `acceptance: unassessed`.

### A standing constraint on the first native acceptance artifact

Before any `xtask native-product-acceptance` artifact is transcribed into
`plans/completion/defects.json` or into any requirement row, this must be
settled first. The harness currently writes `status = "conformance-failed"`,
`exit_code = 1` for **any** post-driver outcome that is not
`driver_code == 0 && window_created && 6/6 conforms` — including the driver's own
exit `3`/blocked. The driver as built exits 3/blocked on this host by design,
because `observe_ime_cjk` blocks whenever the product window's keyboard layout
is not CJK. The first real run would therefore produce an artifact blaming the
product for a missing IME, which is exactly what the harness's own
`missing_packaged_binary_is_reported_blocked_rather_than_failed` test calls
filing a false defect. The mapping is pre-existing and was not introduced by
round r06. **Until a follow-up propagates the driver's blocked exit into a
blocked harness status, no `conformance-failed` artifact from this command may
be entered anywhere in the register.**

### Round r06 register effect

Round r06 applied fifteen reviewer-proposed implementation transitions across
three passed packets: `COMP-PLAT-002` to `partial` and `COMP-P1-F1-T1-1` to
`implemented` for `s1-02c-native-input-driver`; `COMP-DIST-010`,
`COMP-PLAT-001` and `COMP-DIST-001` to `absent` and `COMP-P1-F1-T4-1`,
`COMP-P1-F3-T1-1`, `COMP-P1-F4-T1-1`, `COMP-P6-F4-T1-1`, `COMP-P7-F1-T1-1`,
`COMP-P8-F2-T1-1` and `COMP-P9-F3-T1-1` to `implemented` for
`s0-05d-register-kind-repair`; and `COMP-LANG-001`, `COMP-LANG-002` and
`COMP-LANG-009` to `partial` for `s2-01c-pinned-archive-extract`. **All fifteen
were verified no-ops** — every row already held the value its reviewer proposed.
`plans/completion/requirements.json` is byte-identical across the record step:
419 rows, 143 implemented / 233 partial / 43 absent, all 419 `acceptance` values
`unassessed`.

That is now three rounds — r01, r05 and r06 — in which every proposed transition
was a no-op. The `implementation` column has not been exercised by the
transition mechanism at all, and a future round should expect its first real
transition to surface disagreements these no-ops have hidden.

The file *is* modified against the previous commit, by
`s0-05d-register-kind-repair`: `kind` on 32 rows, `scenario_ids` and
`configuration_ids` on 75 rows, and `protected_product_ids` on 32 rows. The
`implementation` and `acceptance` columns are untouched on all 419 rows against
the previous commit as well as across the record step. No `owner_approval_ref`
value exists anywhere in the register, so there is still no ratified cell on this
date, provisional or otherwise.

The brief authorised 34 re-kindings; 32 were applied, `COMP-P1-F1-T3-1` was held
back and `COMP-DIST-010` was declined — 32 + 1 + 1 = 34, reconciled against the
file. Seven re-kinded rows sit at `implementation: absent` (`COMP-PLAT-001`,
`COMP-DIST-001`, `COMP-SCOPE-FAMILY-15-01`, `COMP-SCOPE-FAMILY-17-01`,
`COMP-REMOTE-001`, `COMP-COLLAB-001`, `COMP-TRAIN-001`), and two more
(`COMP-LANG-001`, `COMP-BTD-001`) at `partial` and stage S2 rather than Stage 0.

`plans/completion/dependencies.json` entry 104 no longer names the deliberate
placeholder host `registry.example.invalid`; it now names the pinned Pyright
1.1.400 archive, its `policy://lsp-download/pyright` gate and its Node 14.0.0
minimum. That closes the stale-register item carried forward from round r05.
Two further `registry.example.invalid` assertions remain and are **not** fixed:
`plans/completion/language-tooling-scope-audit.md:26` and `:60`. They are
carried forward as an open repair.

### The register's structural-issue count was not measured this round

No `verify-completion-register` run happened in round r06. The
`s0-05d-register-kind-repair` report predicts 3 remaining structural issues after
its edits, and a reviewer's independently written port of
`validate_register_structure` reproduced 78-at-HEAD and 3-on-tree, but neither is
an exit code from the validator itself and **neither is recorded here as a
measurement**. The last figure measured on a tree that exists remains the **78
structural issues of round r04**. Closing it needs exactly
`cargo run -p xtask -- verify-completion-register --root .` run from the
workspace root by the cargo lane and logged with its own frame.

One pre-existing register edge holds one of the three predicted remaining issues
open and its repair is an open record-role decision, deliberately **not** made
this round: `COMP-P0-F3-T1-1`, `COMP-P0-F3-T2-1` and `COMP-P0-F3-T3-1` — the
Kanban backlog and its validator — all name `COMP-P1-F1-T3-1` ("Each
representative workflow has at least one headless test driving it through the
harness") in `protected_product_ids`. Choosing which of those four rows is
misclassified is a register judgement that wants the measured validator output in
front of it, which this round does not have. No row was accepted, no exemption
was added, and no convenient target was minted to work around it.

### Round r06 evidence classification

The round's thirty logs are recorded in
`plans/evidence/full-product-resume-2026-09-08/README.md` with one line each
giving the exact command, the exit code and the classification. Twenty-eight
exit 0. The two non-zero are both `EXIT=101` clippy failures on mechanical lints
in *test* targets — `unused import: windows_uia::*` and
`assertions_on_constants` — both repaired in the fix pass and both superseded by
retests at exit 0; the failing logs are retained because they happened. Every
fast gate step exited 0.

The whole set is **component evidence with five crate-level integrated test
targets. None of it is packaged evidence, native GUI evidence, or product
acceptance**, and no `acceptance` value was set from any of it. In particular
the `native_product_acceptance` and `driver_contract` targets exercise the
harness and the driver's own contract against fixtures and temp directories;
they opened no window and injected no input.

## Owner-blocked prerequisites — round r07 additions (2026-09-08)

Ten owner-blocked prerequisites were surfaced in round r07. **Four are
restatements** of prerequisites already recorded above; **six are new** and are
recorded here and in `blockers.json` as `BLK-2026-09-08-08` through
`BLK-2026-09-08-13`. Every new prerequisite string is quoted or paraphrased from
a package's `external_prerequisites` list in
[`plans/completion/dependencies.json`](dependencies.json) or from source text
named in the blocker's `prerequisite_source`. **None was invented.**

### The four restatements

- `BLK-2026-09-08-02` — a macOS host and a Linux host with the packaged native
  product installed and a real display session, able to run the windowed GUI
  e2e suite. Restated this round with a **wider scope** than the r01 entry
  recorded: besides `COMP-LANG-013`, `COMP-BTD-009`, `COMP-SCM-010`,
  `COMP-PRES-011` and packages `S3-07` and `S4-06`, it blocks package `XQ-02`
  and the macOS/Linux halves of every packaged journey row, and packet `s0-05e`
  added it to `XQ-07`'s `external_prerequisites`. The blocker's prerequisite
  string is unchanged; the widened scope is recorded in its `scope_note_r07`
  field rather than by rewriting the r01 entry.
- `BLK-2026-09-08-01` — a Linux or macOS host with this workspace checked out
  and a Rust toolchain able to run
  `cargo test -p legion-platform --test bounded_process`. The Unix nonblocking
  `fcntl` stdin branch is compiled out on this Windows host and stays
  unassessed; it is never passed by substitution from the Windows
  `PIPE_NOWAIT` branch.
- `BLK-2026-09-08-06` — a real TypeScript and JavaScript language-server,
  browser and debug-adapter setup with owner-selected releases.
- `BLK-2026-09-08-07` — an approved `tailwindcss-language-server` release
  artifact with a published version and SHA-256.

`BLK-2026-09-08-04` also still stands, unretired: no MSI was built in round r07
and nothing was staged, so
`target/native-input-acceptance/package/legion-desktop.exe` is still absent and
still blocks every harness run. **No blocker changed status this round.**

### BLK-2026-09-08-08 — signing, notarization and update-feed infrastructure

> Owner-supplied signing, notarization and update-feed infrastructure; no
> signing credential, certificate, key, notarization tool, provider or feed is
> available in the retained facts.

Quoted verbatim from the `external_prerequisites` of packages `S6-01`, `XQ-04`
and `XQ-07`. Blocks `COMP-DIST-003`, `COMP-DIST-005`, `COMP-DIST-006`,
`COMP-DIST-007` and the signed half of `COMP-DIST-010`. The r07 package work is
deliberately unsigned and does not touch it: `scripts/package-native.ps1` writes
`signer_status = "unsigned-beta/no-os-code-signing"` into
`RELEASE-METADATA.toml`, and `scripts/stage-native-acceptance-package.ps1`
copies that value verbatim into `STAGING-EVIDENCE.toml` beside an explicit
`signed = false`, so an unsigned artifact cannot be staged as anything else. No
signature, notarisation or publisher identity was produced.

### BLK-2026-09-08-09 — a clean virtual machine per supported OS

> A clean virtual machine for each supported OS with no prior Legion
> installation.

Quoted verbatim from the `external_prerequisites` of `S6-01` and `XQ-07`. Blocks
`COMP-DIST-007` clean-machine install and trust qualification. Any package the
r07 sequence would produce is built and extracted on a developer host, and an
`msiexec /a` administrative extraction is not an installation.

### BLK-2026-09-08-10 — an OS-level per-process network capture

> An OS-level per-process DNS, TCP and UDP network capture on each supported
> OS, applied to the packaged Manual/offline artifact process tree.

Quoted from `XQ-06`'s `external_prerequisites`. Blocks `COMP-DIST-004`
zero-egress verification. It also constrains
`SC-TRAIN-PACKAGED-STAGE5-MANUAL-OFFLINE`, the scenario packet `s0-05e`
authored for `COMP-TRAIN-009`: its `OR-NO-EGRESS` oracle cannot be answered
without such a capture, and its `RC-NO-CAPTURE-TOOL` recovery case records
blocked with this exact prerequisite rather than passing.

### BLK-2026-09-08-11 — an independent external security and privacy auditor

> An engaged independent external security and privacy auditor and the archived
> audit report; the register currently records that report as missing.

Quoted from `XQ-08`'s `external_prerequisites`. Blocks `COMP-P9-F2-T4-1`. No
agent may stand in for an independent external auditor, and no report exists to
archive.

### BLK-2026-09-08-12 — named external endpoints and their credentials

> Named external endpoints and their credentials for the remote, provider,
> collaboration and enterprise claims; none is available in the retained facts.

Quoted from `S5-14`'s `external_prerequisites`. Blocks `COMP-ENT-005`, and
constrains what `COMP-TRAIN-009`'s new qualification scenario can ever be run
against: a host-controlled collection endpoint is one of the things
`SC-TRAIN-PACKAGED-STAGE5-QUALIFICATION` needs, and its
`RC-NO-HOST-CONTROLLED-DESTINATION` recovery case exists for exactly that
absence.

### BLK-2026-09-08-13 — a CJK IME active on the packaged product's window

> A Windows 11 x64 host with a CJK IME installed (for example Microsoft IME for
> Japanese) and active as the input layout of the packaged product's window, so
> the driver can drive a real composition by key injection rather than
> synthesizing a commit.

Quoted verbatim from `IME_PREREQUISITE` at
`crates/legion-input-driver/src/main.rs:513-518` with the `concat!` parts
joined — source text, not a harness observation. (The round brief cited lines
507-512, the constant's position before this round's `main.rs` edit; the text is
unchanged.) `observe_ime_cjk` blocks whenever the product window's keyboard
layout is not CJK, so the `ime-cjk` class will report blocked on the first real
conformance run on this host. Packet `s1-02d` landed first precisely so that
outcome is published as `status = "blocked"`, `exit_code = 3` carrying this
prerequisite, rather than as `status = "conformance-failed"`, `exit_code = 1`
against the product. **It has never been observed:**
`xtask native-product-acceptance` has not been invoked and no `run.json` exists.

### The r06 standing constraint is retired in code only

Round r06 recorded a standing constraint: until the driver's blocked exit was
propagated into a blocked harness status, no `conformance-failed` artifact from
`xtask native-product-acceptance` could be entered anywhere in the register.
Packet `s1-02d` propagates it. A driver that exits `3`/blocked now yields
`status = "blocked"`, `exit_code = 3`, carrying the driver's own composed
prerequisite, and the artifact gains `input_classes_blocked` and
`input_classes_deviating` so a blocked run names which oracles had no answer.

**That retirement is in the code and nowhere else.** No
`native-product-acceptance` artifact of any status exists on this host, nothing
was transcribed into `defects.json` or any requirement row, and no run was
unblocked. Two asymmetries in the new mapping survive and are recorded in
`packets.json` under the packet's `review_follow_ups`: the pass arm does not
consult the two new fields, and the conformance-failed arm has no
`stated_status` guard matching the pass arm's `stated_status_denies_pass`. Both
are unreachable from today's driver. `COMP-PLAT-002` stays
`implementation: partial`, `acceptance: unassessed`.

### Round r07 register effect

Round r07 applied eight reviewer-proposed implementation transitions across
three passed packets: `COMP-PLAT-002` to `partial` and `COMP-P1-F1-T1-1` to
`implemented` for `s1-02d`; `COMP-P1-F1-T3-1`, `COMP-P0-F3-T1-1`, `-T2-1` and
`-T3-1` to `implemented` and `COMP-DIST-010` and `COMP-TRAIN-009` to `absent`
for `s0-05e`; and `COMP-PLAT-002` to `partial` again for `s1-02e`. **All eight
were verified no-ops** — every row already held the value proposed for it.

`plans/completion/requirements.json` holds 419 rows before and after,
143 `implemented` / 233 `partial` / 43 `absent`, and all 419 `acceptance` values
`unassessed`. The `implementation` and `acceptance` columns are byte-identical
to `HEAD` as well. The file *is* modified against `HEAD` (`21dfc0b`), on 11
fields across 6 rows, all authored by `s0-05e` and all of them structural:

- `COMP-P1-F1-T3-1` — `kind` `product` to `internal`; `scenario_ids` to
  `[SC-REPO-GUI-HARNESS-SOURCE-GATES]`; `configuration_ids` to the four
  `CFG-*-RUST-WORKSPACE` rows; `protected_product_ids` to
  `[COMP-P1-F1-T1-1, COMP-PLAT-002]`.
- `COMP-P0-F3-T1-1`, `-T2-1`, `-T3-1` — `protected_product_ids` from
  `[COMP-P1-F1-T3-1]` to `[COMP-SCOPE-GAP-01, COMP-SCOPE-GAP-02]`, because the
  row they protected is no longer a product row.
- `COMP-TRAIN-009` — `scenario_ids` to the two newly authored
  `SC-TRAIN-PACKAGED-STAGE5-*` scenarios; `configuration_ids` to seven
  `MANUAL-OFFLINE` / `PRODUCT-JOURNEY` configurations.
- `COMP-DIST-010` — `stage` `S6` to `S1`, `package_id` `S6-01` to `XQ-07`,
  keeping `kind: product` so its distribution journeys must still be truly
  earned.

The product-row count moves 357 to 356 with `COMP-P1-F1-T3-1`'s re-kinding,
replaced by a protection edge to two product rows, so release gating is not
loosened. No `owner_approval_ref` value exists anywhere in the register, so
there is still **no ratified cell, provisional or otherwise**.

Two register judgements in that set want owner ratification rather than agent
confidence. `XQ-07` as `COMP-DIST-010`'s home is argued from declared scope and
the argument holds, but
`docs/superpowers/plans/2026-09-04-production-qualification.md:325` still names
`S6-01` as the consumer that performs this row's update/rollback and
uninstall/deletion outcomes; either ratify `XQ-07`/`S1` or amend the plan prose,
because the row cannot return to `S6-01` while the validator rejects a product
row owned by S6. And all three `COMP-P0-F3-*` rows received the identical
protection pair, defensible as one guarantee at three layers but disclosed as a
judgement, not a ruling: if the owner reads the backlog as safeguarding
something narrower, all three edges move together.

`plans/completion/requirements.json` is committed in round r07's **record**
commit rather than in `s0-05e`'s, even though `s0-05e` authored its content,
because the round rule reserves that file to the record role. `scenarios.json`
(120 scenarios to 122) and `dependencies.json` ride in `s0-05e`'s own commit.

### The register's structural-issue count was not measured this round either

There is no r07 `verify-completion-register` log. `s0-05e`'s success criterion —
exit 0 with zero structural issues — is **predicted, not measured**. The last
measured figure remains 3 structural issues at exit 1, from round r06's
post-round run, and its three issues are exactly the three this packet
addresses. Two independently written Node ports of `validate_register_structure`
both calibrate to 3 at that commit and report 0 on this tree; that is
corroboration by reimplementation, not an exit code. Exact prerequisite to close
it: the cargo lane runs `cargo run -p xtask -- verify-completion-register
--root .` from `D:/legion-ide-completion`, logs it with a
CMD/CWD/BEGIN/END/EXIT frame, and a later record cites the measured issue list
and exit code in place of this prediction.

### Round r07 evidence classification

Nineteen raw logs, **all nineteen ending `EXIT=0`** — the first round in this
effort with no non-zero log at all. Every one is **component evidence, with
three crate-level integrated test targets
(`xtask --test native_product_acceptance`,
`legion-input-driver --test driver_contract`,
`xtask --test completion_command`) and one PowerShell contract suite
(`scripts/test-native-package-verifiers.ps1`). Not packaged evidence, not native
GUI evidence, not product acceptance.** No product window was opened, no
OS-level input was injected into any process, `xtask native-product-acceptance`
was not invoked, no MSI was built, and nothing was staged. All nineteen were
copied byte-identical, SHA-256 verified after each copy, into
[`plans/evidence/full-product-resume-2026-09-08/`](../evidence/full-product-resume-2026-09-08/README.md),
whose README carries one line per log with its exact command, exit code and
classification.

## Owner-blocked prerequisites — round r10 additions (2026-09-09)

Eleven owner-blocked prerequisites were surfaced in round r10. **Ten are
restatements** of prerequisites already recorded above; **one is new** and is
recorded here and in `blockers.json` as `BLK-2026-09-09-14`. No prerequisite
string was invented, and **no blocker changed status this round**.

### The ten restatements

`BLK-2026-09-08-02` (macOS and Linux hosts with the packaged product and a real
display), `BLK-2026-09-08-05` (an approved native Node >= 22.22.2 plus retained
TypeScript fixtures), `BLK-2026-09-08-06` (owner-selected TypeScript/JavaScript
server, browser and debug-adapter releases), `BLK-2026-09-08-07` (an approved
`tailwindcss-language-server` artifact with version and SHA-256),
`BLK-2026-09-08-08` (signing, notarization and update-feed infrastructure),
`BLK-2026-09-08-09` (a clean VM per supported OS), `BLK-2026-09-08-10` (an
OS-level per-process DNS/TCP/UDP capture on the packaged process tree),
`BLK-2026-09-08-11` (an engaged external security and privacy auditor with an
archived report), `BLK-2026-09-08-12` (named external endpoints and credentials
for remote/provider/collaboration/enterprise claims) and `BLK-2026-09-08-13` (a
Windows 11 x64 host with a CJK IME active as the packaged window's input
layout). None of them was touched by round r10, which shipped three test-side
repairs and nothing else.

`BLK-2026-09-08-05` carries a re-measurement note rather than a status change.
Round r08 measured node `v24.19.0` on this host with the retained archives
present, and `round-r08-test-s2-02a-live-language-servers.log` records
`typescript_app_startup` at 6 passed / 0 failed, `EXIT=0` — which is exactly the
command the blocker names. The record role does not retire an owner-blocked
prerequisite on its own reading of a prior round's log; the coordinator is asked
to rule on it. It stays `blocked` until then, and this note is the reason it
should not stay there quietly.

### BLK-2026-09-09-14 — macOS 15 and Ubuntu 24.04 hosts for local reproduction of hosted test failures

> A macOS 15 host and an Ubuntu 24.04 host with this workspace checked out and
> the workspace Rust toolchain, able to run `cargo test -p legion-app --lib`,
> `cargo test -p legion-desktop --lib` and `cargo test -p legion-editor --lib`
> locally, so that the failures observed in hosted `Legion Gates` run
> `34322199039` can be reproduced and a candidate repair falsified before it is
> pushed.

This is distinct from `BLK-2026-09-08-01`, which names a Linux or macOS host for
one specific test (`legion-platform --test bounded_process`), and from
`BLK-2026-09-08-02`, which needs the **packaged** product and a real display.
This one needs only a source checkout and a toolchain, and it exists because all
three r10 repairs are reasoned from hosted logs and validated on a host where
none of the three target failures reproduces.

**Measured this round, and it changes the shape of the request.** The record
role ran `gh run list --branch codex/full-product-resume --workflow
"Legion Gates" --limit 30` and `gh run view 34322199039 --job <id> --log-failed`
for each failing job. Nine runs on this branch, all nine `completed / failure`.
Run `34322199039` failed on **all three** `Standing gates` jobs, not two:

| Job | Failing test | Packet |
| --- | --- | --- |
| `windows-latest` (`102371151604`) | `language::typescript_organize_tests::typescript_organize_uses_file_and_all_mode_without_candidates`, panic at `typescript_organize_tests.rs:112:5`; 446 passed / 1 failed | `s2-03e` |
| `macos-latest` (`102371151801`) | the same test, same panic site; 450 passed / 1 failed | `s2-03e` |
| `ubuntu-latest` (`102371151608`) | `view::streamed_layout::worker::tests::worker_admission_is_bounded_and_cancel_rejects_late_output`, panic at `worker.rs:659:18`; 241 passed / 1 failed | `s1-08a` |
| `ubuntu-latest` (`102371151608`) | `tests::retention_drained_prefix_releases_each_unpinned_descriptor_and_preserves_lease`, panic at `legion-editor/src/lib.rs:4573:9`; 56 passed / 1 failed | `s1-04m` |

So the round brief's framing — three repairs to `macos-latest` and
`ubuntu-latest` failures — is wrong in two ways. The TypeScript failure is on
`windows-latest` and `macos-latest`, not on `ubuntu-latest` (`ubuntu-latest` ran
`legion-app --lib` clean at 451 passed / 0 failed), and the branch is red on
three operating systems rather than two.

**The `s2-03e` half of this blocker is not owner-blocked at all**, and that is
recorded here rather than filed as a host request the owner cannot usefully
answer: a hosted `windows-latest` runner already reproduces that failure, and
this Windows host does not, at the same test count (447 here; 446 passed + 1
failed there). A hosted runner is reachable by pushing this branch. What is
genuinely blocked is macOS and Linux, for `s1-08a` and `s1-04m`.

Affects packets `s2-03e-typescript-organize-path-shape`,
`s1-08a-streamed-worker-mailbox-flake` and `s1-04m-snapshot-lease-expiry`, and
requirements `COMP-LANG-007`, `COMP-P1-F4-T2-1`, `COMP-PRES-007` and
`COMP-P0-F4-T5-1`. It promotes and demotes nothing on its own.

### Round r10 register effect

**None in `implementation` or `acceptance`.** All four reviewer-proposed
transitions across the three passed packets were applied as written and **all
four were verified no-ops**: `COMP-LANG-007` (`partial`), `COMP-P0-F4-T5-1`
(`partial`), `COMP-P1-F4-T2-1` (`implemented`) and `COMP-PRES-007` (`partial`)
each already held the proposed value.
[`plans/completion/requirements.json`](requirements.json) is **byte-identical**
before and after the record step — it does not appear in `git diff --numstat`
at all. 419 rows before, 419 after; 143 implemented / 233 partial / 43 absent,
before and after; all 419 `acceptance: unassessed`, before and after.

That is thirty consecutive no-op transitions across r05, r06, r07 and r10 —
three, fifteen, eight and four. The `implementation` column has now gone four
recorded rounds unexercised. The reading stays the honest one: reviewers keep
proposing the value a row already holds, and none of these packets produced the
kind of evidence that moves a row. For r10 that is exactly right — three test
repairs add no product capability — but it means the column's first real
transition is still ahead, and will surface disagreements these no-ops hide.

### Round r10 evidence classification

Twenty-four raw logs, **all twenty-four ending `EXIT=0`**. Twenty-two are
cargo-lane logs and every one of them is **component evidence: not packaged
evidence, not native GUI evidence, not product acceptance.** No product window
was opened, no OS-level input was injected, `xtask native-product-acceptance`
was not invoked, no MSI was built and nothing was staged.
`round-r10-test-packet-tests-s1-04m-atomicity.log` runs a `tests/` binary, but it
links one crate and drives its public API in-process, so it is a crate-level
test rather than an integration of the product's parts.
`round-r10-test-packet-tests-s1-08a-soak.log` is 30 repetitions of the same
in-process unit tests on this host — a local flake measurement, not evidence
about the hosted runners.

The remaining two, `round-r10-record-gh-run-list.log` and
`round-r10-record-gh-run-34322199039-failed-tests.log`, are neither component
nor integrated evidence: they are **hosted-CI observations** taken by the record
role, a record of what GitHub-hosted runners reported. The standing rule already
classifies a GitHub-hosted runner as a component-layer workspace gate and never
as a substitute for packaged-product acceptance, and nothing here changes that.

All twenty-four were copied byte-identical (`cmp` clean on every file) into
[`plans/evidence/full-product-resume-2026-09-08/`](../evidence/full-product-resume-2026-09-08/README.md),
whose README carries one line per log with its exact command, exit code and
classification. Those logs were filed in the `2026-09-08` directory by explicit
round instruction; rounds r08 and r09 filed theirs under
`plans/evidence/full-product-resume-2026-09-09/`, so r10 evidence is not
co-located with the rounds immediately before it.

### What round r10 does not establish

No repair in this round is observed to fix anything. Exact prerequisite for
closing that: a completed `Legion Gates` run on branch
`codex/full-product-resume` after these commits land, with `windows-latest`,
`ubuntu-latest` and `macos-latest` all green. Until that run exists, the three
packets are plausible repairs validated against a host that reproduces none of
the failures they target — and for `s2-03e` specifically, the reason this
Windows host passes a test the hosted `windows-latest` runner fails at the same
panic site is undetermined, which makes the 8.3 short-name falsification the
brief marked optional the obvious next step rather than an optional one.
