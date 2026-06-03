# Plan Phase 0: governance and CI truth lock.
# Runs the documented repository gate sequence from AGENTS.md and plans/dependency-policy.md.

$ErrorActionPreference = "Stop"

Write-Host "[1/6] Dependency policy gate"
cargo run -p xtask -- check-deps

Write-Host "[2/6] Formatting gate"
cargo fmt --all --check

Write-Host "[3/6] Workspace check gate"
cargo check --workspace --all-targets

Write-Host "[4/6] Workspace test gate"
cargo test --workspace --all-targets

Write-Host "[5/6] Clippy gate"
cargo clippy --workspace --all-targets -- -D warnings

Write-Host "[6/6] Supply-chain policy gate"
cargo deny check

Write-Host "All Legion IDE phase gates passed."
