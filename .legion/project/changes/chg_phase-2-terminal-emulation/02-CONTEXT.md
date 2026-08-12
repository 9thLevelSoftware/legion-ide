# Phase 2 Context — Terminal Emulation

## Goal
Make the terminal usable by interpreting VT100/xterm escape sequences so colored output and fullscreen programs work. Currently the PTY works but raw CSI/SGR escape codes pass through as visible garbage text.

## Success Criteria
- `ls --color` shows colored file listings
- `htop` renders a fullscreen colored interface
- `vim` is navigable (cursor movement, insert mode, screen redraws)
- Arrow keys, Home/End, and function keys work in interactive programs

## Current State

### legion-terminal (2,659 source lines)
- **lib.rs**: Full PTY lifecycle (launch, input, resize, poll_output, close, kill). Credential redaction via `redact_secrets()`. FakePty test harness.
- **osc.rs**: OSC 7/133 shell metadata parsing. `split_visible_rows()` handles \r, \n, and CSI cursor-position (H/f) but passes all other CSI sequences through untouched as raw text.
- **grid.rs**: Renderer-friendly grid — sequential text rows from `TerminalPanelProjection`. No per-cell attributes.
- **session.rs**: Per-session metadata (cwd, exit code, boundary marker).
- **conpty.rs**: Windows ConPTY parity metadata.

### legion-desktop terminal rendering
- **view/terminal_panel.rs**: `TerminalPanelRenderModel` built from protocol projection — status labels, text-only grid.
- **view.rs:render_terminal_stream()** (line 3991): Renders grid as egui Grid widget — sequence number, stream label, plain text payload, badges. Uses `render_terminal_payload()` for URL detection only.
- **view/interactive_fields.rs:render_terminal_input_line()** (line 89): Simple `TextEdit::singleline` — sends raw text + newline. No escape sequence translation for special keys.

### legion-protocol
- `TerminalOutputRowProjection`: `redacted_payload: String` — plain text, no cell attributes.
- `TerminalPanelProjection`: Vec of output rows + scrollback + search projections.

## What's Missing
1. **No VT100/xterm escape interpreter** — CSI sequences for cursor movement, erase, scroll, and SGR for colors/attributes are not interpreted
2. **No cell grid model** — Terminal is a list of text rows, not a 2D grid of cells with per-cell attributes
3. **No DEC private modes** — No alt screen buffer (vim), no cursor visibility toggle, no application cursor keys
4. **No keyboard translation** — Arrow keys, function keys, Home/End produce no terminal escape sequences

## Existing Assets
- `split_visible_rows()` in osc.rs already handles basic CSI cursor-position (H/f) sequences
- `render_terminal_payload()` has URL segment detection that can coexist with colored rendering
- FakePty test harness enables thorough unit testing of the emulator
- Credential redaction pipeline is in place and must be preserved

## Architecture Decision
The VT100 emulator will be a new module `vt100.rs` in `legion-terminal`. It processes already-redacted output bytes and maintains a 2D cell grid with per-cell attributes. The emulator sits after credential redaction in the pipeline. The desktop renderer reads the cell grid and paints colored monospace text via egui.

## Plan Structure
| Plan | Wave | Description | Agent |
|------|------|-------------|-------|
| 02-01 | 1 | VT100 state machine + cell grid model | engineering-senior-developer |
| 02-02 | 2 | Pipeline integration + desktop rendering + keyboard | engineering-senior-developer |
