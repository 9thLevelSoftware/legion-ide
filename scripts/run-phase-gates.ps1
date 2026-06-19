# Plan Phase 0: governance and local gate truth lock.
# Runs the documented repository gate sequence from AGENTS.md and plans/dependency-policy.md.

$ErrorActionPreference = "Stop"

try {
    cargo deny --version | Out-Null
} catch {
    Write-Error "cargo-deny is required for the supply-chain policy gate. Install it with: cargo install cargo-deny --locked"
    exit 127
}

Write-Host "[1/7] Dependency policy gate"
cargo run -p xtask -- check-deps
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[2/7] Documentation hygiene gate"
cargo run -p xtask -- docs-hygiene
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[3/7] Formatting gate"
cargo fmt --all --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[4/7] Workspace check gate"
cargo check --workspace --all-targets
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[5/7] Workspace test gate"
cargo test --workspace --all-targets
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[6/7] Clippy gate"
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[7/7] Supply-chain policy gate"
cargo deny check

Write-Host "All Legion IDE phase gates passed."
