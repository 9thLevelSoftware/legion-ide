---
{"body":"Source: FINISH-PLAN.md\n\nWire the LSP client to the editor for diagnostics, completions, and hover.\n\n- Auto-launch language server when workspace opens → `legion-app`\n- Route `publishDiagnostics` to inline error/warning markers → `legion-app`, `legion-desktop`\n- Trigger completion on typing / Ctrl+Space, show popup, insert on accept → `legion-app`, `legion-ui`, `legion-desktop`\n- Show hover on mouse hover → `legion-desktop`\n- Go-to-definition on Ctrl+Click / F12 → `legion-app`, `legion-desktop`\n- Inlay hints rendering → `legion-desktop`\n- **Done when:** open Rust project, see red squiggles on errors, get completions, hover shows types","changeId":"chg_phase-3-live-lsp-integration","dependencies":[{"mediaType":"text/markdown","path":".legion/project/specs/req_phase-3-live-lsp-integration.md","sha256":"sha256:7e4a0750e674fabdf0c50ec62d049a2477905e2ac3b55d3b1a27a8910b0152f4"}],"kind":"change-design","schemaVersion":"0.1.0","title":"Phase 3 implementation plan"}
---

# Phase 3 implementation plan

Source: FINISH-PLAN.md

Wire the LSP client to the editor for diagnostics, completions, and hover.

- Auto-launch language server when workspace opens → `legion-app`
- Route `publishDiagnostics` to inline error/warning markers → `legion-app`, `legion-desktop`
- Trigger completion on typing / Ctrl+Space, show popup, insert on accept → `legion-app`, `legion-ui`, `legion-desktop`
- Show hover on mouse hover → `legion-desktop`
- Go-to-definition on Ctrl+Click / F12 → `legion-app`, `legion-desktop`
- Inlay hints rendering → `legion-desktop`
- **Done when:** open Rust project, see red squiggles on errors, get completions, hover shows types
