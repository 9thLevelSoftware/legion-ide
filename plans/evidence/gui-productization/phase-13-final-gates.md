# Phase 13 Final Gates

## Status

- Phase 13 final gates: passed for the current local checkout on 2026-05-28.
- Evidence mode: metadata-only command labels, exit status, and concise result summaries.
- Final gate outputs archived from current commands.

## Required Commands

| Command | Local outcome |
|---|---|
| `cargo run -p xtask -- check-deps` | passed; dependency policy checks passed |
| `cargo fmt --all --check` | passed |
| `cargo check --workspace --all-targets` | passed |
| `cargo test --workspace --all-targets` | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed |

## Targeted Phase 13 Verification Already Recorded

| Slice | Command | Outcome |
|---|---|---|
| 13-01 | `rg -q "Legion Workflow orchestration" plans/adrs/ADR-0031-legion-workflow-orchestration.md` | passed |
| 13-01 | `rg -q "Phase 13" plans/dependency-policy.md` | passed |
| 13-01 | `rg -q "Autonomous merge: unsupported until approval" plans/evidence/gui-productization/phase-13-governance.md` | passed |
| 13-01 | `cargo run -p xtask -- check-deps` | passed |
| 13-02 | `rg -q "LegionWorkflowSession" crates/devil-protocol/src/lib.rs` | passed |
| 13-02 | `rg -q "validate_legion_workflow" crates/devil-protocol/src/lib.rs` | passed |
| 13-02 | `cargo test -p devil-protocol --test dto_contracts legion_workflow -- --nocapture` | passed, 5 tests |
| 13-02 | `cargo check -p devil-protocol` | passed |
| 13-03 | `rg -q "LegionWorkflowCoordinator" crates/devil-agent/src/lib.rs` | passed |
| 13-03 | `rg -q "devil-app" crates/devil-agent/src/lib.rs; if ($LASTEXITCODE -eq 0) { exit 1 } else { exit 0 }` | passed |
| 13-03 | `cargo test -p devil-agent legion_workflow -- --nocapture` | passed, 8 tests |
| 13-03 | `cargo check -p devil-agent` | passed |
| 13-04 | `rg -q "LegionWorkflow" crates/devil-tracker/src/lib.rs` | passed |
| 13-04 | `rg -q "LegionWorkflow" crates/devil-memory/src/lib.rs` | passed |
| 13-04 | `cargo test -p devil-tracker legion_workflow -- --nocapture` | passed, 4 tests |
| 13-04 | `cargo test -p devil-memory legion_workflow -- --nocapture` | passed, 4 tests |
| 13-04 | `cargo check -p devil-tracker -p devil-memory` | passed |
| 13-05 | `rg -q "LegionWorkflow" crates/devil-app/src/lib.rs` | passed |
| 13-05 | `rg -q "execute_legion_workflow" crates/devil-app/src/lib.rs` | passed |
| 13-05 | `cargo test -p devil-app --test legion_workflow_integration -- --nocapture` | passed, 9 tests |
| 13-05 | `cargo check -p devil-app --all-targets` | passed |
| 13-06 | `rg -q "LegionWorkflow" crates/devil-ui/src/ui.rs` | passed |
| 13-06 | `rg -q "legion workflow command center" crates/devil-desktop/src/view.rs` | passed |
| 13-06 | `rg -q "Autonomous merge" crates/devil-desktop/src/health.rs` | passed |
| 13-06 | `cargo test -p devil-ui legion_workflow -- --nocapture` | passed, 4 tests |
| 13-06 | `cargo test -p devil-desktop --test legion_workflow_command_center -- --nocapture` | passed, 4 tests |
| 13-06 | `cargo check -p devil-desktop --all-targets` | passed |

## Acceptance Marker Checks

| Marker | Artifact | Outcome |
|---|---|---|
| `Phase 13 acceptance: Accepted` | `plans/evidence/gui-productization/phase-13-legion-workflow-orchestration.md` | present |
| `Legion workflow orchestration: approval-gated` | `plans/evidence/gui-productization/phase-13-legion-workflow-orchestration.md` | present |
| `Autonomous merge: unsupported until approval` | evidence and runbook | present |
| `Provider-backed workers: routed through assisted-AI consent` | evidence and runbook | present |
| `Final gate outputs archived from current commands` | evidence and final gates | present |

## Cargo Deny

`cargo deny check` is outside the Phase 13 frontmatter verification command list, but AGENTS.md notes that CI runs it with warning-level policy.

| Command | Local outcome |
|---|---|
| `cargo deny check` | passed; advisories, bans, licenses, and sources were ok; known duplicate dependency warnings were emitted under warning-level policy |

## Evidence Handling

- Command evidence is summarized as command label, local outcome, date, and concise notes.
- Raw terminal logs are intentionally omitted from this artifact.
- Provider payloads, prompts, source bodies, and worker logs are outside the final gate archive.
- The Phase 13 evidence ledger links each workflow slice back to the matching result artifact.

## Residual Risks

- Final broad command results must remain aligned with the current checkout after any subsequent source edits.
- The known repository duplicate-dependency diagnostics remain under warning-level deny policy when `cargo deny check` is run.
- This archive stores command labels and concise outcomes only; full raw terminal logs are intentionally not retained.
