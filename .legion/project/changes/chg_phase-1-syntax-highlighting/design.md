---
{"body":"Source: FINISH-PLAN.md\n\nMake code look like code. The tree-sitter pipeline already produces highlight spans — wire them to egui color rendering.\n\n- Add tree-sitter grammars: TypeScript, Python, Go, C, JSON, TOML, Markdown, HTML/CSS, Bash → `Cargo.toml`, `legion-index`\n- Register grammars in plugin grammar registry, map file extensions to languages → `legion-index`\n- Pipe `highlight_spans` from `SyntaxTreeCache` through `ShellProjectionSnapshot` to desktop renderer → `legion-ui`, `legion-desktop`\n- Build default color scheme (dark + light) for highlight categories → `legion-desktop`\n- **Done when:** open .rs, .py, .ts, .go files and each shows colored syntax","changeId":"chg_phase-1-syntax-highlighting","dependencies":[{"mediaType":"text/markdown","path":".legion/project/specs/req_phase-1-syntax-highlighting.md","sha256":"sha256:8fa7c46468798081ee54f9561f908889ed2c0f626bf384b357e5fef51ce9bb96"}],"kind":"change-design","schemaVersion":"0.1.0","title":"Phase 1 implementation plan"}
---

# Phase 1 implementation plan

Source: FINISH-PLAN.md

Make code look like code. The tree-sitter pipeline already produces highlight spans — wire them to egui color rendering.

- Add tree-sitter grammars: TypeScript, Python, Go, C, JSON, TOML, Markdown, HTML/CSS, Bash → `Cargo.toml`, `legion-index`
- Register grammars in plugin grammar registry, map file extensions to languages → `legion-index`
- Pipe `highlight_spans` from `SyntaxTreeCache` through `ShellProjectionSnapshot` to desktop renderer → `legion-ui`, `legion-desktop`
- Build default color scheme (dark + light) for highlight categories → `legion-desktop`
- **Done when:** open .rs, .py, .ts, .go files and each shows colored syntax
