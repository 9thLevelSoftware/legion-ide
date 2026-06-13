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
| App command palette | Refresh Explorer | `F5` | Reload the workspace tree projection. |
| App command palette | Close Command Palette | `Esc` | Dismiss the foreground command palette. |
| Palette result confirm | Confirm selection | `Enter` | Confirm file, symbol, or recent-item palette results. |

## Mode controls

The product mode switch is currently exposed as labeled pills rather than keyboard bindings:

- `M` — Manual
- `A` — Assist
- `D` — Delegate
- `W` — Legion Workflows

## Where the labels come from

The projected labels are derived from the app command palette surface and the desktop projection tests.
If a label changes in code, update this reference in the same change so the docs stay aligned with the UI.
