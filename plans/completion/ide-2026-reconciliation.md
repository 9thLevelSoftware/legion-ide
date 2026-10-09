# IDE 2026 program reconciliation

Date: 2026-10-08. Ticket: 01. Status: reconciled; independent review passed (Einstein).
Baseline: `24f2970d3fbb917a03c7306e299399e83b23e2b3` on
`codex/ide-2026-ticket-001`, fast-forwarded from the integration branch before edits.

This is a scope/ownership crosswalk, not a second completion register. The
[approved specification](../../.scratch/ide-2026-planning/spec.md),
[Q19–Q36 decisions](../../.scratch/ide-2026-planning/decisions.md),
[Q1–Q18 and F0–F8](../../.omp/plans/FRONTIER_INTEGRATION_PLAN.md) and
[approved numbered tickets](../../.scratch/ide-2026-planning/ticket-breakdown.md)
govern the mapping. Ticket numbers below resolve through that breakdown and its
[issue map](../../.scratch/ide-2026-planning/map.md); they assign downstream work,
not permission to perform it in ticket 01.

Canonical authority remains [requirements](requirements.json), [matrix](matrix.json),
[scenarios](scenarios.json), [dependencies](dependencies.json), [defects](defects.json),
[inventory decisions](decisions.md), [implementation audit](implementation-audit.md)
and the [product-readiness ledger](../product-readiness-ledger.md). None changes
status or evidence here. The inspected register has 419 requirements (143
implemented, 233 partial, 43 absent), all 419 acceptance values `unassessed`, 42
configurations, 122 scenarios and 54 package records. Existing IDs, source/legacy
references, dependencies, protected outcomes, defects and evidence remain intact.

An **anchor** below identifies an existing obligation/service owner; it does not
assert that the newer atomic outcome is already enumerated or implemented. D1–D9
are findings in this document, not new canonical requirement IDs. There are no
registered `COMP-FRONTIER-*` requirements at this baseline. Additions must use that
approved namespace through the owning slice's canonical reconciliation before
activation/qualification; preserve existing IDs and avoid duplicate outcomes.

## Confirmed decision crosswalk

| Decision and retained outcome | Existing canonical anchors (package) | Approved ticket owners; delta or limit |
| --- | --- | --- |
| Q1 Complete all work; support needs independent qualification | COMP-P9-F1-T1-1 (S3-07), COMP-SCOPE-GAP-08 (S0-06) | 71, 72, 73–87, 90, 123, 251, 254; D1/D8: negative experiment can finish work, never supported delivery; blocked work stays open. |
| Q2 Understand and safely review a change | COMP-TRUST-005 (S3-01), COMP-P3-F2-T3-1 (S3-05), COMP-CTX-003 (S3-04) | 27, 31–39; D2/D3 for resolved impact and exact evidence. |
| Q3 Optional checks advisory; required checks satisfied | COMP-TRUST-001, COMP-TRUST-002 (S3-01) | 34, 35, 39; D3: record requirement authority separately from advice. |
| Q4 Automatic bounded impact; advanced work explicitly activated | COMP-CTX-002 (S3-04), COMP-SCOPE-FAMILY-13-04 (S3-06) | 27–30, 59, 73–84; D2/D3/D4/D5/D6. |
| Q5 First complete inspection/impact/test/review journey | COMP-P3-F2-T1-1 (S3-05), COMP-TRUST-006 (S3-07), COMP-BTD-004 (S2-04) | 27–39; no formal/retrieval prerequisite. |
| Q6 Relevant dirty buffers in exact subject; never substitute disk | COMP-CTX-001 (S3-04), COMP-PRES-008 (S1-05) | 32, 33, 35; D3: immutable proposed-editor-state subject. |
| Q7 Only authorized policy/developer makes checks required | COMP-TRUST-001, COMP-TRUST-002 (S3-01) | 35, 39; D3: AI/verifier cannot weaken requirements. |
| Q8 Aggregate activation limits; expansion needs new activation | COMP-SCOPE-FAMILY-13-04 (S3-06), COMP-PROV-006 (S3-02) | 59, 74–84; D3/D4/D5/D6; bounded internal attempts are not unlimited consent. |
| Q9 Independent delivery; separate work/experiment/support | COMP-P9-F1-T1-1 (S3-07), COMP-DIST-010 (XQ-07) | 72, 82, 86, 87, 90, 123, 251–254; D1/D8; partial delivery never closes the program. |
| Q10 Unknown impact widens scope or reports incomplete coverage | COMP-CTX-003 (S3-04), COMP-LANG-006 (S2-03) | 27, 30, 32, 33, 120–122; D2/D3; absent edges do not prove independence. |
| Q11 Tests/build scripts need trust and enforced isolation | COMP-SCOPE-FAMILY-13-02, COMP-SCOPE-FAMILY-13-05 (S3-06), COMP-BTD-002 (S2-04) | 33, 50, 74, 75; D3/D4; unavailable isolation blocks execution. |
| Q12 Apply preserves unrelated dirty work; label editor versus disk evidence | COMP-PRES-008 (S1-05), COMP-P3-F3-T3-1 (S3-06) | 35, 37, 39; D3; verification is not write permission. |
| Q13 Weakened/removed requirement invalidates approval | COMP-TRUST-001, COMP-TRUST-002 (S3-01) | 35, 39; D3; workspace mandates change only through authorized policy. |
| Q14 Experiments default-off with unchanged protections | COMP-TRUST-002 (S3-01), COMP-PROV-005 (S3-02), COMP-TRAIN-002 (S5-12) | 73–87, 90, 123; D1/D8: experimental activation cannot bypass privacy/isolation/mutation. |
| Q15 Approval binds exact proposal, requirements and relevant state | COMP-TRUST-002 (S3-01), COMP-PRES-001 (S1-05) | 30, 34, 35, 39; D3; unknown independence invalidates conservatively. |
| Q16 Unavailable required tool blocks only affected proposal | COMP-TRUST-005 (S3-01), COMP-PROV-004 (S3-01) | 35, 73–78, 90; D3/D4; no implicit waiver/environment switch. |
| Q17 Late output disqualified; stopping differs from termination | COMP-SCOPE-FAMILY-13-07 (S3-06), COMP-SCOPE-FAMILY-12-08 (S3-05) | 36, 41–45, 50–53; D3/D6; failed termination is an execution fault. |
| Q18 Baseline then freeze targets before candidate scoring | COMP-P9-F1-T4-1 (S3-07), COMP-PLAT-006 (XQ-03) | 24, 48, 71–73, 82, 83, 86, 87, 92, 95, 123; D8; never lower targets after scoring. |
| Q19 Unified scope with required/experiment/deferred dispositions | COMP-SCOPE-GAP-08 (S0-06), COMP-P0-F3-T2-1 (S0-01) | 01, 251, 254; retain all 22 families and F0–F8; D1/D9. |
| Q20 Internal daily-driver first; external release separate | COMP-PLAT-009 (XQ-02), COMP-DIST-010 (XQ-07), COMP-ENT-006 (S5-15) | 26, 60, 233, 235–254; D8; pilot is not enterprise/production acceptance. |
| Q21 Ordinary adoption blockers before advanced AI | COMP-BTD-007 (S2-05), COMP-SCOPE-FAMILY-17-11 (S5-15) | 02–26, 155–168; keep 73–87 frontier work; native equivalents do not waive extension runtime scope. |
| Q22 Dependency/prerequisite graph before staffing/schedule | COMP-P0-F3-T2-1 (S0-01), COMP-PLAT-001 (S0-02) | 01, 02, 03, 48, 63, 73, 83, 88, 155, 169, 212, 235; dependencies.json remains authoritative; no delivery deadline invented. |
| Q23 First Windows/Rust on Legion; all other cells retained | COMP-PLAT-001 (S0-02), COMP-LANG-001 (S2-01), COMP-LANG-013 (S2-06) | 02, 04–26, 88–151; execution approval below is limited; D9. |
| Q24 Genuine ACP extends command bridge; MCP retained | COMP-SCOPE-FAMILY-15-01 (S0-02), COMP-SCOPE-FAMILY-15-04, COMP-P6-F4-T3-1 (S4-05) | 50–53, 63–70; D7; ADR-0043 adapter is reusable substrate, not protocol qualification. |
| Q25 Useful real-provider prediction; custom training experiment | COMP-SCOPE-FAMILY-12-04 (S3-05), COMP-TRAIN-008 (S5-13) | 40, 42, 48, 49, 60, 86; D8; qualify real route versus ordinary completion. |
| Q26 Durable agent recovery stopped until explicit resume | COMP-SCOPE-FAMILY-12-11 (S3-05), COMP-SCOPE-FAMILY-13-08 (S3-06), COMP-SCOPE-FAMILY-14-08 (S4-02) | 54, 55, 69; D7; desktop layout restoration is not agent durability; after-exit execution deferred. |
| Q27 Selective settings/keymap and named essential extensions | COMP-WB-008 (S1-04), COMP-SCOPE-FAMILY-17-01 (S5-01), COMP-SCOPE-FAMILY-17-11 (S5-15) | 03, 22, 155–168; D7; unsupported imports and compatibility are explicit. |
| Q28 Independently qualify Codex and Claude Code | COMP-SCOPE-FAMILY-15-04, COMP-SCOPE-FAMILY-15-05 (S4-05), COMP-SCOPE-FAMILY-14-12 (S4-06) | 50–53, 60; D7; names are targets, not support promises. |
| Q29 Explicit prediction profile, no silent provider switch | COMP-PROV-003, COMP-PROV-005 (S3-02), COMP-SCOPE-FAMILY-12-04 (S3-05) | 40, 48, 49, 60; D8; Manual and ordinary completion remain usable. |
| Q30 Native equivalent may satisfy pilot workflow only | COMP-WB-008 (S1-04), COMP-SCOPE-FAMILY-17-07 (S5-03) | 03, 22, 155–168; execution approval below permits equivalents, never relabels them extension compatibility. |
| Q31 Reconcile uncertain external actions before retry | COMP-SCOPE-FAMILY-13-08 (S3-06), COMP-SCM-006 (S1-07) | 55, 69, 87, 182, 183; D7; no blind push/publication/migration replay. |
| Q32 Required traces/skills/remote/team; bounded agent debugging | COMP-BTD-004 (S2-04), COMP-CTX-005 (S3-04), COMP-REMOTE-017, COMP-ENT-006 (S5-15), COMP-TRAIN-008 (S5-13) | 57, 58, 87, 124, 169–233; D7/D8; AHP, Git replacement and autonomous deployment deferred. |
| Q33 Five real working days; attempts/fallback and zero critical defects | COMP-PLAT-009 (XQ-02), COMP-PRES-011 (S1-08), COMP-GAP-001 (XQ-02) | 26; D8; final 254 still requires ten consecutive owner days and two independent usability sessions. |
| Q34 Agent's own tools contained; main writes proposal-only | COMP-SCOPE-FAMILY-15-05 (S4-05), COMP-SCOPE-FAMILY-13-02 (S3-06), COMP-TRUST-002 (S3-01) | 50–53; D7; a copied workspace/cooperative adapter alone is insufficient. |
| Q35 Discovery is not executable skill trust | COMP-CTX-005 (S3-04), COMP-TRUST-002 (S3-01) | 57, 58; D7; display source/version and authorize expanded script/tool/network grants. |
| Q36 Manual and assisted acceptance separately required | COMP-PLAT-009 (XQ-02), COMP-SCOPE-FAMILY-12-13, COMP-TRUST-006 (S3-07), COMP-SCOPE-FAMILY-14-12 (S4-06) | 26, 60, 70, 251, 254; D8; credentials cannot block Manual evaluation. |

## New outcomes, missing mappings and conflicts

These are bounded mapping handoffs, not newly approved runtime contracts. Existing
scenario anchors below remain canonical; A01–A20 are specification targets, not
invented `SC-*` records. The owning ticket must refine/register its missing atomic
outcomes and scenario/configuration joins before claiming them satisfied. Ticket
251 checks full coverage; ticket 254 performs the final complete acceptance join.

| Finding / specification outcomes | Existing anchors | Approved owners / unresolved mapping |
| --- | --- | --- |
| D1 — F0/F8 register and qualification tooling; A20 | COMP-P0-F3-T2-1, COMP-SCOPE-GAP-08; SC-REPO-BACKLOG-REGISTER-VALIDATION, SC-REPO-DOC-TRUTH-AUDIT | 01 supplies this crosswalk; 71 deterministic frontier contracts, 72 real qualification execution/evidence, 251 full coverage, 253 guidance, 254 final join. F0's claim that registers/validators are absent is superseded; candidate nomination remains separate. No COMP-FRONTIER atomic IDs or frontier scenarios exist here; owning slices must add only genuinely new outcomes. |
| D2 — F1/F3 resolved graph, incremental lineage/cache, derived canvas; A03/A04/A13 | COMP-CTX-002, COMP-CTX-003, COMP-LANG-006, COMP-CAN-008; SC-LANG-CALL-HIERARCHY, SC-CANVAS-GROUPS-DERIVED-EDGES | 27–31, 120–122, 152–154. Anchors do not enumerate exact edit lineage, worker tree bounds, resolved declaration identity, independent coverage or dependency epochs. Register those deltas. Native LSP/Tree-sitter/index/scheduler remains selected; SCIP producer/Salsa/search database replacement deferred. |
| D3 — F2/F3 exact verification subject/envelope, requirements, lifecycle, persistence and staged execution; A02–A06/A08/A09 | COMP-TRUST-001, COMP-TRUST-002, COMP-TRUST-003, COMP-TRUST-004, COMP-PRES-009, COMP-SCOPE-FAMILY-13-05; SC-AI-TRUST-PROPOSAL-LIFECYCLE, SC-AI-CHECKPOINT-RESTORE | 32–39, 59. No exact VerificationEnvelope or independent freshness/coverage/outcome/artifact/termination register rows. Dirty-overlay digests, requirement weakening, conditional approval invalidation, isolated staged checks and unknown old records need atomic additions; use existing repositories, preserve originals and consent. Runtime evidence is distinct from qualification EvidenceRun. |
| D4 — F4 formal worker, validated witnesses and bounded repair; A14/A20 | COMP-SCOPE-FAMILY-13-02, COMP-PROV-004, COMP-TRUST-002; no existing formal SC/configuration owner | 73–78, 90. Unmapped positive Kani/Verus/repair and separate Mac unavailability scenarios/configurations need COMP-FRONTIER registration. Linux x64 / provisioned Windows WSL2 only; native Mac unsupported/no process/no proposal. Preserve one job, 120s, 2 GiB aggregate, 64 processes, 1 GiB scratch and bounded output; repair three candidates/180s without weakening properties. Tool bundles/offline dependencies/enforcement are prerequisites, not provisioned here. |
| D5 — F5 pure independently checked refactoring and cross-edit reuse; A13/A14 | COMP-LANG-007, COMP-TRUST-002; SC-LANG-REFACTOR-RUST is an anchor, not an equivalence-proof scenario | 73–75, 79, 80, 90. No pure-u32 IR/rules/root-version/independent checker requirement rows. Preserve wrapping domain and exclusions, 20k e-nodes/32 iterations/500ms/top-eight search, 256 MiB whole arena, binder/type correspondence and cold checks; rule matches never confer proof or write authority. |
| D6 — F6/F7 structured profiles, real cancellation and learned retrieval; A08/A15/A20 | COMP-PROV-004, COMP-SCOPE-FAMILY-12-08, COMP-SCOPE-FAMILY-13-07, COMP-CTX-003, COMP-CTX-006; SC-AI-PROVIDER-SETUP, SC-AI-CONTEXT-MANIFEST | 41–45, 81–84, 123. Add exact request/profile/schema identity, connect/body cancellation and logical-versus-termination states, serving-side XGrammar evaluation and learned-space/model validation/held-out targets. Expand–migrate–contract preserves unmigrated callers. Retain F6's 2 MiB response, 10s connect/120s upper request bound and 400ms cancellation target; F7's 256 MiB cache, one batch, 16 chunks/256 KiB, 100 labeled queries. No IDE decoder/new vector DB; no cloud fallback; Manual forbids even loopback inference. |
| D7 — ACP, stopped session recovery, migration imports, skill trust and browser artifacts; A07/A09/A11/A12/A17 | COMP-SCOPE-FAMILY-15-04, COMP-SCOPE-FAMILY-15-05, COMP-SCOPE-FAMILY-12-11, COMP-SCOPE-FAMILY-13-08, COMP-WB-008, COMP-CTX-005, COMP-BTD-004; SC-INTEROP-MCP-ACP-PEERS, SC-AI-ASSIST-SESSION-DURABILITY, SC-AI-DELEGATE-CHECKPOINT-RESUME | 03, 22, 50–55, 57, 58, 124. Refine existing ACP/durability owners; named peer/version/containment and uncertain action reconciliation are not supplied by the command bridge or in-memory reconstruction. Import preview/revert, executable skill grant expansion and browser trace provenance lack exact atomic rows/scenarios. Browser evidence cannot qualify native IDE input. |
| D8 — Real prediction usefulness, milestone separation and experiment honesty; A01/A10/A20 | COMP-SCOPE-FAMILY-12-04, COMP-PROV-005, COMP-P9-F1-T4-1, COMP-TRAIN-008, COMP-PLAT-009; SC-AI-ASSIST-REVIEW, SC-TRAIN-CANDIDATE-FEEDBACK | 26, 48, 49, 60, 82, 86, 87, 123, 251, 254. Add baseline/frozen retained-edit/rejection/undo/latency outcome joins and separate five-day Manual/assisted milestones. Agent stepping/trajectory fork lacks an exact canonical owner (context/training anchors only); 87 owns bounded experiment registration, not supported delivery. Existing training scope remains required; custom prediction success is experimental. Freeze targets before scoring; missing resources cannot count as negative conclusions. |
| D9 — Full scope/configuration/authority reconciliation; A11/A16/A18/A19 | COMP-PLAT-001, COMP-LANG-001, COMP-SCOPE-FAMILY-17-01, COMP-REMOTE-001, COMP-COLLAB-001, COMP-DIST-001; SC-REPO-STAGE0-LANGUAGE-BTD-MATRIX, SC-REPO-STAGE0-INTEROP-EXTENSION-CONTRACT | 02, 03, 09, 88, 155, 169, 212, 235. Existing matrix fields are provisional/pending; execution approval below does not ratify every field. Shared TSJS-NODE cells need separate TS/JS evidence; Mac x64 and arm64 obligations remain although tickets 134–142 say only macOS. S1-03B is a design-plan gate, not a dependency-register package; existing canvas rows belong to S1-03C. ADR-0051/F3 read-only cards conflict with retained editable-card obligation: 09 resolves contract before 11 activation; do not erase COMP-CAN or person-edge distinctions. ADR-0046 is retired, but PR-VSC-002/PR-ENT-001/PR-ENT-002 gates remain unqualified. |

## Retained full-product scope

All atomic rows remain authoritative, including those not repeated in this compact
crosswalk. The following anchors retain the 22 approved families; ticket ranges
are inclusive and assign the relevant slices, not new acceptance statuses.

| Required family | Concrete canonical anchor / existing package | Approved tickets |
| --- | --- | --- |
| Text editing | COMP-EDIT-001 / S1-04 | 04–07, 24, 89, 92–95 |
| Workbench | COMP-WB-001, COMP-WB-008 / S1-03, S1-04 | 08, 12, 21, 22, 89, 93 |
| Canvas | COMP-CAN-004, COMP-CAN-008 / S1-03C | 09–11, 31, 152–154 |
| Navigation/search | COMP-NAV-013 / S1-05 | 13, 15, 96–119 |
| Language tooling | COMP-LANG-001, COMP-LANG-013 / S2-01, S2-06 | 15, 16, 88, 96–122, 125–151 |
| Build/test/debug | COMP-BTD-004, COMP-BTD-007 / S2-04, S2-05 | 17, 18, 98, 99, 102, 103, 106, 107, 110, 111, 114, 115, 118, 119, 125–151 |
| Terminal | COMP-TERM-006 / S1-06 | 14, 176, 186 |
| Source control | COMP-SCM-010 / S1-07 | 19, 20, 179, 187, 222 |
| Work preservation | COMP-PRES-011 / S1-08 | 05, 21, 37, 38, 54, 55 |
| Provider/model setup | COMP-PROV-001, COMP-PROV-010 / S3-02, S3-03 | 40–45, 48, 61, 62, 81 |
| Context/retrieval | COMP-CTX-010 / S3-07 | 27–30, 46, 56, 83, 84, 120–123 |
| Assist | COMP-SCOPE-FAMILY-12-13 / S3-07 | 47–49, 60 |
| Delegate | COMP-SCOPE-FAMILY-13-11 / S3-07 | 43, 50, 54, 55, 59, 60 |
| Multi-agent workflows | COMP-SCOPE-FAMILY-14-12 / S4-06 | 67–70 |
| External interoperability | COMP-SCOPE-FAMILY-15-01, COMP-SCOPE-FAMILY-15-03, COMP-SCOPE-FAMILY-15-04 / S0-02, S4-04, S4-05 | 50–53, 63–66, 70 |
| Trust/proposals | COMP-TRUST-006 / S3-07 | 32–39, 47, 50, 59, 216, 219 |
| Extensions | COMP-SCOPE-FAMILY-17-11 / S5-15 | 03, 155–168, 181, 189 |
| Remote development | COMP-REMOTE-017 / S5-15 | 169–211 |
| Collaboration/enterprise | COMP-ENT-006 / S5-15 | 212–233 |
| Training/telemetry | COMP-TRAIN-008, COMP-TRAIN-009 / S5-13, S5-15 | 85–87, 221 |
| Platform quality | COMP-PLAT-009 / XQ-02 | 23, 24, 88–95, 125–154 |
| Distribution/operations | COMP-DIST-010 / XQ-07 | 234–254 |

S0 internal governance/provenance and all XQ-01–08 obligations are retained too;
ticket 01 does not retire an internal row because it is not a product workflow.
The approved breakdown assigns 251 the pre-final join (only its named final
audit/guidance/campaign descendants may remain pending) and 254 the complete join
through S6-01. No release decision is made here.

## Explicit prerequisites and limited execution approval

The integration [execution record](../../.scratch/ide-2026-planning/execution.md)
records the owner's approval of Windows 11 Pro x64 build 26200, i9-14900HX / 32 GiB,
Rust/Cargo/rust-analyzer 1.98.1 and Legion-on-Legion as the pilot basis. It also
permits demonstrated native equivalents for initial essential workflows. This
approval supersedes the old absence of those specific selections; ticket 02 still
owns their canonical configuration update and remaining debugger/native-input
prerequisites. Ticket 03 owns the exact migration inventory. Containers 2.5.2 and
Remote Containers 0.469.0 remain later extension/remote qualification targets;
installed presence is not compatibility evidence. No additional OS/tool/version,
extension set, provider or acceptance result is selected here.

| Prerequisite / canonical route | Approved owner | What remains explicit |
| --- | --- | --- |
| CFG-WIN11-X64-RUST-WORKSPACE, CFG-WIN11-X64-PRODUCT-JOURNEY; COMP-PLAT-001, COMP-LANG-001 | 02, 04, 18, 26 | Incorporate limited owner selections; debugger/driver, immutable artifact and actual native workflow/observation still required. |
| Remaining matrix and SC-LANG-LSP-LIFECYCLE-JS, SC-LANG-BUILD-TEST-JS, SC-LANG-DEBUG-JS | 88, 96–151 | Pin exact tools/projects/lockfiles/OS/hardware; qualify JavaScript independently, both Mac architectures, and native/AT sessions. Existing PENDING/PROVISIONAL fields are not ratified by this document. |
| COMP-SCOPE-FAMILY-15-01, COMP-PROV-003, COMP-PROV-011 | 40, 48, 50–53, 63 | Real endpoints/agents/peers, credentials, profiles/protocol versions, containment and frozen usefulness workloads; not inferred from fixtures. |
| D4–D6; COMP-CTX-003, COMP-TRAIN-008 | 73, 74, 75, 81–87, 90, 123 | Formal tool bundles/offline builds, enforceable Linux/WSL2 limits, model/tokenizer identity, independent workloads, consent and reviewers; blocked resources stay blocked. |
| COMP-SCOPE-FAMILY-17-01, COMP-REMOTE-001, COMP-COLLAB-001 | 155, 169, 212 | Bounded ADR/policy/dependency/contract gates, real signed extension artifacts, remote hosts/container images and team/identity services. No runtime activation by scope approval alone. |
| COMP-DIST-001, COMP-DIST-003, COMP-DIST-004, COMP-P9-F2-T4-1 | 235–252 | Signer/notarizer/feed, clean hosts, OS network capture and independent audit. Descriptors stay dry-run/no-production-signer pending real approved support. |
| COMP-PLAT-009, COMP-DIST-010; dependencies S6-01 | 26, 60, 254 | Real five-day Manual observation, separately qualified assisted milestone, ten consecutive final owner days and two independent usability sessions. |

Existing defects DEF-2026-09-05-01 (`fixed-awaiting-verification`),
DEF-2026-09-05-02 (`open`) and DEF-0003 (`open`) are preserved, along with their
repair owners and verification references. Historical blocked-prerequisite notes
are not current host probes; this reconciliation does not close them by inference.

## Local verification

Planned before editing: shared xtask `docs-hygiene`, shared xtask
`verify-completion-register --root .`, and PowerShell structural mapping checks.
The latter compare Q1–Q36 with the independent decision sources, check referenced
requirement/scenario/configuration/package IDs and approved ticket numbers, retain
22 family rows and F0–F8 coverage, and verify canonical records unchanged.
These are documentation/register checks, not runtime tests or product acceptance.
Actual commands/results are recorded in [ticket 01](../../.scratch/ide-2026-planning/issues/01-reconcile-the-2026-program-delta.md).

Independent review: PASS (Einstein), reported by the coordinator on 2026-10-08.
All 36 decision mappings, 22 families, F0–F8 references and ownership handoffs were
validated; no authority or acceptance issues were identified. This review passes
the bounded reconciliation, not downstream implementation or product acceptance.
