# Plan 04-01 Summary: Find/Replace + Keybinding Type Layer

## Result
**Status**: Complete
**Wave**: 1
**Agent**: engineering-senior-developer
**Completed**: 2026-08-11

## Completed Tasks

- **Task 1: Find/Replace Intents + Projections + Keybinding Types in legion-ui** — Added 12 `CommandDispatchIntent` find/replace variants (ToggleFindBar, CloseFindBar, SetFindQuery, FindNext, FindPrevious, ToggleFindReplace, SetFindReplaceText, ReplaceOne, ReplaceAll, SetFindCaseSensitive, SetFindWholeWord, SetFindRegex). Created `FindBarProjection` and `FindMatchProjection` structs. Added `find_bar_projection` field to `ShellProjectionSnapshot` and `Shell`. Created `KeyCombo`, `KeybindingEntry` types and `default_keymap()` with 21 bindings.

- **Task 2: BufferSearchState + find_matches in legion-editor** — Added `regex` dependency to legion-editor. Created `BufferSearchState` struct with `find_matches()` (regex-based, supports case-insensitive, whole-word, regex mode), `next_match()`, `prev_match()`, `current_match()`. Invalid regex returns 0 matches (no panic). Match positions stored as `(start_line, start_char, end_line, end_char)` tuples.

- **Task 3: Buffer Search Tests** — Created `crates/legion-editor/tests/buffer_search.rs` with 10 tests: literal, case-insensitive default, case-sensitive, whole-word, regex, invalid regex, empty query, multiline, next/prev wrap, no-matches navigation safety.

## Files Modified
- `crates/legion-ui/src/ui.rs` — Added find/replace intents, FindBarProjection, FindMatchProjection, KeyCombo, KeybindingEntry, default_keymap()
- `crates/legion-editor/src/lib.rs` — Added BufferSearchState with find_matches, next_match, prev_match, current_match
- `crates/legion-editor/Cargo.toml` — Added regex workspace dependency
- `crates/legion-editor/tests/buffer_search.rs` — 10 integration tests for buffer search

## Verification Results
All verification commands passed:
- `cargo check -p legion-ui` — exit 0
- `cargo check -p legion-editor` — exit 0
- `cargo test -p legion-editor --test buffer_search` — 10/10 tests passed

## Verification Commands
| Command | Exit Code | Result |
|---------|-----------|--------|
| `cargo check -p legion-ui` | 0 | PASS |
| `cargo check -p legion-editor` | 0 | PASS |
| `cargo test -p legion-editor --test buffer_search` | 0 | PASS — 10 tests passed |

## Key Decisions
- `BufferSearchState` is standalone (not a field on `EditorEngine`) — the app layer owns it, following the existing projection pattern.
- Keybinding types use `String`-based key names (not `egui::Key`) since legion-ui doesn't depend on egui. Conversion happens in legion-desktop.
- Match positions are `(u32, u32, u32, u32)` tuples for lightweight storage and copy semantics.

## Issues Encountered
None.

## Escalations
None.

## Handoff Context
- **Key outputs**: FindBarProjection, FindMatchProjection, BufferSearchState, KeyCombo, KeybindingEntry, default_keymap() — all types/functions needed by Wave 2
- **Decisions made**: Standalone BufferSearchState, string-based key names, tuple match positions
- **Open questions**: None
- **Conventions established**: Find intents follow SetFindX{field} pattern for consistency

## Requirements Covered
- In-editor find (Ctrl+F) with match highlighting and navigation
- Find-and-replace (Ctrl+H)
- Default keybinding map wired through intent system
