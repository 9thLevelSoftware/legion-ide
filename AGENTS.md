# Repository Guidelines

## Project Overview

Legion IDE is a proprietary, privacy-first native Rust IDE. The workspace validates editor, workspace-mutation, UI, AI, and workflow architecture; it is not yet a general-availability product. Manual mode is the default and zero-egress; AI modes are opt-in, and workspace changes remain proposal-mediated. Keep product-readiness claims separate from accepted substrate evidence (`README.md`; `plans/product-readiness-ledger.md`).

This is an internal repository: do not redistribute or publish crates, open public issues or PRs, add credentials, or commit without applicable internal authorization (`CONTRIBUTING.md`; root workspace metadata). Never place private signing keys, certificates, tokens, or notarization credentials in the tree. Release descriptors remain `dry-run/no-production-signer` until real signing support is approved.

## Architecture & Data Flow

`legion-protocol` defines shared DTOs and contracts. `legion-ui` converts interaction into typed `CommandDispatchIntent` values and renders app-produced projections; it does not own editor sessions or write files. `legion-app` (`AppComposition`) coordinates policy and workflows. `legion-editor` owns editable buffers, while `legion-project` (`WorkspaceActor`) owns guarded workspace/filesystem mutation. `legion-desktop` renders the UI.

Route saves through `AppComposition::save_active_buffer` / `SaveWorkflowService` to `WorkspaceActor::save_file_with_proposal`. Preserve proposal review and fingerprint, snapshot, buffer-version, and workspace-generation preconditions; stale/conflict outcomes must not discard dirty editor text. Do not add direct UI, provider, or worker writes; non-atomic write fallback is fail-closed.

Provider and worker output is not authority: mutation requires the applicable proposal and workspace gates. Preserve Manual-mode egress policy and metadata-only retention defaults; raw traces require explicit consent. Observability rejects zero `CorrelationId`, nil `CausalityId`, and zero `EventSequence`. Preserve the 5 MiB text snapshot cache budget and use bounded `TextSnapshot` chunk APIs for large text rather than forcing full-string materialization. Treat Vim parsing as a pure action/intent stage; app dispatch implements effects. See `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`.

## Key Directories

- `crates/` — Rust product crates; key boundaries include `legion-protocol`, `legion-app`, `legion-ui`, `legion-desktop`, `legion-editor`, `legion-text`, and `legion-project`.
- `xtask/` — repository gates, policy checks, and workflow verification.
- `scripts/` — cross-platform phase-gate and packaging scripts.
- `fixtures/`, `evals/`, `training/` — deterministic evaluation data and optional harnesses.
- `docs/` and `plans/` — canonical documentation, architecture/dependency policies, readiness ledgers, and evidence. Start at `docs/INDEX.md`; use current ledgers, not historical assessments.
- `mockups/` — separate React/Vite design mockup, not the native desktop application.

## Development Commands

Run from the repository root:

```sh
cargo build --workspace
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- check-deps
cargo run -p legion-app -- .                 # CLI proof; supports :w and :q
cargo run -p legion-desktop -- --workspace . # native desktop
```

For focused integration coverage, target the relevant crate and behavior; for example: `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict`. Full local gates: `sh scripts/run-phase-gates.sh`; on Windows use `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-phase-gates.ps1`. These scripts require `cargo-deny` (`cargo deny check`). The operator runbook is the source for gate details and platform prerequisites.

## Code Conventions & Common Patterns

Use four-space Rust formatting and `rustfmt`; name modules, functions, and tests `snake_case`, types `CamelCase`. Propagate typed `Result` errors across crate boundaries; existing domain errors use `thiserror`. Use shared protocol DTOs for cross-crate contracts and inject service ports through explicit constructors rather than global lookup.

Keep state with its authority: UI projects and emits intents, app composition coordinates, editor owns buffers, and workspace owns filesystem writes. Preserve save fingerprints/versions/generations and fail-closed conflict handling. Prefer bounded text APIs on large files. Name the actual service runtime: app work includes supervised `std::thread` workers, so do not assume all workflows use Tokio.

Follow `plans/dependency-policy.md` and `cargo run -p xtask -- check-deps` for internal crate edges; in particular, `legion-editor` must not depend on `legion-project`. Update policy and enforcement together when an authorized dependency change requires it.

## Important Files

- `Cargo.toml` — workspace membership, shared dependencies, Rust edition/MSRV, and package metadata.
- `crates/legion-app/src/lib.rs` — application composition, intent dispatch, save workflow, and projections.
- `crates/legion-project/src/lib.rs` — workspace actor, guarded writes, and workspace errors.
- `crates/legion-ui/src/ui.rs` and `crates/legion-ui/src/vim_intent.rs` — projection/intent contracts and Vim action routing.
- `crates/legion-text/src/lib.rs` — text snapshots and bounded access.
- `docs/INDEX.md`, `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`, `docs/OPERATOR_RUNBOOK.md`, `plans/dependency-policy.md`, and `plans/product-readiness-ledger.md` — authoritative navigation, boundaries, gates, dependency rules, and product status.

## Runtime/Tooling Preferences

The Rust workspace uses edition 2024 and requires Rust/Cargo 1.92 or newer (`Cargo.toml`). Cargo is the product build/test tool. Install `cargo-deny` for the full local gate scripts. Python harnesses require Python 3.10 or newer; `evals/` and `training/` are separate workflows, with heavier dependencies used only for relevant evaluation/training modes. `mockups/` is a separate pnpm/Vite package; no Node version requirement is established here. Do not use Node or Bun as a prerequisite for the Rust IDE.

## Testing & QA

Put unit tests alongside source and integration tests in `crates/<crate>/tests/`. Name tests for observable behavior and expected outcome. Prefer isolated temporary directories, fixed/scripted transports, and committed replay fixtures; keep live-provider checks distinct and opt-in/credential-gated. When tests mutate process-global environment, isolate the test binary and guard shared state.

Useful focused commands include `cargo test -p xtask --test docs_hygiene`, `cargo test -p legion-agent --test agent_loop_integration basic_tool_use_loop_completes`, `python3 -m unittest evals.test_run_eval`, and `python3 -m unittest training.test_qlora_train`. CI runs workspace Rust checks across Linux, Windows, and macOS. Recorded provider/benchmark replays are deterministic; live-provider benchmarks are separate and non-gating. Windowed GUI E2E is hard-fail within its scheduled/manual workflow but is currently independent of PR gates; it is not interchangeable with headless smoke. No numerical code-coverage target is documented.

Active AI/provider, indexing, memory, and tracker crates require contract tests. New surfaces require an ADR, phase gate, dependency-policy entry, and contract tests. ADR-0046's freeze is retired, but `PR-VSC-002`, `PR-ENT-001`, and `PR-ENT-002` remain deferred until evidenced; check `cargo run -p xtask -- deferred-surfaces` and the current ledgers rather than treating historical reviews as current status. For acceptance, distinguish substrate validation from product-workflow readiness in `plans/product-readiness-ledger.md`. Internal reviews should state the affected authority boundary, tests/gates run, readiness/evidence updates, and GUI evidence; attach screenshots for visible desktop changes. Recent commits use imperative, issue-oriented subjects and describe the concrete behavior.
