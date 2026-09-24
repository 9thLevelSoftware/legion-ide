# S0-01l Work preservation scope audit

This inventory replaces only `COMP-SCOPE-FAMILY-09` from committed baseline `b80d1f1` with `COMP-PRES-001..013`. The governing family is **“Work preservation”**: dirty recovery, external edits, save conflicts, checkpoints, restart, restore, storage failure, and safe migration (`docs/superpowers/specs/2026-09-04-product-completion-design.md`, Required feature families, family-09). All product acceptance remains `unassessed`.

## Exact source promises and mappings

The manual plan requires the authoritative input-to-app-to-service-to-external-effect path, proposal-mediated saves, preservation on cancel/process loss/LSP failure/storage failure/external edit/save conflict/restart/migration failure, and native external oracles. S1-07 explicitly requires dirty recovery, external-change conflict, checkpoint and storage-failure recovery, local-history restoration, and Git state. The editing package requires ordinary undo/redo semantics; the cross-stage evidence contract requires disk reads, restart-restored state, and recovery results. These promises normalize as follows:

| inspected source / heading | mapping | classification boundary |
|---|---|---|
| `docs/superpowers/specs/2026-09-04-product-completion-design.md`, family-09 Work preservation; qualification/evidence | PRES-001..013 | governing full scope; status is not acceptance |
| `docs/superpowers/plans/2026-09-04-manual-language-completion.md`, S1-02, S1-04, S1-06, S1-07, S1-08, S2-05, native/manual recovery scenarios | PRES-001..013 | native host effects required; direct action/projection tests are not final proof |
| `docs/superpowers/plans/2026-09-04-completion-traceability.md`, P1/P2/P3 save, local-history, checkpoint, undo and recovery sources | PRES-001..010 | exact retained editor/SCM/WB rows are dedup targets; narrower end-to-end recovery remains explicit |
| retained master/roadmap/readiness/phase/install/adaptive/foundational/remaining plans and canvas direction | PRES-001..013 | inspected leads; no work-preservation promise omitted due to historical status |
| `crates/legion-app/src/lib.rs`, save/proposal/local-history/workspace/terminal lifecycle symbols | PRES-001..006, PRES-009, PRES-012 | authority traces, not host-observed proof |
| `crates/legion-app/src/debug_workflow.rs`, `crates/legion-debug/src/live_session.rs`, `crates/legion-editor/src/lib.rs` | PRES-013 | process/editor state traces, not host-observed proof |
| `crates/legion-editor/src/lib.rs`, `crates/legion-platform/src/lib.rs` | PRES-007, PRES-009/010 | editor/history and filesystem/encoding contracts; native persistence unassessed |
| `crates/legion-storage/src/local_history.rs`, `crates/legion-desktop/src/{bridge,view}.rs` | PRES-003..006 | checkpoint/history/recovery routes and projections; labels are not effects |
| `crates/legion-app/tests/workspace_vfs_integration.rs`, `crates/legion-observability/src/lib.rs` | PRES-008 | external overwrite and stale-rejection traces; no new acceptance claim |
| `crates/legion-protocol/tests/dto_contracts.rs` | PRES-005, PRES-007, PRES-010 | protocol contracts do not prove implementation/native recovery |
| `xtask/src/completion/schema.rs`, retained WS-MANUAL-01 evidence | PRES-011..013 | EvidenceRun schema/historical evidence only; required native record is not asserted present |

## Manual preservation promise cross-reference

The manual plan's explicit promise to preserve user work on cancel, terminal/debug process loss, LSP failure, storage failure, external edit, save conflict, restart, and migration failure (`docs/superpowers/plans/2026-09-04-manual-language-completion.md`, line 21) is not wholly a family-09 duplicate. The retained rows provide the following exact semantic coverage:

| manual case | retained requirement IDs | coverage boundary |
|---|---|---|
| cancellation of a replacement/refactor proposal | `COMP-NAV-010`, `COMP-NAV-013`, `COMP-LANG-007` | These titles explicitly require cancel to withdraw/leave dirty buffers unchanged or preserve proposal rollback; they cover the named navigation/language proposal cases, not every cancellable workflow. |
| terminal process loss/cancellation | `COMP-PRES-012`; retained `COMP-TERM-006`, `COMP-TERM-010`, `COMP-TERM-011` | `PRES-012` explicitly requires active editor text/selection/history preservation; terminal rows cover cancellation/exit, cleanup, recovery, truthful crashed/unavailable state, and stale-output suppression. |
| debug process loss/crash recovery | `COMP-PRES-013`; retained `COMP-LANG-012`, `COMP-BTD-006`, `COMP-LANG-013` | `PRES-013` explicitly requires active editor text/selection/history preservation; debug rows cover crash recovery/truthful failure and native recovery evidence. |
| LSP failure/restart/cancellation | `COMP-LANG-004`, `COMP-LANG-013` | `COMP-LANG-004` explicitly requires launch/handshake/read failure, cancellation, crash/backoff/restart while preserving the active buffer; `COMP-LANG-013` requires stale/failure/restart recovery in native EvidenceRun records. |

The previously identified terminal/debug process-loss gap is now represented explicitly by `PRES-012` (S1-06) and `PRES-013` (S2-05). Existing retained IDs remain valid deduplication targets for their narrower stated outcomes: `COMP-NAV-010`/`013` and `COMP-LANG-007` for cancellable proposals; `COMP-TERM-006`/`010`/`011` and `COMP-LANG-012`/`COMP-BTD-006`/`COMP-LANG-013` for truthful process/session failure and recovery; and `COMP-LANG-004`/`COMP-LANG-013` for LSP failure/restart while preserving the active buffer. `PRES-011` in S1-08 qualifies native evidence for the new terminal/debug preservation outcomes. Storage failure, external edit, save conflict, restart, and migration failure remain represented by the other PRES atomic outcomes above, subject to their existing unassessed status.

## Atomic outcomes and implementation limits

| ID | atomic outcome | committed-source classification and limit |
|---|---|---|
| PRES-001 | versioned proposal save, atomic/fail-closed write, dirty preservation | app save authority and platform errors are partial traces |
| PRES-002 | Save/All/As preserve path/content/encoding/newline and isolate per-file failure | app/project save symbols; acceptance unassessed |
| PRES-003 | autosave/hot-exit dirty recovery and no silent newer-disk overwrite | lifecycle/projection sources; no native hot-exit claim |
| PRES-004 | crash/restart restores recoverable buffers/metadata and handles corrupt state | platform/lifecycle traces; recovery not proven |
| PRES-005 | checkpoint restore preserves edits and rejects stale/conflict restores | bridge/view/protocol checkpoint traces; contracts are not native proof |
| PRES-006 | local-history browse/restore via proposal with conflict protection | storage/app symbols; no accepted restore record |
| PRES-007 | grouped undo/redo for compound and multi-cursor edits | editor/protocol traces; existing EDIT rows own exact editor promises |
| PRES-008 | external overwrite/stale save conflict preserves memory and disk | workspace VFS/stale event traces; product acceptance unassessed |
| PRES-009 | partial write/storage/permission/encoding/migration failure is fail-closed and retryable | platform/app error models; no fabricated success claimed |
| PRES-010 | encoding/newline/final-newline preservation through persistence/history | platform/protocol fields; native round-trip unassessed |
| PRES-011 | native host-observed disk/buffer/checkpoint/history/restart/conflict evidence with OS/reviewer coverage | EvidenceRun schema and historical evidence only |
| PRES-012 | terminal cancellation/process loss preserves active editor text, selection, and undo/redo history with truthful cleanup/retry state | terminal/editor lifecycle traces exist; native preservation remains unassessed |
| PRES-013 | debug-adapter cancellation/process loss preserves active editor text, selection, and undo/redo history with truthful crash/retry state | debug/editor lifecycle traces exist; native preservation remains unassessed |

## Exact deduplication

Existing editor, Workbench, and Source-control rows remain unchanged. Exact broad outcomes are mapped rather than duplicated: editor rows cover ordinary undo/redo and save input; SCM rows cover Git status/stage/commit/history and external Git state; WB rows cover layout/session metadata. The PRES rows retain narrower cross-surface save-all/save-as, hot-exit/crash restart, checkpoint/local-history proposal recovery, stale snapshot, write failure, encoding/newline, and host-observed end-to-end preservation promises. Protocol DTOs, status rows, and harness fixtures are implementation evidence only. Every new row is `implementation: partial` from committed source traces and `acceptance: unassessed`; this inventory does not claim S0 completion.
