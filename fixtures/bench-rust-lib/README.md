# bench-rust-lib (miniconf)

A **Legion bench corpus fixture**: a tiny INI-style configuration parser
library written in plain Rust with **zero external dependencies**, so
`cargo test --offline` resolves and runs with no network access.

## Grammar

One entry per line:

```
# comment            ignored, as are blank lines
[section]            starts a new section
key = value          assigns within the current section
```

Keys assigned before any `[section]` header land in the root section, whose
name is the empty string.

## Layout

| File | Purpose |
|------|---------|
| `src/lib.rs` | Public API: `Config`, `Value`, re-exported `ParseError` |
| `src/parser.rs` | Line-oriented parser for the grammar above |
| `src/error.rs` | `ParseError` enum with `Display`/`Error` impls |
| `tests/parse_basic.rs` | Integration tests for parsing and lookup |

## Fixture rules

- No external crate dependencies — verification runs fully offline.
- `cargo test` must pass on the committed source at rest (this fixture hosts
  test-add / refactor / feature bench tasks, all of which start green).
- Keep `target/` and `Cargo.lock` out of version control (see `.gitignore`).
- The bench runner copies this directory to a temp checkout before every run;
  the committed files are templates and are never mutated in place.
