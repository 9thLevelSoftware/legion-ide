# Repository Guidelines synthesis plan

## Context
The requested deliverable is a replacement root `D:/legion-ide/AGENTS.md`, titled `Repository Guidelines`, using the eight requested Markdown sections. It must synthesize parallel research of core source, tests, configuration/build tooling, and scripts/documentation into concise, practical instructions for AI coding assistants. The current session is read-only plan mode: research and this session-local plan are permitted; the root file is written only after execution approval.

## Already completed
Read the supplied root repository guidelines, the `orchestrate-omp` and `research` skills, and the repository root listing. Confirmed the presence of `crates/`, `xtask/`, `scripts/`, `docs/`, `plans/`, `fixtures/`, `evals/`, `training/`, `mockups/`, root Cargo manifests, `pyproject.toml`, and root documentation. No repository files or system state have been changed. Do not repeat initial directory discovery or read skills again.

### Verified evidence recorded during research
- Root `AGENTS.md:1–39` has five legacy sections; the replacement must reorganize its material instructions into the eight requested headings, not append another guide.
- `docs/INDEX.md:3,23–31,48–60,73–77` makes the documentation index the canonical entry point, identifies authority/runbook/privacy documents, and distinguishes historical evidence from current readiness.
- `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md:5–97` explicitly forbids UI ownership of editor/session state, direct UI/provider/worker writes, unsafe non-atomic fallback, and unconsented raw training payload retention.
- `docs/OPERATOR_RUNBOOK.md:7–18` verifies `cargo run -p xtask -- docs-hygiene`, `check-deps`, `claim-audit`, and the standard Cargo gates. Lines 22–39 distinguish independent golden-path/windowed GUI workflows from standing PR gates; GUI failure remains hard-fail.
- `docs/OPERATOR_RUNBOOK.md:57–97` says ADR-0046 is retired but PR-VSC-002, PR-ENT-001, and PR-ENT-002 remain deferred; `cargo run -p xtask -- deferred-surfaces` enforces evidence-based promotion.
- `plans/product-readiness-ledger.md:12–26,55–59` distinguishes substrate acceptance from product workflow validation and preserves extension/remote/collaboration cut lines. Do not turn historical pass counts or current substrate tests into product-ready claims.


## Approach
### 1. Research four independent slices concurrently
Run one `task` call containing four read-only `scout` agents. All agents must read actual relevant files, return repository-path and symbol/section evidence, skip builds/tests/formatters/installations, avoid accessing secrets, and neither edit files nor spawn children. Do not search for additional agent/context files.

- **Core source:** Discover source entry points and authority boundaries under `crates/`; identify app composition, desktop/UI rendering, protocol contracts, editor/text state, workspace save flow, error handling, async execution, injection patterns, and state ownership. Validate the supplied guidelines against current source rather than repeating them. Exact source filenames/symbols beyond the supplied `AppComposition::save_active_buffer`, `SaveWorkflowService`, `WorkspaceActor::save_file_with_proposal`, `CommandDispatchIntent`, and `VimState` are unverified — confirm first. Return a small set of concrete examples, not a crate inventory.
- **Tests:** Discover `crates/*/tests/`, representative inline Rust tests, `xtask/tests/`, `evals/`, and `training/`. Read representative behavior tests and relevant CI workflows discovered under `.github/workflows/`. Identify actual test frameworks, focused commands, fixtures, CI/provider isolation, GUI verification expectations, and any documented coverage requirement. Do not invent a numerical coverage target.
- **Configurations/build:** Read `Cargo.toml`, `pyproject.toml`, `deny.toml`, `xtask/Cargo.toml`, and `mockups/package.json`; discover related build/runtime configuration only as needed. Verify Rust edition/MSRV, workspace dependency/lint patterns, Cargo runtime/tooling requirements, Python dependencies, and the mockup package-manager distinction. Discover and read `xtask` dispatch before documenting its commands. No package installation or lockfile modification.
- **Scripts/docs:** Read relevant sections of `docs/INDEX.md`, `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`, `docs/OPERATOR_RUNBOOK.md`, `plans/product-readiness-ledger.md`, `plans/phase-status-ledger.md`, `plans/dependency-policy.md`, `README.md`, `CONTRIBUTING.md`, and `scripts/run-phase-gates.ps1` / `scripts/run-phase-gates.sh`. All named files except the supplied guidelines and listed root-level paths are unverified — confirm before opening. Verify development commands, gate prerequisites, private-repository restrictions, readiness/deferred-surface caveats, and current versus historical documentation. Coordinate scripts/docs evidence with test and build agents rather than duplicating broad investigation.

### 2. Resolve findings into one authoritative document
Collect all four reports. Inspect the cited source sections for load-bearing facts: runtime versions, development command syntax, save authority, UI ownership, AI privacy defaults, and CI/gate scope. When reports conflict, prefer current source/manifests/dispatchers for behavior and current operator/readiness documentation for policy; mark historical claims as historical or omit them. If a fact cannot be verified, omit it from the final guidance rather than turning it into an asserted requirement. Preserve applicable internal-only, no-credential, and no-unauthorized-commit restrictions from the existing supplied guidelines.

Use exactly this document structure:

```markdown
# Repository Guidelines

## Project Overview
## Architecture & Data Flow
## Key Directories
## Development Commands
## Code Conventions & Common Patterns
## Important Files
## Runtime/Tooling Preferences
## Testing & QA
```

Write approximately 900–1,300 words maximum, preferably shorter when evidence permits. Use short bullets and one compact shell-command block; every section must contain actionable, evidence-backed content. Put source paths inline beside facts rather than adding a bibliography. Avoid obvious inventories, unsupported claims, marketing descriptions, duplicate rules, and speculative future behavior.

- **Project Overview:** State the product purpose and private/proprietary status, with no unsupported readiness claims.
- **Architecture & Data Flow:** Describe product layering and the actual input/intent/snapshot path. Explicitly state mutation ownership and the proposal-mediated save path with its stale/conflict behavior. Include only verified high-impact AI/privacy and memory/streaming constraints.
- **Key Directories:** Summarize grouped product crates, supporting tooling, fixtures/evaluations, and authoritative docs/plans. Clearly distinguish `mockups/` from the shipped Rust UI if confirmed.
- **Development Commands:** Include verified root-Cargo build/check/test/fmt/clippy commands, the actual CLI command and interaction limitation, the discovered desktop launch command, one real focused integration-test example, and the Windows phase-gate command plus Unix counterpart. Exact desktop syntax and focused test identity are unverified — confirm first.
- **Code Conventions & Common Patterns:** Rust formatting/naming, existing focused error types, observed async and dependency-injection patterns, explicit state ownership, workspace dependency policy, and forbidden dependency direction if confirmed. Describe real patterns rather than prescribing a new abstraction.
- **Important Files:** Select entry points/configs/authority modules with a concrete explanation; do not repeat the directory list. Exact source entry points are unverified — confirm first.
- **Runtime/Tooling Preferences:** Verified Rust edition/MSRV, Cargo and cargo-deny requirements, Python tooling where used, platform prerequisites, and the mockup-only package manager where supported by files. Never imply a Node/Bun runtime is required for the Rust application.
- **Testing & QA:** Actual framework conventions and isolated behavioral test naming; contract/gate expectations; deterministic fixtures and opt-in provider benches; actual GUI evidence requirements; documented coverage policy only if one exists. Explain the limited synthetic-performance override only if confirmed relevant.

### 3. Freeze the execution specification before proposing approval
While still in plan mode, append the complete researched replacement `AGENTS.md` content to this same canonical plan under `## Exact replacement document`. Update the preceding research steps to record completed evidence and replace every load-bearing unverified placeholder with the observed choice. This makes approval executable without another design/research pass.

Then submit the plain slug `repository-guidelines` to `xd://propose`. Do not write root `AGENTS.md` before approval. Do not request approval through prose or `ask`.

### 4. Write the root document after approval
Reread root `AGENTS.md` directly; it is already known to exist and must not be rediscovered via search. Replace only `D:/legion-ide/AGENTS.md` with the exact replacement document embedded in this plan, using `write` because this is an intentional whole-file synthesis. If the root document has acquired new applicable instructions since research, retain those instructions within the requested eight sections before writing; do not discard user changes. Do not modify source, tests, verification assets, manifests, or unrelated documentation.

## Verification
Research-phase verification is read-only: confirm each documented command against its manifest, dispatcher, script, or first-party runbook; confirm behavior descriptions against actual source and representative behavior tests. Do not execute cargo commands during plan mode because they can modify the working tree/system.

After the root replacement, reread `AGENTS.md` and verify the exact title, all eight headings in the requested order, concrete architecture/pattern guidance, real file paths, and commands without unverified placeholders. Run the discovered applicable documentation-hygiene gate exactly as recorded in the finalized plan; its dispatcher syntax is unverified — confirm first. If that gate fails, fix the new document only for violations it introduces; never change tests, allowlists, or verification assets to make it pass. Report unrelated baseline failures without altering them. A documentation-only change requires no full Rust test/build or GUI launch; the documentation gate and direct document inspection are the applicable exercised proof.

## Assumptions & contingencies
- Replace the existing root guidelines rather than create a second file, preserving material repository restrictions while reorganizing to the user-requested sections.
- Keep the document concise and oriented toward assistant implementation work; no new workflow requirements or coverage thresholds are invented.
- If agents disagree, inspect the owning primary source and commit that result in the plan; unresolved claims are omitted.
- If a required gate cannot run because a documented tool is absent, do not install it implicitly; record the exact missing prerequisite and complete direct structural/content verification instead.
- Paths from the supplied guidelines can be stale. Confirm discovered paths before writing them, and use current source for architecture rather than promoting historical plans to current fact.
## Exact replacement document

# Repository Guidelines

## Project Overview

Legion IDE is a proprietary, privacy-first native Rust IDE. The workspace validates editor, workspace-mutation, UI, AI, and workflow architecture; it is not yet a general-availability product. Manual mode is the default and zero-egress; AI modes are opt-in, and workspace changes remain proposal-mediated. Keep product-readiness claims separate from accepted substrate evidence (`README.md`; `plans/product-readiness-ledger.md`).

This is an internal repository: do not redistribute or publish crates, open public issues or PRs, add credentials, or commit without applicable internal authorization (`CONTRIBUTING.md`; root workspace metadata).

## Architecture & Data Flow

`legion-protocol` defines shared DTOs and contracts. `legion-ui` converts interaction into typed `CommandDispatchIntent` values and renders app-produced projections; it does not own editor sessions or write files. `legion-app` (`AppComposition`) coordinates policy and workflows. `legion-editor` owns editable buffers, while `legion-project` (`WorkspaceActor`) owns guarded workspace/filesystem mutation. `legion-desktop` renders the UI.

Route saves through `AppComposition::save_active_buffer` / `SaveWorkflowService` to `WorkspaceActor::save_file_with_proposal`. Preserve proposal review and fingerprint, snapshot, buffer-version, and workspace-generation preconditions; stale/conflict outcomes must not discard dirty editor text. Do not add direct UI, provider, or worker writes.

Provider and worker output is not authority: mutation requires the applicable proposal and workspace gates. Preserve Manual-mode egress policy and metadata-only retention defaults; raw traces require explicit consent. For large text, use bounded `TextSnapshot` chunk APIs rather than forcing full-string materialization. Treat Vim parsing as a pure action/intent stage; app dispatch implements effects. See `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`.

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

Useful focused commands include `cargo test -p xtask --test docs_hygiene`, `cargo test -p legion-agent --test agent_loop_integration basic_tool_use_loop_completes`, `python3 -m unittest evals.test_run_eval`, and `python3 -m unittest training.test_qlora_train`. CI runs workspace Rust checks across Linux, Windows, and macOS. Recorded provider/benchmark replays are deterministic; live-provider benchmarks are separate and non-gating. Windowed GUI E2E is hard-fail within its scheduled/manual workflow but is currently independent of PR gates; it is not interchangeable with headless smoke. No numerical code-coverage target is documented. For status and acceptance, distinguish substrate validation from product-workflow readiness in `plans/product-readiness-ledger.md`.

## Research evidence and finalization

Research was completed in four concurrent read-only scout slices (`CoreSource`, `TestPatterns`, `ConfigBuild`, `ScriptsDocs`). Primary evidence was checked in current source/manifests/docs: `Cargo.toml`; `crates/legion-app/src/lib.rs`; `crates/legion-project/src/lib.rs`; `crates/legion-ui/src/ui.rs`; `crates/legion-ui/src/vim_intent.rs`; `crates/legion-text/src/lib.rs`; `xtask/src/main.rs`; `xtask/src/docs_hygiene.rs`; representative agent/provider/xtask tests; `README.md`; `CONTRIBUTING.md`; `docs/INDEX.md`; `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`; `docs/OPERATOR_RUNBOOK.md`; `plans/dependency-policy.md`; `plans/phase-status-ledger.md`; `plans/product-readiness-ledger.md`; and the full-gate scripts. The exact replacement above reflects verified facts and omits unsupported numerical coverage/toolchain claims. No repository checks were run in plan mode.

After approval, reread root `AGENTS.md` and replace only that file with the exact replacement document above. Preserve any new user-authored instruction if the root file changes before execution, without adding a ninth section. Then reread it and check the exact title, all eight requested headings and order, command spellings, evidence-backed paths, and absence of placeholders. Run `cargo run -p xtask -- docs-hygiene` from the repository root. If it fails, only correct violations introduced by this document; do not change allowlists or unrelated files. Since this is documentation-only, do not run builds, Rust suites, or GUI tests.

