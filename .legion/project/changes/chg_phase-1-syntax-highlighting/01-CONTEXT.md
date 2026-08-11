# Phase 1: Multi-Language Syntax Highlighting

## Goal
Enable syntax highlighting for all major programming languages. The rendering pipeline from tree-sitter captures through to egui colored text is already fully wired — this phase extends it beyond the current Rust-only support.

## Current State (from code audit 2026-08-11)
- `legion-index/src/lib.rs:2656` — `tree_sitter_supports_path()` is hardcoded to `.rs` only
- `legion-app/src/lib.rs:11869` — `highlight_captures_from_text()` is hardcoded to `LanguageId("rust")`
- `Cargo.toml:62` — only `tree-sitter-rust = "0.24.2"` is a workspace dependency
- `legion-index/src/lib.rs:2732` — `rust_highlight_query()` is the only highlight query
- `legion-desktop/src/view.rs:2565` — `token_color()` already maps 10 token kinds to theme colors
- `legion-desktop/src/view.rs:2500` — `code_line_layout_job()` already renders colored spans via egui LayoutJob
- `legion-app/src/lib.rs:11886` — `tree_sitter_overlays_from_captures()` already converts captures to viewport overlays

## Existing Assets
- Plugin grammar registration system exists (`PLUGIN_TREE_SITTER_GRAMMAR_REGISTRY` at `legion-index/src/lib.rs:2673`) but is unused for built-in grammars
- `ViewportSemanticTokenKind` enum has 10 variants: Ident, Keyword, Type, String, Number, Comment, Punct, Function, Attribute, Error
- `TreeSitterHighlightCapture` struct includes `capture_name`, `line_number`, byte positions, and `token_kind`
- Theme color mapping in `token_color()` is complete for all 10 kinds

## Architecture Decision
Use built-in tree-sitter grammar crates (compile-time linked) rather than the plugin grammar registry (runtime WASM). The plugin system is for third-party extensions; core language support should be built-in for performance and reliability.

## Plans
- **01-01**: Multi-Language Grammar Support (Wave A, 3 tasks)
