# T1 — A8 / A10 / A11 storage, watcher, terminal

**Date:** 2026-07-21  

## A10 — Durable product state

| Change | Detail |
| --- | --- |
| Event sink | `AppComposition::new` uses `RedactingEventSink` (not `NoopEventSink`) so audit events are retained in-process |
| Proposal audit durability | `InMemoryStorageRepositoryPort` supports optional `base_dir`; saves to `.legion/proposal-audit/<id>.json` and reloads on open (PKT-CKPT I4) |
| Workspace enable API | `enable_proposal_audit_persistence` + `enable_workspace_state_persistence` (palette + checkpoints + proposal audit) |
| Desktop open | Calls `enable_workspace_state_persistence` on workspace root |

## A11 — Recursive watcher poll

| Change | Detail |
| --- | --- |
| `NativeWatcherService::snapshot` | Recursive walk, depth cap 32, skips `target`/`node_modules`/`.git`/`.legion`/etc. |
| Overflow | Still fail-closed at 4096 events |

Still poll-based (no OS `notify` backend); nested monorepo files now appear in `last_scan` fingerprints.

## A8 — Interactive terminal

| Change | Detail |
| --- | --- |
| Frame poll | Active terminal session → `TerminalOutputPoll` every frame + 50ms repaint |
| Input UI | Terminal panel text field + Send; Enter submits payload with trailing newline |
| Controls | Poll / Kill / Close buttons |
| Idle deadline | Extended on `TerminalInput` by the launch timeout window |
| Default timeout | Product launch default **3600s** (was 30s); explicit values still honored |

## Verification (this session)

```text
cargo check -p legion-desktop --lib
cargo test -p legion-platform --lib watcher
cargo test -p legion-storage --lib proposal_audit
cargo test -p legion-terminal --lib terminal_runtime  # 15 pass
cargo test -p legion-desktop --test input_conformance  # 8 pass
```
