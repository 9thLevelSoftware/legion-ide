# GAP-09.3 — Lexical indexer off the file-open path

**Date:** 2026-09-01  
**Wave:** 2 proof surface  
**Task:** GAP-09.3

## What this is

`AppComposition::bind_opened_file` no longer calls
`LanguageToolingWorkflow::refresh_retrieval_document`. Opening a file binds the
editor buffer and tab state, then returns. `LexicalIndexer::index_document`
runs later, on demand:

- language-read (hover, completion, outline, …)
- Delegate retrieval
- symbol palette refresh

Save, first viewport projection, and typing do not wait on the indexer.

`p8.startup` ceiling in `xtask/src/perf_workloads.rs` dropped from 30 s to 10 s
because the ~3.3 s indexer cost is gone. 10 s is still a catastrophe guard for
cold `open_workspace`, not a 1 s target. Re-adding the indexer to open is
caught by `open_save_and_first_projection_do_not_wait_on_lexical_indexer`.

## Local `product_perf` (debug, this machine, `--skip-fixture-100k`)

Reference document: `crates/legion-app/src/lib.rs` (~1.5 MB, 36097 lines).

| Phase | Before (P8.F4 warm) | After |
| --- | ---: | ---: |
| `AppComposition::new` | 0.2 ms | 1.7 ms |
| `open_workspace` | 343.8 ms | 491.1 ms |
| `open_file` | 3268.3 ms | 42.9 ms |
| first projection | 0.1 ms | 1.1 ms |
| open-to-ready | 3612.4 ms | 536.8 ms |

`p8.input_to_paint` on the same run: p50=4.180 ms / p95=4.524 ms (ADR-0048
16/32 ms). Typing is not paying the indexer.

The `--skip-fixture-100k` run exited 3 because that row is unmeasured by flag.
`p8.legion_repo` search was 177 s (still over the 120 s product-search budget).
That is not this task.

## What this is not

- Not background/async indexing
- Not a 1 s open-to-ready target
- Not GAP-01.2 windowed GUI
- Not arming `p8.legion_repo` search
- Not a ledger promotion of PR-UI-001/002

## Verification

```text
cargo test -p legion-app --test language_tooling_workflow open_save_and_first_projection_do_not_wait_on_lexical_indexer
cargo test -p legion-app --test palette palette_symbol_mode_opens_symbol_location
cargo test -p legion-app --test language_tooling_workflow
cargo test -p xtask --test perf_workloads
cargo clippy -p legion-app --all-targets -- -D warnings
cargo run -p xtask -- extract-before-modify
```

Ledger row statuses are unchanged.
