# bench-rust-cli (wordtally)

A **Legion bench corpus fixture**: a small word/line/character counting CLI
written in plain Rust with **zero external dependencies**, so `cargo test
--offline` resolves and runs with no network access.

## Layout

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point: reads files named on the command line, prints a report |
| `src/cli.rs` | Flag parsing (`--lines`, `--words`, `--chars`, `--top N`) |
| `src/stats.rs` | Line/word/char counting and word-frequency tally |
| `src/report.rs` | Plain-text report rendering |

## Fixture rules

- No external crate dependencies — verification runs fully offline.
- This fixture hosts **bug-fix bench tasks**: it intentionally contains seeded
  defects, so *some unit tests fail at rest by design*. The crate always
  compiles cleanly; only test assertions fail. Do not "fix" the failures
  outside a bench run — task prompts and suite fingerprints depend on them.
- Keep `target/` and `Cargo.lock` out of version control (see `.gitignore`).
- The bench runner copies this directory to a temp checkout before every run;
  the committed files are templates and are never mutated in place.
