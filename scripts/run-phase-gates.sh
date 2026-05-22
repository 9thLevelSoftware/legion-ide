#!/usr/bin/env sh
# Plan Phase 0: governance and CI truth lock.
# Runs the documented repository gate sequence from AGENTS.md and plans/dependency-policy.md.

set -eu

printf '%s\n' '[1/6] Dependency policy gate'
cargo run -p xtask -- check-deps

printf '%s\n' '[2/6] Formatting gate'
cargo fmt --all --check

printf '%s\n' '[3/6] Workspace check gate'
cargo check --workspace --all-targets

printf '%s\n' '[4/6] Workspace test gate'
cargo test --workspace --all-targets

printf '%s\n' '[5/6] Clippy gate'
cargo clippy --workspace --all-targets -- -D warnings

printf '%s\n' '[6/6] Supply-chain policy gate'
cargo deny check

printf '%s\n' 'All Devil IDE phase gates passed.'
