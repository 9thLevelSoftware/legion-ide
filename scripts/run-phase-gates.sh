#!/usr/bin/env sh
# Runs the documented 20-gate repository sequence from AGENTS.md.

set -eu

if ! command -v cargo-deny >/dev/null 2>&1 && ! cargo deny --version >/dev/null 2>&1; then
  printf '%s\n' 'cargo-deny is required for the supply-chain policy gate.' >&2
  printf '%s\n' 'Install it with: cargo install cargo-deny --locked' >&2
  exit 127
fi

gate() {
  index="$1"
  name="$2"
  shift 2
  printf '[%s/20] %s\n' "$index" "$name"
  "$@"
}

gate 1 'Dependency policy gate' cargo run -p xtask -- check-deps
gate 2 'Documentation hygiene gate' cargo run -p xtask -- docs-hygiene
gate 3 'Claim-audit gate' cargo run -p xtask -- claim-audit
gate 4 'egui TextEdit boundary gate' cargo run -p xtask -- no-egui-textedit
gate 5 'Kanban backlog gate' cargo run -p xtask -- verify-kanban-backlog
gate 6 'Release pipeline dry-run gate' cargo run -p xtask -- release-pipeline --dry-run
gate 7 'Release pipeline verification gate' cargo run -p xtask -- verify-release-pipeline
gate 8 'Formatting gate' cargo fmt --all --check
gate 9 'Workspace check gate' cargo check --workspace --all-targets
gate 10 'Workspace test gate' cargo test --workspace --all-targets
gate 11 'Clippy gate' cargo clippy --workspace --all-targets -- -D warnings
gate 12 'Supply-chain policy gate' cargo deny check
gate 13 'Rust analyzer smoke gate' cargo run -p xtask -- rust-analyzer-smoke
gate 14 'Golden Path 1 gate' cargo run -p xtask -- golden-path-1
gate 15 'Golden Path 2 gate' cargo run -p xtask -- golden-path-2
gate 16 'Golden Path 3 gate' cargo run -p xtask -- golden-path-3
gate 17 'Golden Path 4 gate' cargo run -p xtask -- golden-path-4
gate 18 'Performance harness gate' cargo run -p xtask -- perf-harness
gate 19 'Performance harness verification gate' cargo run -p xtask -- verify-perf-harness
gate 20 'Update drill gate' cargo run -p xtask -- update-drill

printf '%s\n' 'All Legion IDE phase gates passed.'
