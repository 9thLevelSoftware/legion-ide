# Runs the documented 20-gate repository sequence from AGENTS.md.

$ErrorActionPreference = "Stop"

try {
    cargo deny --version | Out-Null
} catch {
    Write-Error "cargo-deny is required for the supply-chain policy gate. Install it with: cargo install cargo-deny --locked"
    exit 127
}

function Invoke-Gate {
    param(
        [int] $Index,
        [string] $Name,
        [scriptblock] $Command
    )

    Write-Host "[$Index/20] $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

Invoke-Gate 1 "Dependency policy gate" { cargo run -p xtask -- check-deps }
Invoke-Gate 2 "Documentation hygiene gate" { cargo run -p xtask -- docs-hygiene }
Invoke-Gate 3 "Claim-audit gate" { cargo run -p xtask -- claim-audit }
Invoke-Gate 4 "egui TextEdit boundary gate" { cargo run -p xtask -- no-egui-textedit }
Invoke-Gate 5 "Kanban backlog gate" { cargo run -p xtask -- verify-kanban-backlog }
Invoke-Gate 6 "Release pipeline dry-run gate" { cargo run -p xtask -- release-pipeline --dry-run }
Invoke-Gate 7 "Release pipeline verification gate" { cargo run -p xtask -- verify-release-pipeline }
Invoke-Gate 8 "Formatting gate" { cargo fmt --all --check }
Invoke-Gate 9 "Workspace check gate" { cargo check --workspace --all-targets }
Invoke-Gate 10 "Workspace test gate" { cargo test --workspace --all-targets }
Invoke-Gate 11 "Clippy gate" { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-Gate 12 "Supply-chain policy gate" { cargo deny check }
Invoke-Gate 13 "Rust analyzer smoke gate" { cargo run -p xtask -- rust-analyzer-smoke }
Invoke-Gate 14 "Golden Path 1 gate" { cargo run -p xtask -- golden-path-1 }
Invoke-Gate 15 "Golden Path 2 gate" { cargo run -p xtask -- golden-path-2 }
Invoke-Gate 16 "Golden Path 3 gate" { cargo run -p xtask -- golden-path-3 }
Invoke-Gate 17 "Golden Path 4 gate" { cargo run -p xtask -- golden-path-4 }
Invoke-Gate 18 "Performance harness gate" { cargo run -p xtask -- perf-harness }
Invoke-Gate 19 "Performance harness verification gate" { cargo run -p xtask -- verify-perf-harness }
Invoke-Gate 20 "Update drill gate" { cargo run -p xtask -- update-drill }

Write-Host "All Legion IDE phase gates passed."
