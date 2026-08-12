---
{"body":"Source: FINISH-PLAN.md\n\nMake the terminal usable. Interpret CSI/SGR escapes so colored output and fullscreen programs work.\n\n- Implement VT100/xterm state machine: CSI (cursor, erase, scroll), SGR (colors, bold), DEC modes (alt screen, cursor visibility) → `legion-terminal`\n- Build terminal grid model: cell grid with character + attribute, dirty tracking → `legion-terminal`\n- Render grid in egui: monospace font with color attributes, cursor, selection → `legion-desktop`\n- Wire keyboard: translate egui keys to terminal escape sequences → `legion-desktop`\n- **Done when:** `ls --color`, `htop`, `vim` render correctly","changeId":"chg_phase-2-terminal-emulation","dependencies":[{"mediaType":"text/markdown","path":".legion/project/specs/req_phase-2-terminal-emulation.md","sha256":"sha256:155d1839de02a2f0cc7489dcb1f8009f5982a6560fc10a6f80ea7598343754e8"}],"kind":"change-design","schemaVersion":"0.1.0","title":"Phase 2 implementation plan"}
---

# Phase 2 implementation plan

Source: FINISH-PLAN.md

Make the terminal usable. Interpret CSI/SGR escapes so colored output and fullscreen programs work.

- Implement VT100/xterm state machine: CSI (cursor, erase, scroll), SGR (colors, bold), DEC modes (alt screen, cursor visibility) → `legion-terminal`
- Build terminal grid model: cell grid with character + attribute, dirty tracking → `legion-terminal`
- Render grid in egui: monospace font with color attributes, cursor, selection → `legion-desktop`
- Wire keyboard: translate egui keys to terminal escape sequences → `legion-desktop`
- **Done when:** `ls --color`, `htop`, `vim` render correctly
