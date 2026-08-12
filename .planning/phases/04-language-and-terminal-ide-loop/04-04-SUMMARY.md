# Plan 04-04 Summary

Desktop language and terminal panels are wired as adapter-local projection views and command bridges. Product state remains in app/protocol projections.

## Verification

- `cargo test -p devil-desktop --test language_terminal_view -- --nocapture`
- `cargo check -p devil-desktop --all-targets`
