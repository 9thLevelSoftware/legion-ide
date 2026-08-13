# PKT-GP2: GP-2 Golden Path Smoke Harness — Evidence

**Packet:** PKT-GP2 (Wave 5, M9 milestone closer)
**Branch:** `m9/gp2-smoke`
**PR:** #58
**Base:** `660abe6` (main after PKT-INLINE merge)
**Date:** 2026-07-06

## Deliverables

| # | Deliverable | File(s) | Status |
|---|-------------|---------|--------|
| 1 | GP-2 binary | `crates/legion-app/src/bin/golden_path_2.rs` (1176 lines) | Done |
| 2 | xtask orchestrator | `xtask/src/golden_path_2.rs` (91 lines) | Done |
| 3 | xtask wiring | `xtask/src/main.rs`, `xtask/src/lib.rs` | Done |
| 4 | CI workflow | `.github/workflows/legion-smoke.yml` (`smoke-gp2` job) | Done |

## GP-2 Steps

| Step | Name | What it exercises | Result |
|------|------|-------------------|--------|
| s1 | copy-fixture | Copy `fixtures/gp1-rust`, git-init, `open_workspace(Trusted)` | Passed (178ms) |
| s2 | provider-setup | `set_product_mode(Assist)`, `open_file(src/main.rs)` | Passed (1ms) |
| s3 | inline-prediction | `RequestAssistInlinePrediction` → ghost text → `AcceptAssistInlinePrediction` → buffer changed → undo available | Passed (1ms) |
| s4 | provider-route | `ProviderRegistry` + `DenyByDefaultBroker` + `ProviderRouter`: local-loopback → Completed, unauthorized remote → Refused | Passed (0ms) |
| s5 | context-manifest | `collect_file_context` + `assemble_context_manifest_from_sources`: manifest_id valid, items non-empty, permissions non-empty | Passed (0ms) |
| s6 | checkpoint-apply | Undo s3 → CreateFile proposal → register → validate → preview → apply → `list_checkpoints()` non-empty → `restore_checkpoint()` → file removed → buffer at original | Passed (9ms) |
| s7 | evidence | Write `target/golden-path/gp2_report.toml` | Passed (0ms) |

## Key design decisions

1. **CreateFile proposal in s6 (not TextEdit):** `TextEdit` proposals produce `ProposalMutationRollback::TextEdit` which `collect_checkpoint_targets` maps to `Vec::new()` — no durable checkpoint is created. `CreateFile` produces a `WorkspaceFile` rollback with `CreatedFile` target, which is the canonical checkpoint-producing path.

2. **Default features (ai ON):** GP-2 cargo args do NOT include `--no-default-features`. This is the key distinction from GP-1 and the entire point of the harness — proving the AI-enabled product APIs work end-to-end.

3. **DeterministicInlinePredictionProvider only:** No network calls, no API keys. Produces predictable ghost text for CI. Live Ollama/Anthropic legs are future extensions.

## Verification

| Gate | Result |
|------|--------|
| `manual_zero_egress` | Passed |
| `cargo fmt --check` | Clean |
| `cargo clippy --all-targets` | 0 warnings |
| `cargo test --all-targets` | All pass |
| GP-1 (7 steps) | Passed |
| GP-2 (7 steps) | Passed |
| All 17 standing gates | Green |
| Disk space | 164 GB free |

## Review rounds

| Round | Reviewer | Verdict | Findings |
|-------|----------|---------|----------|
| 1 | Task reviewer (sonnet) | Not approved | 1 Important (s6 missing checkpoint pipeline), 4 Minor |
| Fix | Fix subagent (sonnet) | s6 rewritten with full proposal lifecycle + checkpoint store | — |
| Re-review | Task reviewer (sonnet) | Approved | Fix confirmed, 1 new Minor (dead params) |
| Final | Whole-branch (opus) | Ready to merge | 1 new Minor (stale doc, fixed), 4 prior Minor deferred |

## Minor findings (deferred)

1. Step IDs s3/s4/s5 reordered vs brief (cosmetic — evidence TOML uses implementation order)
2. s4 remote-refusal test uses `AssistedAiProviderClass::Local` with remote `network_target` (test valid via broker logic, label misleading)
3. `parse_args()` silently drops unknown flags (internal binary, controlled invocation)
4. `run_s6` accepts `workspace_id`/`generation` params but discards with `let _ =` (generation refreshed inside s6)

## M9 exit condition

- [x] GP-2 passes with deterministic-local provider (all 7 steps green)
- [x] `manual_zero_egress` remains green
- [ ] GP-2 CI 3-OS history (pending PR #58 CI)
- [ ] Kanban P3/P4 tasks marked done (pending merge)
