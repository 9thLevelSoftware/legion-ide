# Codebase Map Search Protocol

Generated: 2026-05-27T12:57:55.9718684-04:00
Analyzed commit: beb896492685fadbb4d1669250f0a5f5a145f613
Schema: 2.0

## Required Artifacts

- `.planning/CODEBASE.md`
- `.planning/codebase/index.jsonl`
- `.planning/codebase/symbols.json`
- `.planning/codebase/search.md`
- `.planning/config/directory-mappings.yaml`

## Query Planning

For a `/legion:map --query "..."` request, split the query into:

- terms: important nouns, verbs, feature names, and technology names
- path_hints: explicit file or directory names
- symbol_hints: structs, enums, traits, functions, commands, or crate names
- domain_hints: likely areas such as renderer, app, protocol, save, language, terminal, trust, AI, plugin, collaboration, remote, storage, security, or governance

## Retrieval Order

1. Search explicit path hints in `index.jsonl` and `symbols.json`.
2. Search symbol hints in `symbols.json`.
3. Search terms and aliases in `index.jsonl`.
4. Search `.planning/CODEBASE.md` section headings for broad architecture context.
5. Read the original source files for the top matches before planning, reviewing, or editing.

## Ranking

Rank matches by exact path/symbol hit, keyword overlap, same domain, risk level, fan-in relevance, and current source recency. Return at most five primary chunks and five read-next paths unless the caller requests broader coverage.

## Safety Rules

- Chunk summaries are not source of truth for code edits.
- Current source wins if it conflicts with this map.
- Do not cite stale map data as current without checking metadata and source fingerprint.
- Do not load the whole index when a targeted query is enough.

## Example

Query: `trust assisted ai proposal preview`

Expected high-value matches:

| Rank | Chunk | Path | Lines | Kind | Why it matched |
| --- | --- | --- | --- | --- | --- |
| 1 | `map:protocol-trust-ai:001` | `crates/legion-protocol/src/lib.rs` | 5112-6660 | module | Exact trust, assisted AI, permission, approval, checkpoint, proposal preview terms |
| 2 | `map:app-composition:001` | `crates/legion-app/src/lib.rs` | 7090-7630 | module | App composition owns projection and workflow wiring |
| 3 | `map:desktop-view:001` | `crates/legion-desktop/src/view.rs` | 1-330 | component | Desktop view displays trust and assistant rows |

Read next:

- `crates/legion-protocol/src/lib.rs` around trust/assisted-AI projection DTOs
- `crates/legion-app/src/lib.rs` around projection helpers and app workflow methods
- `crates/legion-desktop/src/view.rs` around trust and assistant rendering rows
