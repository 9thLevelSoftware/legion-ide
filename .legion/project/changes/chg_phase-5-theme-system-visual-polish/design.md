---
{"body":"Source: FINISH-PLAN.md\n\nDark/light themes, consistent syntax colors, rendering polish.\n\n- Define `ThemeDefinition` type (editor, syntax, UI chrome colors) → `legion-protocol`, `legion-ui`\n- Ship 2 built-in themes: dark and light → `legion-app`\n- Wire theme selection through settings, apply to egui + syntax → `legion-desktop`\n- Code minimap / overview ruler → `legion-desktop`\n- Tab bar polish: reordering, close buttons, overflow → `legion-desktop`\n- **Done when:** switch dark/light, all panels update, syntax colors match","changeId":"chg_phase-5-theme-system-visual-polish","dependencies":[{"mediaType":"text/markdown","path":".legion/project/specs/req_phase-5-theme-system-visual-polish.md","sha256":"sha256:79b9750dc507a581476f5a626bd2e6f84c7659bf32a320b95ea2f5251c4ac992"}],"kind":"change-design","schemaVersion":"0.1.0","title":"Phase 5 implementation plan"}
---

# Phase 5 implementation plan

Source: FINISH-PLAN.md

Dark/light themes, consistent syntax colors, rendering polish.

- Define `ThemeDefinition` type (editor, syntax, UI chrome colors) → `legion-protocol`, `legion-ui`
- Ship 2 built-in themes: dark and light → `legion-app`
- Wire theme selection through settings, apply to egui + syntax → `legion-desktop`
- Code minimap / overview ruler → `legion-desktop`
- Tab bar polish: reordering, close buttons, overflow → `legion-desktop`
- **Done when:** switch dark/light, all panels update, syntax colors match
