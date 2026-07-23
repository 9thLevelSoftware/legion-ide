# Legion Keyboard Reference

This page lists the shortcut labels currently projected by Legion.
Treat the labels as the product source of truth for the current profile and platform; the exact binding set can vary, but the projection should stay consistent with the command surface.

## Projected shortcut labels

| Surface | Action | Shortcut label | Notes |
| --- | --- | --- | --- |
| App command palette | Save All | `Ctrl+Shift+S` | Save every open tab through app authority. |
| Desktop projection row | Save all open files | `Ctrl+S` | Surface-specific label currently rendered in the desktop projection tests. |
| App command palette | Save Active Buffer | `⌘S` | Save the active tab through app authority. |
| App command palette | Close Active Tab | `⌘W` | Close the active tab through app authority. |
| App command palette | Reveal Active File in Explorer | `⇧⌘E` | Reveal the active file in the explorer. |
| App command palette | Refresh Explorer | `F5` | When **no** debug session and **no** launch configs are projected. |
| Debug idle + configs | Launch first config | `F5` | B17: zero-config start when configs exist and no session is active. |
| Debug session | Continue | `F5` | Live/fixture continue (non-blocking on live path; auto-poll drains stop). |
| Debug session | Stop / disconnect | `Shift+F5` | Tear down live adapter or exit fixture session. |
| Debug session | Step over | `F10` | |
| Debug session | Step into | `F11` | |
| Debug session | Step out | `Shift+F11` | |
| Debug | Toggle breakpoint at cursor | `F9` | Zero-based projected cursor line on the active buffer. |
| App command palette | Close Command Palette | `Esc` | Dismiss the foreground command palette. |
| Palette result confirm | Confirm selection | `Enter` | Confirm file, symbol, or recent-item palette results. |
| Completion popup | Navigate next | `↓` (Down Arrow) | Move selection down in the completion list. |
| Completion popup | Navigate previous | `↑` (Up Arrow) | Move selection up in the completion list. |
| Completion popup | Accept selected item | `Tab` / `Enter` | Insert the selected label through editor authority. |
| Completion popup | Dismiss | `Esc` | Close the popup without inserting. |
| Hover tooltip | Dismiss | `Esc` | Close the hover tooltip (re-opens only when new hover data arrives). |
| Editor | Go to definition | Command palette → `GoToDefinition` | Navigate to the definition site for the symbol under the cursor. |

## SCM diff review navigation

| Surface | Action | Shortcut label | Notes |
| --- | --- | --- | --- |
| SCM diff panel | Next Hunk | `]h` | Move focus to the next changed hunk. Projected from `GitNavNextHunk` intent. |
| SCM diff panel | Previous Hunk | `[h` | Move focus to the previous changed hunk. Projected from `GitNavPrevHunk` intent. |
| SCM diff panel | Next File | `]f` | Move focus to the next changed file. Projected from `GitNavNextFile` intent. |
| SCM diff panel | Previous File | `[f` | Move focus to the previous changed file. Projected from `GitNavPrevFile` intent. |

Navigation state (`focused_hunk_id`) is owned by the application layer and reflected in `GitProjection`; the desktop shell is projection-only.

## Mode controls

The product mode switch is currently exposed as labeled pills rather than keyboard bindings:

- `M` — Manual
- `A` — Assist
- `D` — Delegate
- `W` — Legion Workflows

## Where the labels come from

The projected labels are derived from the app command palette surface and the desktop projection tests.
If a label changes in code, update this reference in the same change so the docs stay aligned with the UI.
