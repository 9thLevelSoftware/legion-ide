# PR-13 — 3-OS Rust-Analyzer Smoke Evidence

## Packet status

- **Packet:** `PR-LANG-001` / `WS-LANG-01` GP-1 language evidence.
- **Recorded:** August 24, 2026.
- **Ledger effect:** None. `PR-LANG-001` remains `Substrate validated`.
- **Promotion:** Not claimed. This packet records the evidence contract and the
  local evidence already available; it does not promote a readiness row.

## Evidence already available

The existing `WS-LANG-01-evidence.md` packet records a real local
rust-analyzer 1.95.0 smoke on Windows 11:

- `cargo run -p xtask -- rust-analyzer-smoke` completed the ignored LSP and app
  smoke tests successfully.
- The LSP smoke covered initialization, capability negotiation, diagnostics,
  and session liveness.
- The app workflow covered completion, hover, definition, references,
  formatting, rename, and restart-policy behavior.

That is **single-OS local evidence**, not 3-OS product validation. No Linux or
macOS smoke result is inferred from the Windows run.

## Hosted GP-1 contract

The repository's `.github/workflows/legion-smoke.yml` defines the intended
hosted matrix:

| Dimension | Contract |
| --- | --- |
| Job | `GP-1 smoke (${{ matrix.os }})` |
| Matrix | `ubuntu-latest`, `windows-latest`, `macos-latest` |
| Smoke command | `cargo run -p xtask -- golden-path-1` |
| Rust-analyzer | Provisioned with `rustup component add rust-analyzer`; provisioning may skip the language steps with a warning |
| Artifact | `target/golden-path/gp1_report.toml`, uploaded as `gp1-report-${{ matrix.os }}` |
| Merge policy | Independent workflow; not a required PR merge check |

The workflow is deliberately independent and the repository does not contain
a committed hosted run ID or three-platform artifact bundle for this packet.
Therefore this PR does **not** claim a green 3-OS run, four consecutive green
scheduled runs, or owner sign-off.

## Companion product paths

The execute-plan sequence also records the product paths that the smoke packet
must exercise after integration:

- PR-11 commit `bf83528768ffec9f1404fab77f573d91ae1a94b6` adds F8/Shift+F8
  diagnostics navigation through the existing desktop actions.
- PR-12 commit `75d5362205eee178bf9383b7ebd351fe351179ec` makes rename, format,
  organize-imports, and code-action requests reachable through the existing
  proposal preview/apply path.

These companion commits are implementation references, not evidence that the
hosted 3-OS smoke has passed. The merged tree must rerun the relevant product
tests and GP-1 workflow before any promotion decision.

## Reproduction

For local real-server evidence, run with `rust-analyzer` available on `PATH`:

```text
cargo run -p xtask -- rust-analyzer-smoke
```

For the hosted-equivalent GP-1 workflow locally:

```text
cargo run -p xtask -- golden-path-1
```

Record the host OS, toolchain, rust-analyzer version, command result, and
generated report path. Do not combine results from different hosts into a
single green matrix claim unless the corresponding artifacts are retained.

## Remaining gate

Promotion remains governed by the existing ledger and operator runbook: the
3-OS smoke history must meet its stated bar (four consecutive green scheduled
runs or an owner sign-off naming the residual). This evidence packet does not
change `.github/workflows/legion-smoke.yml`, branch protection, or the ledger.
