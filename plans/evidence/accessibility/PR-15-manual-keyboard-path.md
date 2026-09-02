# PR-15 Accessibility Evidence

## Probe status

| Platform | Repeatable probe | Observation |
| --- | --- | --- |
| Windows | `scripts/a11y-uia-walk.ps1` | Observed when run against a live desktop window; the committed script is the source of truth. |
| macOS | No committed OS-tree probe | Unobserved. |
| Linux | No committed OS-tree probe | Unobserved. |

Run `scripts/a11y-platform-probe.sh` to produce a machine-readable status. It
executes the committed Windows UIA walk (using `LEGION_A11Y_PROCESS` or the
default `legion-desktop`) and deliberately exits non-zero for macOS/Linux rather
than implying an observation that was not made.

## Manual keyboard-only path

Renderer-backed certification lives in `crates/legion-desktop/tests/keyboard_nav.rs`
and `crates/legion-desktop/tests/accessibility.rs`. Those tests drive
`DesktopEframeApp` with egui key events (not AccessKit roles alone). They are
not a human OS-level walk and not an NVDA/VoiceOver/Orca transcript.

### Certified (renderer keymap or command palette)

| Route | Gesture | Test |
| --- | --- | --- |
| Product mode switch | Tab/arrows/Enter, Confirm/Escape | `product_mode_switch_*`, `product_mode_escalation_*` |
| Command palette | Ctrl/Cmd+Shift+P | `command_palette_keyboard_commits_staged_changes`; `keyboard_only_operation_opens_the_command_palette` (Ctrl/Cmd+P file palette) |
| Workspace search | Ctrl/Cmd+Shift+F | `ctrl_shift_f_opens_workspace_search_palette` |
| Active-file search | Ctrl/Cmd+F | published `ToggleFindBar` keymap; palette Search mode |
| Go to definition | F12 | `f12_on_the_open_editor_requests_go_to_definition` |
| Git: Stage Focused Hunk | Ctrl/Cmd+Shift+G and palette `Git: Stage Focused Hunk` | `ctrl_shift_g_stages_the_focused_hunk` |
| Git: Commit Staged Changes | command palette `git commit <message>` then Enter | `command_palette_keyboard_commits_staged_changes` |
| Problems next/prev | F8 / Shift+F8 | `t4_problem_*` |

Use the published palette/keymap to reach those routes. `Git: Stage Focused Hunk` from its published Ctrl/Cmd+Shift+G binding stages the focused unstaged hunk.

### Residual (explicitly cut from default keymap)

These stay typed-shell or operand-only. They are named so they are not mistaken
for certified keyboard routes:

| Route | Why it remains residual |
| --- | --- |
| `:search-workspace <query>` | Superseded for daily use by Ctrl/Cmd+Shift+F. The colon command remains as a typed-intent contract. |
| `:definition <byte-offset>` | Superseded for daily use by F12. The colon command remains as a typed-intent contract. |
| `:git-nav-next-hunk` / `:git-nav-prev-hunk` / `:git-nav-next-file` / `:git-nav-prev-file` | No default keymap chord. Focusing a hunk still uses the typed shell (or GitNav intents). Staging after focus is certified. |
| `:git-stage-hunk <hunk-id>` | Operand requires an explicit hunk id. The focused-hunk keymap is the pointer-free product path. |
| `:term-launch <command>` | Operand requires a program string. Not a default keymap. No renderer-backed focus check is claimed. |
| `:test-refresh` / `:test-run` / `:test-run-group` | Typed shell; not palette entries. |
| `:format` / `:rename` / `:organize-imports` / `:code-action` | Proposal-mediated typed shell. Format/rename/organize also have keymap/palette entries (`Shift+Alt+F`, `F2`, `Ctrl+Shift+O`). |

The desktop accessibility tests still do not verify a human OS-level keyboard
walk or an NVDA/VoiceOver/Orca transcript. Those are GAP-05.2–05.4.

## Acceptance boundary

PR-UI-001 remains Substrate validated until each supported OS has a committed,
repeatable OS-tree observation and a real screen-reader transcript. This packet
does not promote the ledger or claim macOS/Linux coverage.
