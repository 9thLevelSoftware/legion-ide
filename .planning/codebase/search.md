# Legion Codebase Search Protocol

Required artifacts:

- `.planning/CODEBASE.md`
- `.planning/codebase/index.jsonl`
- `.planning/codebase/symbols.json`
- `.planning/codebase/search.md`
- `.planning/config/directory-mappings.yaml`

## Query Planning

For a query, extract:

- Terms: important nouns, verbs, feature names, and technology names.
- Path hints: explicit files or directories.
- Symbol hints: structs, enums, traits, functions, crates, or commands.
- Domain hints: UI, editor, workspace, save, security, storage, index, AI, terminal, remote, planning, governance.

## Retrieval Order

1. Search explicit path hints in `index.jsonl` and `symbols.json`.
2. Search symbol hints in `symbols.json`.
3. Search terms and aliases in `index.jsonl`.
4. Search section headings and risk entries in `CODEBASE.md`.
5. Read the original source files for the top matches before planning, reviewing, or editing.

## Ranking

Rank matches by exact path/symbol match first, then keyword/alias overlap, domain relevance, risk level, and high fan-in relevance.

Return at most 5 primary chunks unless the caller explicitly requests broader coverage.

## Safety Rules

- Treat map chunks as navigation metadata, not source of truth for code edits.
- If source files conflict with the map, source files win and `/legion:map --refresh` should be run.
- Do not load the whole index into implementation prompts when a targeted query is enough.
- Do not cite stale map data as current without checking metadata freshness.

## Example

Query:

```text
/legion:map --query "GUI renderer projection boundary"
```

Example result format:

| Rank | Chunk | Path | Lines | Kind | Why it matched |
| --- | --- | --- | --- | --- | --- |
| 1 | `map:ui-projection:001` | `crates/devil-ui/src/ui.rs` | 1-180 | module | exact UI/projection boundary match |
| 2 | `map:app-projection:001` | `crates/devil-app/src/lib.rs` | 3899-3968 | module | app builds projection snapshots consumed by UI |
| 3 | `map:renderer-adr:001` | `plans/adrs/ADR-0002-ui-editor-rendering.md` | 1-220 | doc | renderer decision context |

Read next:

- `crates/devil-ui/src/ui.rs` lines 1-460
- `crates/devil-app/src/lib.rs` lines 3899-4145
- `plans/adrs/ADR-0002-ui-editor-rendering.md`
