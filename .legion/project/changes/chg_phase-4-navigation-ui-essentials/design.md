---
{"body":"Source: FINISH-PLAN.md\n\nFile tree, find/replace, keybindings, persistent state.\n\n- Build file tree panel from workspace file listing APIs → `legion-ui`, `legion-desktop`\n- In-editor find (Ctrl+F) with match highlighting and navigation → `legion-editor`, `legion-desktop`\n- Find-and-replace (Ctrl+H) → `legion-editor`, `legion-desktop`\n- Default keybinding map wired through intent system → `legion-ui`\n- Serialize open tabs / recent files / window geometry / dock layout on close, restore on startup → `legion-storage`, `legion-app`\n- Simple settings panel → `legion-desktop`\n- **Done when:** navigate via tree, Ctrl+F finds text, close and reopen restores tabs","changeId":"chg_phase-4-navigation-ui-essentials","dependencies":[{"mediaType":"text/markdown","path":".legion/project/specs/req_phase-4-navigation-ui-essentials.md","sha256":"sha256:a6c4d364a257e165d1a18bc855c871a1419a1bf4f2eb395137d2f36d4c69e9b2"}],"kind":"change-design","schemaVersion":"0.1.0","title":"Phase 4 implementation plan"}
---

# Phase 4 implementation plan

Source: FINISH-PLAN.md

File tree, find/replace, keybindings, persistent state.

- Build file tree panel from workspace file listing APIs → `legion-ui`, `legion-desktop`
- In-editor find (Ctrl+F) with match highlighting and navigation → `legion-editor`, `legion-desktop`
- Find-and-replace (Ctrl+H) → `legion-editor`, `legion-desktop`
- Default keybinding map wired through intent system → `legion-ui`
- Serialize open tabs / recent files / window geometry / dock layout on close, restore on startup → `legion-storage`, `legion-app`
- Simple settings panel → `legion-desktop`
- **Done when:** navigate via tree, Ctrl+F finds text, close and reopen restores tabs
