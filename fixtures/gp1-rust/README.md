# GP-1 Smoke Fixture

This directory is a **smoke test fixture** used exclusively by the Legion IDE
GP-1 golden-path smoke (`cargo run -p xtask -- golden-path-1`).

## What this fixture is

A minimal Rust binary crate that the smoke test copies into a temporary
directory before every run. The smoke then drives the Legion IDE product APIs
against the copy — the committed files here are templates only and are never
mutated by the smoke.

## Contents

| File | Purpose |
|------|---------|
| `Cargo.toml` | Minimal crate manifest; no external dependencies |
| `src/main.rs` | Entry point; contains `SMOKE_MARKER_ALPHA` literal (search-step target) |
| `src/scratchpad.rs` | Edited at runtime by step s3 to introduce then fix a compile error |

## Fixture rules

- `cargo test` must pass on the committed source at rest (no compile errors).
- The committed source must contain the literal `SMOKE_MARKER_ALPHA` at least
  once (search-step target in step s4).
- `src/scratchpad.rs` is the only file the smoke edits at runtime; all other
  files are read-only from the smoke's perspective.
- Do **not** add external crate dependencies — the smoke runs with zero hosted
  egress and offline Cargo resolution.
