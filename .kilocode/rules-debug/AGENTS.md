# Project Debug Rules (Non-Obvious Only)

- Save failures often return `Ok(AppSaveOutcome::Rejected(_))`; debug the contained `ProposalResponse` and verify dirty text/disk content were preserved.
- Expected save event order in app integration is `editor.transaction_applied` -> `proposal.created` -> `proposal.validated` -> `proposal.previewed` -> `proposal.applied` or `proposal.stale_rejected`/conflict events.
- A `FullCacheBudgetExceeded` result in the ignored 100MB editor performance workload is the documented current boundary for missing degraded/streaming large-file mode.
- `cargo run -p legion-app -- <path>` is a minimal interactive proof: it trusts the current directory and only reacts to `:w` and `:q`.
- If `xtask check-deps` fails for protocol symbols, it checks literal `struct`/`enum`/`trait` definitions in `crates/legion-protocol/src/lib.rs`; type aliases or reexports will not satisfy it.
- The cargo-deny baseline is warning-heavy in `deny.toml`; distinguish supply-chain warnings from hard Rust test/clippy failures.

