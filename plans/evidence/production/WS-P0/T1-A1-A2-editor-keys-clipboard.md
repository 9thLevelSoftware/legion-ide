# T1 — A1/A2 Editor keys + OS clipboard

**Date:** 2026-07-21  
**Packet:** Tier 1 daily-driver — Backspace/Delete/Enter + copy/cut OS clipboard  

## Changes

### A1 — Buffer mutation keys

| Key | Behavior |
| --- | --- |
| **Backspace** | Deletes non-empty selection, else previous UTF-8 scalar (including cross-line newline) using app buffer text |
| **Delete** | Deletes non-empty selection, else next UTF-8 scalar |
| **Enter** | Replaces selection with `\n`, else inserts `\n` at cursor; suppressed while completion popup open; does not steal from Alt+Enter review apply |

Problems-panel plain Enter only fires when `!editor_input_enabled` so the editor keeps Enter for newlines.

### A2 — OS clipboard

- `AppComposition::selected_text_for_clipboard` returns selection text for desktop (outcomes stay metadata-only).
- Before dispatching `ClipboardCopy` / `ClipboardCut`, `handle_keyboard` calls `ui.ctx().copy_text(...)`.
- Cut still deletes via existing app `ClipboardCut` path.

### A12 — Tree depth (same Tier 1 wave)

- `MAX_TREE_CHILDREN_DEPTH` raised **2 → 32** so paths like `crates/<crate>/src/lib.rs` enter explorer/quick-open/search.

## Primary files

- `crates/legion-desktop/src/workflow.rs`
- `crates/legion-app/src/lib.rs` (`selected_text_for_clipboard`, `buffer_text_for_input`)
- `crates/legion-project/src/lib.rs` (tree depth)
- Tests: `crates/legion-desktop/tests/input_conformance.rs`, unit tests in `workflow.rs`

## Gates (when cargo available)

```text
cargo test -p legion-desktop --test input_conformance
cargo test -p legion-desktop --lib
cargo test -p legion-app --lib
cargo test -p legion-project --lib
cargo test -p legion-desktop --test daily_editing_controls
```
