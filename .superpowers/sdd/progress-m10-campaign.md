# M10 Delegate Mode Agent Harness — Campaign Progress Ledger

Plan: `C:/Users/dasbl/.claude/plans/optimized-gliding-gizmo.md` (approved 2026-07-06)
Mode: multi-agent packets, branch+PR per packet, merges serialized (user-confirmed).
Machine constraints: builds at `-j 4`; disk check (>60GB) before every gate chain.
Prior ledger: `.superpowers/sdd/progress-m9-campaign.md` (complete).

## Packets

- [x] PKT-0: orphan sweep + honesty fixes (branch m10/residuals)
- [x] PKT-MODELIO: tool-calling model I/O (branch m10/tool-model-io)
- [x] PKT-SANDBOX: OS sandbox enforcement (branch m10/os-sandbox)
- [x] PKT-LOOP: native execution loop (branch m10/agent-loop)
- [x] PKT-WORKTREE: worktree scope + honest UI (branch m10/worktree-scope)
- [x] PKT-START: scope picker + production dispatch (branch m10/delegate-start)
- [x] PKT-WORKER: worker panel + kill switch (branch m10/worker-panel)
- [x] PKT-EVAL: adversarial evals (branch m10/adversarial-evals)
- [x] PKT-GP3: GP-3 harness + exit gate (branch m10/gp3-smoke)

## Completion log

(entries appended as packets complete)

### PKT-0 COMPLETE (2026-07-06)
- Commits: d221aed..332d59b (7 commits on m10/residuals)
- Review: Approved (sonnet) — 0 Critical, 1 Important (P4.F4 task count in evidence, fixed), 1 Minor (filter allocation, fixed)
- Deliverables: orphan declarations (agent + debug), desktop orphan deletion (4 files), sandbox panel honesty, trust gate on CreateGitWorktree, kanban P3/P4→done + P5→M10, stale evidence marker
- Tests: all pass, manual_zero_egress green
- Minor deferred: stale notice wording deviates slightly from spec-prescribed text (semantic intent satisfied)

### PKT-MODELIO COMPLETE (2026-07-06)
- Commits: 6b3fa5f..8cc1417 (4 commits on m10/tool-model-io)
- Review: Approved (sonnet) — 0 Critical, 1 Important (trailing expect_prior_result_contains silently dropped in build(), fixed with assert), 3 Minor (FixedTransport vs RecordingTransport, commit count, type name differences from brief)
- Deliverables: ToolCallingProvider trait + DTOs, ScriptedToolCallingProvider with builder DSL + determinism guards, Anthropic tool-calling wire format (extract_assistant_blocks + turn serialization)
- Tests: 26 legion-ai lib tests (4 new tool_calls), 29+1ignored legion-ai-providers (4 new), manual_zero_egress green
- Minor deferred: D3 used FixedAnthropicTransport instead of RecordingProviderTransport (functionally correct)

### PKT-SANDBOX COMPLETE (2026-07-06)
- Commits: da2bcc4..f8f3ea6 (4 commits on m10/os-sandbox)
- Review: Approved (sonnet task + opus final) — Task review: 1 Critical (Linux dishonest network_enforced, fixed), 3 Important (handle leak, SECURITY.md wrong, dead Timeout variant, all fixed), 2 Minor (fixed). Final review: 2 Important (pipe deadlock reorder, SBPL injection escaping, both fixed), 4 Minor (ResumeThread check, RestrictionStatus comment, evidence accuracy, SECURITY.md wording, all fixed).
- Deliverables: spawn_sandboxed API (Windows job-object, Linux Landlock, macOS Seatbelt SBPL), SandboxSpawnSpec/SandboxedCommandOutput/SandboxEnforcementReport DTOs, escape probe binary, real-process escape tests (5 Windows + 3 cross-platform SBPL), SECURITY.md enforcement matrix, evidence file
- Tests: 18 legion-sandbox tests (5 lib, 3 compile_profiles, 10 escape_attempts), all pass, manual_zero_egress green, cargo deny green
- New deps: landlock 0.4 (Linux), windows features (Win32_Security, JobObjects, Threading, Pipes, FileSystem), tempfile (dev)
- Simplification: Windows uses job-object-only (no restricted token) — filesystem_write_enforced=false, network_enforced=false, honestly reported with caveat labels

### PKT-LOOP COMPLETE (2026-07-06)
- Commits: c54cad5..9773339 (8 commits on m10/agent-loop, squash merged as 9d948cb)
- Review: Approved (opus task + opus final) — Task review: 0 Critical, 3 Important (audit pairing on total_output_bytes path, scope assertion too permissive, event_sequence non-monotonic on max_model_turns — all fixed). Final review: 0 Critical, 3 Important (wall_clock_limit_ms silent no-op, max_consecutive_retries missing audit step, step_index duplicates — all fixed), 4 Minor (double containment validation accepted, max_tokens hardcoded, Read placeholder for UnknownTool, D3 DTOs unused in loop).
- Deliverables: state.rs + worktree.rs extraction from lib.rs (2852→~170 lines), DelegatedTaskLoopBudget + step record DTOs in delegate_loop.rs, LegionToolCallInvocation/Result/Outcome DTOs in tools.rs, run_delegated_task_loop with ports + 5-step validation pipeline + 7 tool executors + budget enforcement + audit pairing, 10 integration tests, evidence file
- Tests: all pass (10 agent_loop_integration, full workspace green), manual_zero_egress green, cargo deny green
- New deps: regex + globset (legion-agent, already in workspace), serde_json (legion-agent), tempfile (dev)
- Minor deferred: D3 DTOs (LegionToolCallInvocation/Result/Outcome) are protocol exports not yet consumed by the loop internally; double containment validation kept as defense-in-depth; max_tokens 4096 hardcoded (config field deferred); LegionToolKind::Read used as placeholder for UnknownTool feedback

### PKT-WORKTREE COMPLETE (2026-07-07)
- Commits: bf4d41a..390daa6 (5 commits on m10/worktree-scope, squash merged as 8f7b5b1)
- Review: Approved (sonnet task + opus final) — 0 Critical, 0 Important, 4 Minor (isolation_mode_label placeholder, activation-inferred lease, undocumented target/ exclusion, CWD-relative test helper — all accepted)
- Deliverables: SandboxIsolationMode enum + isolation_mode()/lease_acquired() accessors, with_workspace_root adopted in production, validate_not_main_workspace guard, SandboxPanelState + honest strength labels (os-enforced/process-isolated/fallback), evidence file
- Tests: all pass, manual_zero_egress green
- No new deps

### PKT-START COMPLETE (2026-07-07)
- Commits: 086d27e..b6e8c86 (7 commits on m10/delegate-start, squash merged as d71797e)
- Review: Approved (sonnet task + opus final) — Task review: 0 Critical, 5 Important (AgentRuntime bypassed, proposals comment misleading, missing tool host unit test, missing forbidden-path test, cleanup error silently discarded — all fixed), 2 Minor (ChatSent misused, evidence description wrong). Final review: 0 Critical, 4 Important (proposals evidence marks D4 Done, synchronous blocking undocumented, ChatSent for task loop, DelegatedTaskStarted naming — all fixed), 3 Minor (evidence row, cleanup comment, all fixed).
- Deliverables: D1 scope picker call site in Delegate dock, D2 StartDelegatedTask command routing (4 layers), D3 AppDelegatedToolHost (TerminalCommand→spawn_sandboxed, MCP→honest error), D4 start_delegated_task production dispatch with AgentRuntime + AllowAllCapabilityBroker + 5 integration tests + TaskLoopCompleted status, D5 evidence file
- Tests: 14 delegated_task_integration tests (5 new: completion, audit pairing, mode guard, forbidden-path, tool host echo), all pass, manual_zero_egress green
- New deps: legion-sandbox (ai feature dep of legion-app)
- Partial: D4 proposal extraction deferred — DelegatedTaskLoopResult doesn't surface proposals (tracked as PKT-PROPOSAL-SURFACE)
- Renames: AppDelegatedTaskStartOutcome → AppDelegatedTaskOutcome, DelegatedTaskStarted → DelegatedTaskCompleted, ChatSent → TaskLoopCompleted

### PKT-WORKER COMPLETE (2026-07-07)
- Commits: abf0212..71d925b (7 commits on m10/worker-panel, squash merged as 1e26322)
- Review: Approved (sonnet task + opus final) — Task review: 0 Critical, 2 Important (evidence file errors — method name, test name, cancel semantics — all fixed inline), 2 Minor (evidence test name mismatch fixed, Kill button unconditional accepted). Final review: 0 Critical, 2 Important (loop error path leaves activation at Executing, Kill button shows red error when idle — both fixed), 2 Minor (evidence concerns section, is_cancelled duplication).
- Deliverables: worker_panel module declared + wired into Delegation Console, SharedCancellationFlag (Arc<AtomicBool> Release/Acquire) implementing DelegatedTaskCancellationProbe, NeverCancelled stub removed, Executing + Cancelled activation states wired, Kill button gated on Executing, CancelDelegatedTask command pipeline (4 layers), cancel_delegated_task method, evidence file
- Tests: 15 delegated_task_integration tests (1 new: pre_cancelled_flag), 3 SharedCancellationFlag unit tests, all pass, manual_zero_egress green
- No new deps
- Known limitation: kill switch requires background-thread dispatch for live cancellation (start_delegated_task is synchronous with &mut self)

### PKT-EVAL COMPLETE (2026-07-07)
- Commits: 8d7d90d..6507ba6 (7 commits on m10/adversarial-evals, squash merged as a972ae0)
- Review: Approved (sonnet task + opus final) — Task review: 0 Critical, 2 Important (verify-hostile-evals structural no-op, redundant import — both fixed), 2 Minor (test name/assertion gap, terminal-command naming — already addressed by implementer). Final review: 0 Critical, 2 Important (test 4 claims exceed assertions, dead objective_for arm — both fixed), 3 Minor (TOML/test name inconsistency, ledger redundancy, temp workspace labels).
- Deliverables: 4 hostile eval TOML fixtures expanded (exfiltration, prompt-injection, hostile-file, tool-output), 4 integration tests in hostile_eval_integration.rs, HostileEval task kind + plan_hostile_eval_suite + hostile-evals/verify-hostile-evals xtask subcommands, PR-AI-002 readiness ledger updated, evidence file
- Tests: 4 hostile_eval_integration tests, hostile-evals + verify-hostile-evals (4/4 pass), all workspace tests pass, manual_zero_egress green
- No new deps
- Remaining: live-model adversarial evals deferred (requires provider API keys in CI); Windows network enforcement caveat (tested in PKT-SANDBOX)

### PKT-GP3 COMPLETE (2026-07-07)
- Commits: 88370d7..a2a9183 (5 commits on m10/gp3-smoke, squash merged as 1ae144f)
- Review: Approved (sonnet task + opus final) — Task review: 0 Critical, 0 Important, 3 Minor (redundant is_file guard, unused s8 params, byte-offset slice). Final review: 0 Critical, 0 Important, 4 Minor (same 3 + s5 shell redirect portability — all fixed in a2a9183).
- Deliverables: golden_path_3.rs binary (9 steps: fixture/workspace/Delegate, scope-select, worker-loop, scope-denial, sandbox-teeth, kill-switch, orphan-reap, review-apply, evidence TOML), xtask golden-path-3 subcommand (--features test-helpers), regenerated GP-3 delegate walkthrough, PKT-GP3 evidence file
- Tests: 9/9 steps pass, all workspace tests pass, manual_zero_egress green, fmt + clippy clean
- No new deps
- Remaining: 3-OS CI job pending (legion-smoke.yml smoke-gp3); live-model run deferred; PKT-PROPOSAL-SURFACE extraction deferred

## M10 Campaign COMPLETE — all 9/9 packets merged
