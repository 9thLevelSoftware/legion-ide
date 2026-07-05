#!/usr/bin/env sh
# Plan Phase 0: governance and local gate truth lock.
# Runs the documented repository gate sequence from AGENTS.md and plans/dependency-policy.md.

set -eu

if ! command -v cargo-deny >/dev/null 2>&1 && ! cargo deny --version >/dev/null 2>&1; then
  printf '%s\n' 'cargo-deny is required for the supply-chain policy gate.' >&2
  printf '%s\n' 'Install it with: cargo install cargo-deny --locked' >&2
  exit 127
fi

printf '%s\n' '[1/8] Dependency policy gate'
cargo run -p xtask -- check-deps

printf '%s\n' '[2/8] Documentation hygiene gate'
cargo run -p xtask -- docs-hygiene

printf '%s\n' '[3/8] Claim-audit gate'
cargo run -p xtask -- claim-audit

printf '%s\n' '[4/8] Formatting gate'
cargo fmt --all --check

printf '%s\n' '[5/8] Workspace check gate'
cargo check --workspace --all-targets

printf '%s\n' '[6/8] Workspace test gate'
cargo test --workspace --all-targets

printf '%s\n' '[7/8] Clippy gate'
cargo clippy --workspace --all-targets -- -D warnings

printf '%s\n' '[8/8] Supply-chain policy gate'
cargo deny check

printf '%s\n' 'All Legion IDE phase gates passed.'
