# GAP-04.3 — Desktop-integration proof of hot-exit restore

**Date:** 2026-08-31  
**Wave:** 1 daily-driver safety  
**Task:** GAP-04.3 (desktop-integration proof of GAP-04.1)

## What this is

DesktopRuntime integration of crash-safe unsaved-buffer restore:

1. Edit a buffer without `SaveActive`.
2. Persist session metadata + `.legion`-adjacent `unsaved/` sidecar (`save_session_state`).
3. Drop the runtime (killed session).
4. Reopen the same workspace from that session path.

## What this is not

This is not a windowed `eframe::run_native` run and not GAP-01 installed-package GUI E2E. It drives `DesktopRuntime` the same way other desktop session tests do: real session store, real hot-exit store, real app restore. Headless AppComposition-only restore remains in `crates/legion-app/tests/daily_editing_contracts.rs`.

## Assertions

| Check | Result |
| --- | --- |
| Disk file still has the pre-crash body | `notes.txt` stays `clean` |
| `session.json` has no dirty body | marker `DIRTY` absent |
| Sidecar exists | `unsaved/manifest.json` next to `session.json` |
| Reopened projection shows dirty text | `small_buffer_text() == Some("cleanDIRTY")` |
| Restore still does not write disk | file remains `clean` after reopen |

Proposal-mediated save remains the durable write.

## Verification

```text
cargo test -p legion-desktop --test session_restore session_restore_killed_dirty_session_restores_sidecar_without_writing_disk
```

Primary files:

- `crates/legion-desktop/tests/session_restore.rs`
- `crates/legion-desktop/src/workflow.rs` (`save_session_state`, hot-exit load on open)
- `crates/legion-storage/src/hot_exit.rs`

Ledger row statuses are unchanged. No production-ready / GA claim.
