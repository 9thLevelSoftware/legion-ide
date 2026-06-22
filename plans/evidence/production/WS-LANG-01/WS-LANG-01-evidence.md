# WS-LANG-01 Rust LSP Substrate Evidence

## Workstream status

- Status: Complete (single-OS local validation; 3-OS hosted CI deferred — see LANG.12 note)
- Plan: `.superpowers/sdd/` (tasks 1–12 brief files)
- Master plan reference: `plans/legion-production-master-plan-v0.2.md` WS-LANG-01

## Product gate

- `PR-LANG-001` Rust language workflow: WS-LANG-01 substrate evidence complete; mock-gate tests across all four tiers (legion-lsp, legion-app, legion-ui, legion-desktop) implemented; real rust-analyzer 1.95.0 smoke validated on Windows (single OS)

## Evidence records

| Task | Description | Status | Evidence |
| --- | --- | --- | --- |
| LANG.01 | LSP stdio protocol types and session lifecycle skeleton | Done | `cargo test -p legion-lsp --test lifecycle_contract` (3 pass); `cargo test -p legion-lsp --test stdio_transport_contract` (2 pass); `cargo test -p legion-lsp --test registry_contract` (3 pass) |
| LANG.02 | Blocking pump and notification seam | Done | `cargo test -p legion-lsp --test pump_contract` (3 pass) |
| LANG.03 | rust-analyzer discovery and version-probe | Done | `cargo test -p legion-lsp --test discovery_contract` (5 pass) |
| LANG.04 | Gated binary download via capability broker | Done | `cargo test -p legion-app --test rust_analyzer_download_policy` (4 pass) |
| LANG.05 | Health record and provenance tracking | Done | `cargo test -p legion-protocol` (LSP health record tests, 6 pass in `lsp_server_health_record`); `cargo test -p legion-lsp` (lifecycle tests covering health state transitions) |
| LANG.06 | LSP health projection (protocol DTO + UI read projections) | Done | `cargo test -p legion-desktop --test language_health_view` (6 pass); `cargo test -p legion-ui` (23 pass, includes health projection coverage) |
| LANG.07 | Stale-snapshot detection and read-side contracts | Done | `cargo test -p legion-app --test language_stale_snapshot` (4 pass) |
| LANG.08 | Write-to-proposal routing (workspace edit → proposal) | Done | `cargo test -p legion-app --test language_edit_proposal_routing` (5 pass) |
| LANG.09 | Restart policy and backoff | Done | `cargo test -p legion-app --test language_restart_policy` (3 pass) |
| LANG.10 | LSP session orchestration — session handshake, doc-sync, read requests | Done | `cargo test -p legion-app --test rust_analyzer_session_handshake` (3 pass); `cargo test -p legion-app --test rust_analyzer_doc_sync` (3 pass); `cargo test -p legion-app --test rust_analyzer_read_requests` (4 pass) |
| LANG.11 | Ignored real rust-analyzer smoke tests (requires binary on PATH) | Done | `cargo test -p legion-lsp --test rust_analyzer_smoke -- --ignored` (1 pass: `rust_analyzer_initializes_and_emits_diagnostics`, 32s); `cargo test -p legion-app --test rust_analyzer_workflow -- --ignored` (1 pass: `rust_analyzer_full_workflow`, 0.8s) |
| LANG.12 | xtask rust-analyzer-smoke command; evidence and ledger; standing gates | Done | `cargo run -p xtask -- rust-analyzer-smoke` (both ignored tests pass, exit 0); 3-OS hosted CI deferred — see note below |

## Real rust-analyzer smoke results (LANG.11 / LANG.12)

Executed on: Windows 11 Pro, rust-analyzer 1.95.0, `stable-x86_64-pc-windows-msvc`

**legion-lsp smoke** (`cargo test -p legion-lsp --test rust_analyzer_smoke -- --ignored`):
- `rust_analyzer_initializes_and_emits_diagnostics` — PASS (32s)
- Initialize sequence: `Fresh` lifecycle, capabilities negotiated, `initialized` notification sent
- Diagnostic pump: at least 1 `textDocument/publishDiagnostics` notification received within 30s deadline
- Session alive check: `is_running()` true after smoke

**legion-app workflow** (`cargo test -p legion-app --test rust_analyzer_workflow -- --ignored`):
- `rust_analyzer_full_workflow` — PASS (0.8s)
- Completion, hover, definition, references, formatting, rename requests — all return well-formed JSON LSP responses against rust-analyzer 1.95.0
- Restart policy exercised: session stop/restart with backoff policy succeeds

## Standing gate results (2026-06-21, Windows 11)

| Gate | Command | Result |
| --- | --- | --- |
| fmt | `cargo fmt --all --check` | PASS (formatting issues fixed; 2 test files updated for SCALE.05 format change) |
| check-deps | `cargo run -p xtask -- check-deps` | PASS |
| docs-hygiene | `cargo run -p xtask -- docs-hygiene` | PASS |
| cargo check | `cargo check --workspace --all-targets` | PASS (28 crates) |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0 warnings) |
| tests | `cargo test --workspace --all-targets` | PASS (all pass; 2 ignored real-smoke tests skipped by default) |
| rust-analyzer-smoke | `cargo run -p xtask -- rust-analyzer-smoke` | PASS (both ignored smokes ran and passed against rust-analyzer 1.95.0) |
| deny | `cargo deny check` | PASS (advisories ok, bans ok, licenses ok, sources ok) |

### Pre-existing test regressions found and fixed (not WS-LANG-01-introduced)

Two test assertions in `crates/legion-desktop/tests/` were broken by WS-MANUAL-02 commit `6472a1e` (SCALE.05 — "add degraded large-file banner to desktop editor canvas"), which changed the banner row format from `"large-file degraded: bytes=..."` / `"capability reduced: {reason}"` to `"⚠ Large file (X MB) — some features disabled"` / `"  • {sanitized_reason}"` without updating the test assertions. These were surfaced and fixed as part of this gate run:

- `large_file_guardrails_degraded_banner_names_capability_reduction` (legion-desktop): updated assertion to match current banner format
- `projection_rendering_handles_empty_and_degraded_snapshots` (legion-desktop): updated assertion to match current sanitized-reason row format

### 3-OS hosted CI deferral (LANG.12)

No `.github/workflows/` directory exists in this repository. The repo convention is local xtask commands. The `cargo run -p xtask -- rust-analyzer-smoke` command is implemented and verified on Windows; 3-OS (Linux, macOS, Windows) hosted CI execution of the smoke is deferred pending CI infrastructure — consistent with the existing PR-REL-001 posture that states "no hosted CI workflow currently runs them."
