# Completion traceability register

Status: implementation planning in progress; owner-approved completion design is the governing scope. This register maps every current kanban task to one of seven program stages. Stage assignment is semantic routing; the master plan will refine concrete work packages. Existing kanban status and evidence are historical implementation signals; they do not establish product acceptance.

## Register rules

- Source of truth for this inventory: [plans/kanban/legion-ga-backlog.toml](../../../plans/kanban/legion-ga-backlog.toml), read at planning time.
- Coverage: 163 tasks across 42 feature families and 10 epics. The source currently reports 160 done, 2 in-progress, 1 blocked, and 0 todo.
- Every row starts with acceptance = **unassessed**. A kanban done value or fixture/component evidence is retained as historical evidence only. Product outcomes require a layer-3 native scenario; internal requirements follow the protected-outcome and safety-evidence rule below.
- Stage routing follows semantics: P0->S0; P1 and P2 terminal/search/Git plus P6 canvas->S1; P2 language/debug/test->S2; P3-P5->S3 (Assist and Delegate); P6 orchestration excluding canvas->S4; P7 plus P9 collaboration/remote/training/enterprise->S5. Production implementation is cross-cutting XQ-01..08: P8.F1 signing is XQ-07 across S0-S5, P8.F2 updates is XQ-04 across S1-S5, P8.F3 crash handling is S1/XQ-05, P8.F4 performance is S1/XQ-03, and P8.F5 accessibility is S1/XQ-02. P9.F1 evaluation is S3-S5 with final S6 qualification. S6 is final qualification only and does not own production implementation.

| Stage | Scope routing |
|---|---|
| S0 baseline | Truth, inventory, requirement reconciliation, and acceptance-register governance. |
| S1 manual | Manual editor, terminal, search, Git, canvas arrangement, and daily-driver foundations. |
| S2 languages | Rust, TypeScript/JavaScript, and Python language, debug, and test workflows. |
| S3 AI | Assist, Delegate, provider, context, proposal review, and privacy-controlled AI workflows. |
| S4 orchestration | Multi-agent Legion Workflow coordination, worker lanes, sandboxed orchestration, and orchestration UI; base Delegate belongs to S3. |
| S5 extensions/team | Extensions, collaboration, remote development, training controls, and enterprise administration. |
| S6 qualification | Final qualification of all accepted implementation and cross-cutting XQ evidence; it owns no production implementation. |

## Feature-family coverage

Rows below are historical task mappings, not a one-to-one count of product features. S0-01 separates product requirements from internal implementation/governance requirements. Internal requirements need their applicable safety evidence and links to the user outcomes they protect; they do not independently increase the accepted-feature count.

All 42 source families are represented below: P0.F1, P0.F2, P0.F3, P0.F4, P0.F5, P1.F1, P1.F2, P1.F3, P1.F4, P2.F1, P2.F2, P2.F3, P2.F4, P2.F5, P3.F1, P3.F2, P3.F3, P3.F4, P4.F1, P4.F2, P4.F3, P4.F4, P5.F1, P5.F4, P5.F2, P5.F3, P6.F1, P6.F2, P6.F3, P6.F4, P6.F5, P7.F1, P7.F2, P8.F1, P8.F2, P8.F3, P8.F4, P8.F5, P9.F1, P9.F2, P9.F3, P9.F4.

## Additional requirement sources and gaps

The mapping must be reconciled against the approved design and these requirement sources before implementation stages close: [legion-production-master-plan-v0.2.md](../../../plans/legion-production-master-plan-v0.2.md), [legion-production-roadmap-v1.0.md](../../../plans/legion-production-roadmap-v1.0.md), [product-readiness-ledger.md](../../../plans/product-readiness-ledger.md), [phase-status-ledger.md](../../../plans/phase-status-ledger.md), [p0-installed-product-sequence-v0.1.md](../../../plans/p0-installed-product-sequence-v0.1.md), [control-first-adaptive-ide-granular-implementation-plan-v0.1.md](../../../plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md), [control-first-adaptive-ide-technical-design-v0.1.md](../../../plans/control-first-adaptive-ide-technical-design-v0.1.md), [foundational-core-ide-platform-roadmap-v0.1.md](../../../plans/foundational-core-ide-platform-roadmap-v0.1.md), [remaining-implementation-tasks-plan-v0.1.md](../../../plans/remaining-implementation-tasks-plan-v0.1.md), [canvas-workspace-direction.md](../../../docs/ui/canvas-workspace-direction.md), applicable ADRs, and retained evidence under `plans/evidence/`.

The complete P0 installed-product sequence and its ten-gap parking lot are also requirement sources. GAP-01 (installed-product truth), GAP-02 (release signing), GAP-03 (update safety), GAP-04 (data safety), GAP-05 (accessibility), GAP-06 (security hardening), GAP-07 (governance), GAP-08 (documentation truth), GAP-09 (performance), and GAP-10 (support/legal) are all in scope. The sequence is [p0-installed-product-sequence-v0.1.md](../../../plans/p0-installed-product-sequence-v0.1.md); committed evidence currently includes GAP-01, GAP-02, GAP-04, GAP-05, GAP-07, GAP-09, and GAP-10 under `plans/evidence/production/WS-P0/`. GAP-03 and GAP-06 remain requirements from the sequence and associated updater/security ADRs even where no dedicated `gap-03`/`gap-06` evidence file exists. Parking-lot and deferred-surface requirements are additionally retained in [legion-production-master-plan-v0.2.md](../../../plans/legion-production-master-plan-v0.2.md), [future-surface-deferral-audit.md](../../../plans/evidence/phase-6/future-surface-deferral-audit.md), [ADR-0046-surface-expansion-freeze.md](../../../plans/adrs/ADR-0046-surface-expansion-freeze.md), and [procurement-and-key-escrow.md](../../../plans/release/procurement-and-key-escrow.md).

Unavailable sources are recorded rather than inferred: the generator source `.hermes/plans/2026-06-13_173122-legion-current-to-ga-kanban-plan.md` is absent (the backlog says it was removed); `ENGINEERING_STATUS.md`, `ENGINEERING_AUDIT.yaml`, and `ENGINEERING_PLAN.yaml` are absent after the documented cleanup. The historical [legion-production-master-plan-v0.1.md](../../../plans/legion-production-master-plan-v0.1.md) and [legion-e2e source package](../../../plans/legion-e2e/source-package/04_PRODUCT_IMPLEMENTATION_ROADMAP.md) remain available only as supporting leads. Their historical claims are not treated as reviewed current requirements.

## Task register

| Task ID | Feature family | Title | Kanban status | Program stage / cross-cut track | Evidence source | Product acceptance |
|---|---|---|---|---|---|---|
| `P0.F1.T1` | `P0.F1` | Decide canonical v1 modes: Manual, Assist, Delegate, Legion Workflows; treat Automate as internal/legacy | done | S0 baseline | `docs/MODES.md` | unassessed |
| `P0.F1.T2` | `P0.F1` | Update docs/keys/UI/design to the canonical labels | done | S0 baseline | `docs/KEYBOARD_REFERENCE.md` | unassessed |
| `P0.F1.T3` | `P0.F1` | Add a mode taxonomy regression test in legion-protocol/legion-ui | done | S0 baseline | `crates/legion-protocol/tests/mode_taxonomy.rs` | unassessed |
| `P0.F1.T4` | `P0.F1` | Add a docs-hygiene rule/static test rejecting legacy labels | done | S0 baseline | `xtask/src/docs_hygiene.rs` | unassessed |
| `P0.F2.T1` | `P0.F2` | Remove/replace plans/legion-customizable-autonomy-continuation-plan-v0.1.md links from README and docs index | done | S0 baseline | `docs/INDEX.md` | unassessed |
| `P0.F2.T2` | `P0.F2` | Mark older E2E and engineering audit files as supporting/historical in docs index | done | S0 baseline | `docs/INDEX.md` | unassessed |
| `P0.F2.T3` | `P0.F2` | Add current-state caveat to docs/USER_GUIDE.md (desktop proof vs full readiness) | done | S0 baseline | `docs/USER_GUIDE.md` | unassessed |
| `P0.F2.T4` | `P0.F2` | Keep Devil-era historical references allowlisted only in archived evidence | done | S0 baseline | `docs/hygiene-allowlist.toml` | unassessed |
| `P0.F3.T1` | `P0.F3` | Convert the GA plan into a machine-readable backlog file (this file) | done | S0 baseline | `plans/kanban/legion-ga-backlog.toml` | unassessed |
| `P0.F3.T2` | `P0.F3` | Add fields: epic, feature, task, dependency, mode, readiness row, files, verification, acceptance | done | S0 baseline | `plans/kanban/legion-ga-backlog.toml` | unassessed |
| `P0.F3.T3` | `P0.F3` | Add an xtask subcommand that validates required fields and orphan dependencies | done | S0 baseline | `xtask/src/kanban_backlog.rs` | unassessed |
| `P0.F4.T1` | `P0.F4` | Repair references to files deleted in the 2026-08-12 cleanup (ledger, INDEX, README, CONTRIBUTING, CODEBASE, hygiene allowlist) | done | S0 baseline | `plans/evidence/production/WS-P0/phase-0.2-truth-closure.md` | unassessed |
| `P0.F4.T2` | `P0.F4` | Reconcile backlog statuses and meta against commit 5c09a24 (course correction) | done | S0 baseline | `plans/evidence/production/WS-P0/phase-0.2-truth-closure.md` | unassessed |
| `P0.F4.T3` | `P0.F4` | Fix cargo check -p legion-app --no-default-features and gate the configuration in CI | done | S0 baseline | `plans/evidence/production/WS-P0/phase-0.2-truth-closure.md` | unassessed |
| `P0.F4.T4` | `P0.F4` | Close the native-release RCA leftovers (verifier script extraction, Debian Maintainer, VALIDATION-SUMMARY publish gate) | done | S0 baseline | `docs/superpowers/plans/2026-08-12-native-release-e2e-rca-and-resolution.md` | unassessed |
| `P0.F4.T6` | `P0.F4` | Restore GP-1 s3 on rust-analyzer >= 1.96 via LSP 3.17 pull diagnostics | done | S0 baseline | `plans/evidence/production/WS-P0/2026-08-15-hosted-smoke-first-run.md` | unassessed |
| `P0.F4.T5` | `P0.F4` | Activate hosted 3-OS legion-smoke.yml runs and start the promotion clock | done | S0 baseline | `plans/evidence/production/WS-P0/2026-08-18-smoke-promotion-clock.md` | unassessed |
| `P0.F5.T1` | `P0.F5` | Survey the SmallCode repository and adopt ADR-0049 (port map, authority rule, governor classification, bench holdout policy) | done | S0 baseline | `plans/adrs/ADR-0049-smallcode-behavioral-cannibalization.md` | unassessed |
| `P0.F5.T2` | `P0.F5` | Publish MIT attribution artifacts (THIRD_PARTY_NOTICES.md, docs/legal/smallcode-attribution.md) | done | S0 baseline | `THIRD_PARTY_NOTICES.md` | unassessed |
| `P0.F5.T3` | `P0.F5` | Extract SmallCode test-vector corpora (malformed tool calls, patch/edit blocks) as attributed fixture data | done | S0 baseline | `docs/legal/smallcode-attribution.md` | unassessed |
| `P1.F1.T1` | `P1.F1` | Write failing test that sends synthetic key/mouse input into DesktopEframeApp::update | done | S1 manual | `crates/legion-desktop/tests/headless_input.rs` | unassessed |
| `P1.F1.T2` | `P1.F1` | Add a headless egui/eframe test harness seam without giving renderer product authority | done | S1 manual | `crates/legion-desktop/tests/headless_input.rs` | unassessed |
| `P1.F1.T3` | `P1.F1` | Port representative projection-only tests to the harness (open/edit/save/search/palette/mode switch) | done | S1 manual | `plans/evidence/gui-productization/gui-headless-input-evidence.md` | unassessed |
| `P1.F1.T4` | `P1.F1` | Emit gui-headless-input-evidence.md with exact command output and scenario list | done | S1 manual | `plans/evidence/gui-productization/gui-headless-input-evidence.md` | unassessed |
| `P1.F2.T1` | `P1.F2` | Make Manual mode hide all AI/provider/cloud/worker surfaces by construction | done | S1 manual | `plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md` | unassessed |
| `P1.F2.T2` | `P1.F2` | Complete top bar, status bar, dock layout, file tree, editor tabs, terminal/tests panel, problems/symbols panels | done | S1 manual | `plans/evidence/gui-productization/phase-3-daily-editing-mvp.md` | unassessed |
| `P1.F2.T3` | `P1.F2` | Add keyboard-only navigation for all Manual surfaces | done | S1 manual | `crates/legion-desktop/tests/keyboard_nav.rs` | unassessed |
| `P1.F2.T4` | `P1.F2` | Persist/restore layout metadata only (no code, no AI context) | done | S1 manual | `plans/evidence/production/WS-MANUAL-01/session-persistence-default-evidence.md` | unassessed |
| `P1.F3.T1` | `P1.F3` | Preserve/verify no egui::TextEdit in the code canvas | done | S1 manual | `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md` | unassessed |
| `P1.F3.T2` | `P1.F3` | Implement/harden row virtualization, gutter lanes, selection, cursor, multi-cursor projection, scroll behavior | done | S1 manual | `plans/evidence/production/WS-MANUAL-01/multi-cursor-reachable-evidence.md` | unassessed |
| `P1.F3.T3` | `P1.F3` | Wire syntax/token styling from current legion-index/LSP data into the painter | done | S1 manual | `plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md` | unassessed |
| `P1.F3.T4` | `P1.F3` | Add input conformance tests for keyboard, mouse, selection, clipboard, IME event plumbing, and focus | done | S1 manual | `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md` | unassessed |
| `P1.F3.T5` | `P1.F3` | Wire Vim modal editing: motions, operators, register, insert entry, and the desktop key feed | done | S1 manual | `crates/legion-app/tests/vim_modal_editing.rs` | unassessed |
| `P1.F4.T1` | `P1.F4` | Add failing 100MB open/scroll test marked ignored only until implementation starts | done | S1 manual | `plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md` | unassessed |
| `P1.F4.T2` | `P1.F4` | Add streaming/chunked snapshot path that does not materialize full text for large files | done | S1 manual | `plans/evidence/production/WS-MANUAL-02/large-file-typing-fix.md` | unassessed |
| `P1.F4.T3` | `P1.F4` | Update ActiveBufferProjection to distinguish full, degraded, and streaming states | done | S1 manual | `crates/legion-editor/tests/streaming_projection.rs` | unassessed |
| `P1.F4.T4` | `P1.F4` | Wire UI badge and disabled-feature explanations for streaming mode | done | S1 manual | `crates/legion-desktop/tests/large_file_guardrails.rs` | unassessed |
| `P1.F4.T5` | `P1.F4` | Un-ignore/enforce the 100MB workload in the perf harness | done | S1 manual | `plans/evidence/production/WS-MANUAL-02/large-file-100mb-measurement.md` | unassessed |
| `P2.F1.T1` | `P2.F1` | Write a failing integration test launching a mock/rust-analyzer-compatible server through the product registry | done | S2 languages | `plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md` | unassessed |
| `P2.F1.T2` | `P2.F1` | Add rust-analyzer binary resolution, workspace config, lifecycle supervision, restart/backoff, and status projection | done | S2 languages | `plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md` | unassessed |
| `P2.F1.T3` | `P2.F1` | Wire diagnostics to gutter/problems panel through desktop harness | done | S2 languages | `plans/evidence/production/M8/WS-LANG-01-product-ui-evidence.md` | unassessed |
| `P2.F1.T4` | `P2.F1` | Wire completion, hover, definition, references, symbols, inlay hints, code lenses, and runnables | done | S2 languages | `plans/evidence/production/M8/WS-LANG-01-product-ui-evidence.md` | unassessed |
| `P2.F1.T5` | `P2.F1` | Route write-side actions (rename, format, code actions, organize imports) through proposals | done | S2 languages | `plans/evidence/production/M8/WS-LANG-01-write-side-evidence.md` | unassessed |
| `P2.F1.T6` | `P2.F1` | Wire the call-hierarchy app stub so incoming/outgoing calls reach a panel | done | S2 languages | `plans/evidence/production/WS-LANG-01/2026-08-19-call-hierarchy-and-the-capability-gate.md` | unassessed |
| `P2.F2.T1` | `P2.F2` | Write failing test expecting trusted workspace terminal launch to produce real output | done | S1 manual | `plans/evidence/production/M8/WS-TERM-01-evidence.md` | unassessed |
| `P2.F2.T2` | `P2.F2` | Promote legion-terminal runtime behind explicit capability policy, not test-only fixture toggles | done | S1 manual | `plans/evidence/production/M8/WS-TERM-01-evidence.md` | unassessed |
| `P2.F2.T3` | `P2.F2` | Add terminal renderer grid, scrollback, input, resize, selection/copy, and kill escalation | done | S1 manual | `plans/evidence/production/M8/WS-TERM-01-evidence.md` | unassessed |
| `P2.F2.T4` | `P2.F2` | Add OSC 133/OSC 7 command boundary and cwd/exit-code tracking | done | S1 manual | `plans/evidence/production/M8/WS-TERM-01-evidence.md` | unassessed |
| `P2.F2.T5` | `P2.F2` | Windows ConPTY parity task | done | S1 manual | `plans/evidence/production/M8/WS-TERM-01-evidence.md` | unassessed |
| `P2.F3.T1` | `P2.F3` | Add DAP client runtime or promote existing state machine beyond fixture use | done | S2 languages | `plans/evidence/production/WS-A-D/phase-2-dap/B5-persistent-live-session.md` | unassessed |
| `P2.F3.T2` | `P2.F3` | Wire CodeLLDB for Rust with policy-gated adapter resolution | done | S2 languages | `plans/evidence/production/WS-A-D/phase-2-dap/B20-three-platform-dogfood.md` | unassessed |
| `P2.F3.T3` | `P2.F3` | Persist breakpoints and hit them in a fixture Rust binary | done | S2 languages | `plans/evidence/production/WS-A-D/phase-2-dap/B15-f9-toggle-breakpoint.md` | unassessed |
| `P2.F3.T4` | `P2.F3` | Build test explorer from LSP runnables / cargo test list | done | S2 languages | `plans/evidence/production/WS-LANG-01/P2-F3-T4-test-explorer-discovery.md` | unassessed |
| `P2.F3.T5` | `P2.F3` | Expose debug/test results as metadata-only evidence for agents | done | S2 languages | `plans/evidence/production/WS-LANG-01/P2-F3-T5-test-evidence-and-run-group.md` | unassessed |
| `P2.F4.T1` | `P2.F4` | Wire workspace search UI to real search APIs with streaming results and cancellation | done | S1 manual | `plans/evidence/production/M8/WS-SEARCH-01-evidence.md` | unassessed |
| `P2.F4.T2` | `P2.F4` | Add multi-file search/replace as a workspace-edit proposal | done | S1 manual | `plans/evidence/production/M8/WS-SEARCH-01-evidence.md` | unassessed |
| `P2.F4.T3` | `P2.F4` | Add fuzzy file finder, symbol finder, recent buffer switcher, and command palette coverage report | done | S1 manual | `plans/evidence/production/M8/WS-SEARCH-01-evidence.md` | unassessed |
| `P2.F4.T4` | `P2.F4` | Add 50K/100K-file fixture benchmarks | done | S1 manual | `plans/evidence/production/M8/WS-SEARCH-01-evidence.md` | unassessed |
| `P2.F5.T1` | `P2.F5` | Wire live gutter diff/blame to desktop painter | done | S1 manual | `plans/evidence/production/M8/WS-GIT-01-evidence.md` | unassessed |
| `P2.F5.T2` | `P2.F5` | Add status panel, hunk/range staging, commit editor, push/fetch/pull actions | done | S1 manual | `plans/evidence/production/M8/WS-GIT-01-evidence.md` | unassessed |
| `P2.F5.T3` | `P2.F5` | Add branch/worktree manager and agent worktree visibility | done | S1 manual | `plans/evidence/production/M8/WS-GIT-01-evidence.md` | unassessed |
| `P2.F5.T4` | `P2.F5` | Keep network/auth operations policy-visible | done | S1 manual | `plans/evidence/production/M8/WS-GIT-01-evidence.md` | unassessed |
| `P3.F1.T1` | `P3.F1` | Inventory every mutation route (manual save, text edit, closed-file, workspace edit, LSP action, AI proposal, batch, plugin, remote, collaboration) | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F1.T2` | `P3.F1` | For each accepted route, write failing stale/conflict/audit tests before enabling product apply | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F1.T3` | `P3.F1` | Keep plugin/remote/collaboration/terminal-command apply routes denied until their own epics activate them | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F1.T4` | `P3.F1` | Remove or narrow any `runtime_apply_disabled`-style flag only after evidence exists | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F2.T1` | `P3.F2` | Build multibuffer/excerpt diff surface over existing snapshots and anchors | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F2.T2` | `P3.F2` | Add per-file and per-hunk accept/reject/edit-in-place | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F2.T3` | `P3.F2` | Add evidence panel: test results, command summaries, context manifest, risk rules, provenance | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F2.T4` | `P3.F2` | Add keyboard-first review workflow | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F3.T1` | `P3.F3` | Auto-create checkpoint before AI/agent proposal apply and batch operations | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F3.T2` | `P3.F3` | Add checkpoint timeline and restore command | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F3.T3` | `P3.F3` | Add test for apply -> manual edit -> checkpoint restore preserving non-conflicting manual edits | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F3.T4` | `P3.F3` | Record checkpoint/restore metadata in audit ledger | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F4.T1` | `P3.F4` | Define deterministic risk rules: path scope, file count, deletion ratio, dependency/lockfile touch, migrations, secrets proximity, binary/generated file changes | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F4.T2` | `P3.F4` | Add envelope config for auto-approval of explicitly low-risk changes only | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F4.T3` | `P3.F4` | Add visible risk strip and approval queue | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P3.F4.T4` | `P3.F4` | Optional model-assisted classifier may recommend risk but never override policy/human authority | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F1.T1` | `P4.F1` | Add provider setup UI for local-first default, BYOK hosted providers, and air-gap denial | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F1.T2` | `P4.F1` | Store keys through OS keyring/retention primitives; never raw config files | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F1.T3` | `P4.F1` | Add provider capability matrix for tools, structured output, streaming, vision, context length, thinking modes, and cost usage | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F1.T4` | `P4.F1` | Add live/recorded smoke tests for local and hosted paths where credentials are available | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F2.T1` | `P4.F2` | Assemble context from file, selection, symbols, diagnostics, terminal excerpts, memory, and rules | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F2.T2` | `P4.F2` | Show manifest before invocation with per-item exclusion | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F2.T3` | `P4.F2` | Add byte-for-byte egress equality test between manifest and sent request after redaction | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F2.T4` | `P4.F2` | Add post-run privacy inspector with retention/deletion handles | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F3.T1` | `P4.F3` | Wire ghost text to provider registry with cancellation/dismiss/accept feedback | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F3.T2` | `P4.F3` | Add assistant rail commands: explain, fix, test, doc, refactor | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F3.T3` | `P4.F3` | Add streaming markdown/codeblock UI with apply-as-proposal buttons | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F3.T4` | `P4.F3` | Add telemetry for suggestion latency/acceptance only under metadata/consent policy | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F4.T1` | `P4.F4` | Selection/cursor instruction produces streaming diff overlay anchored to current text | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F4.T2` | `P4.F4` | Accept/reject per hunk routes through proposal apply | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P4.F4.T3` | `P4.F4` | Undo integrates with editor history and checkpoint ledger | done | S3 AI | `plans/evidence/production/M9/` | unassessed |
| `P5.F1.T1` | `P5.F1` | Add schema-validated tools: read, grep, glob, outline, edit-as-proposal, terminal command, MCP passthrough | done | S3 AI | `plans/evidence/production/M10/PKT-LOOP-evidence.md` | unassessed |
| `P5.F1.T2` | `P5.F1` | Route every tool call through capability broker and observability causality chain | done | S3 AI | `plans/evidence/production/M10/PKT-LOOP-evidence.md` | unassessed |
| `P5.F1.T3` | `P5.F1` | Add retry/error feedback protocol for invalid tool calls | done | S3 AI | `plans/evidence/production/M10/PKT-LOOP-evidence.md` | unassessed |
| `P5.F1.T4` | `P5.F1` | Add bounded output redaction and truncation policy | done | S3 AI | `plans/evidence/production/M10/PKT-LOOP-evidence.md` | unassessed |
| `P5.F4.T1` | `P5.F4` | Recover tool calls embedded in model prose (tagged, Liquid, fenced, bare) | done | S3 AI | `plans/evidence/production/WS-AICP-01/tool-normalizer-evidence.md` | unassessed |
| `P5.F4.T2` | `P5.F4` | Surface unparseable tool arguments as a non-dispatchable typed block | done | S3 AI | `plans/evidence/production/WS-AICP-01/tool-normalizer-evidence.md` | unassessed |
| `P5.F4.T3` | `P5.F4` | Resolve model-authored edit fragments by exact unique match (patch-first editing) | done | S3 AI | `plans/evidence/production/WS-AICP-01/patch-first-evidence.md` | unassessed |
| `P5.F2.T1` | `P5.F2` | Implement OS sandbox layer: Seatbelt on macOS, bubblewrap/Landlock on Linux, restricted token/AppContainer or documented fallback on Windows | done | S3 AI | `plans/evidence/production/M10/PKT-SANDBOX-evidence.md` | unassessed |
| `P5.F2.T2` | `P5.F2` | Ensure worker write scope is a disposable worktree/copy, not main workspace | done | S3 AI | `plans/evidence/production/M10/PKT-WORKTREE-evidence.md` | unassessed |
| `P5.F2.T3` | `P5.F2` | Add network egress policy and escape-attempt tests | done | S3 AI | `plans/evidence/production/M10/PKT-SANDBOX-evidence.md` | unassessed |
| `P5.F2.T4` | `P5.F2` | Surface sandbox strength/weakness in UI | done | S3 AI | `plans/evidence/production/M10/PKT-WORKTREE-evidence.md` | unassessed |
| `P5.F3.T1` | `P5.F3` | Build scope picker: files/module/repo, risk tolerance, allowed tools, forbidden paths | done | S3 AI | `plans/evidence/production/M10/PKT-START-evidence.md` | unassessed |
| `P5.F3.T2` | `P5.F3` | Show worker live status, plan, tool calls, test evidence, and proposal bundle | done | S3 AI | `plans/evidence/production/M10/PKT-WORKER-evidence.md` | unassessed |
| `P5.F3.T3` | `P5.F3` | Add cancellation/reap flow | done | S3 AI | `plans/evidence/production/M10/PKT-WORKER-evidence.md` | unassessed |
| `P5.F3.T4` | `P5.F3` | Add failure recovery states: blocked, needs approval, validation failed, conflict | done | S3 AI | `plans/evidence/production/M10/PKT-WORKER-evidence.md` | unassessed |
| `P6.F1.T1` | `P6.F1` | Directive creates editable requirements/design/tasks plan artifact | done | S4 orchestration | `plans/evidence/production/M11/PKT-PLAN-evidence.md` | unassessed |
| `P6.F1.T2` | `P6.F1` | Plan revisions are diffable and audited | done | S4 orchestration | `plans/evidence/production/M11/PKT-PLAN-evidence.md` | unassessed |
| `P6.F1.T3` | `P6.F1` | Approved plan becomes DAG input for workflow coordinator | done | S4 orchestration | `plans/evidence/production/M11/PKT-PLAN-evidence.md` | unassessed |
| `P6.F2.T1` | `P6.F2` | Wire LegionWorkflowCoordinator to real Delegate workers | done | S4 orchestration | `plans/evidence/production/M11/PKT-WORKERS-evidence.md` | unassessed |
| `P6.F2.T2` | `P6.F2` | Schedule dependencies and parallel lanes | done | S4 orchestration | `plans/evidence/production/M11/PKT-LANES-evidence.md` | unassessed |
| `P6.F2.T3` | `P6.F2` | Detect conflicts and pause for human resolution | done | S4 orchestration | `plans/evidence/production/M11/PKT-LANES-evidence.md` | unassessed |
| `P6.F2.T4` | `P6.F2` | Aggregate verification evidence and merge-readiness | done | S4 orchestration | `plans/evidence/production/M11/PKT-LANES-evidence.md` | unassessed |
| `P6.F3.T1` | `P6.F3` | Kanban columns: Assigned, In Progress, Waiting on Human, Testing, Done | done | S4 orchestration | `plans/evidence/production/M11/PKT-CONSOLE-evidence.md` | unassessed |
| `P6.F3.T2` | `P6.F3` | Cards: owner/model/status/progress/files/risk/test status/mini diff/last activity | done | S4 orchestration | `plans/evidence/production/M11/PKT-CONSOLE-evidence.md` | unassessed |
| `P6.F3.T3` | `P6.F3` | Agent comm stream tags: PLAN, WRITE, TEST, REVIEW, ERROR, APPROVAL, COMPLETE | done | S4 orchestration | `plans/evidence/production/M11/PKT-CONSOLE-evidence.md` | unassessed |
| `P6.F3.T4` | `P6.F3` | Risk monitor, approval queue, kill switch, budget meter | done | S4 orchestration | `plans/evidence/production/M11/PKT-GP4-evidence.md` | unassessed |
| `P6.F4.T1` | `P6.F4` | Ratify ACP host scope or local adapter bridge scope | done | S4 orchestration | `plans/adrs/ADR-0043-acp-host-local-adapter-bridge.md` | unassessed |
| `P6.F4.T2` | `P6.F4` | Run one external agent in a Legion-governed worktree/sandbox | done | S4 orchestration | `plans/evidence/production/M11/PKT-EXTERNAL-AGENT-evidence.md` | unassessed |
| `P6.F4.T3` | `P6.F4` | Convert external edits into Legion proposals and external logs into evidence artifacts | done | S4 orchestration | `plans/evidence/production/M11/PKT-EXTERNAL-AGENT-evidence.md` | unassessed |
| `P6.F5.T1` | `P6.F5` | Open files as draggable cards with connections the person draws | done | S1 manual | `docs/ui/canvas-workspace-direction.md` | unassessed |
| `P7.F1.T1` | `P7.F1` | Add ADR/dependency-policy update for wasmtime or selected runtime | done | S5 extensions/team | `plans/evidence/production/P7-F1/wasmtime-adr-and-wit-abi.md` | unassessed |
| `P7.F1.T2` | `P7.F1` | Define minimal WIT ABI for grammars, themes, and LSP adapters | done | S5 extensions/team | `plans/evidence/production/P7-F1/wasmtime-adr-and-wit-abi.md` | unassessed |
| `P7.F1.T3` | `P7.F1` | Enforce quotas, crash containment, capability denial, and audit | done | S5 extensions/team | `plans/evidence/production/P7-F1-T3-T4-plugin-quota-and-hostile-containment.md` | unassessed |
| `P7.F1.T4` | `P7.F1` | Add hostile-plugin tests: loop, OOM, capability probing, workspace access | done | S5 extensions/team | `plans/evidence/production/P7-F1-T3-T4-plugin-quota-and-hostile-containment.md` | unassessed |
| `P7.F2.T1` | `P7.F2` | Add install/update/remove UI for signed extension artifacts | done | S5 extensions/team | `plans/evidence/production/P7-F2-signed-extension-install-and-permission-review.md` | unassessed |
| `P7.F2.T2` | `P7.F2` | Add manifest permission review | done | S5 extensions/team | `plans/evidence/production/P7-F2-signed-extension-install-and-permission-review.md` | unassessed |
| `P7.F2.T3` | `P7.F2` | Add tampered artifact rejection test | done | S5 extensions/team | `plans/evidence/production/P7-F2-signed-extension-install-and-permission-review.md` | unassessed |
| `P7.F2.T4` | `P7.F2` | Keep VSIX metadata ingestion as compatibility reporting only; do not execute Node extensions | done | S5 extensions/team | `plans/product-readiness-ledger.md` | unassessed |
| `P8.F1.T1` | `P8.F1` | Define non-committed signer config: env/keyring/KMS/CI secret reference | done | S0-S5 cross-cutting / XQ-07 | `plans/evidence/production/M12/PKT-SIGN-evidence.md` | unassessed |
| `P8.F1.T2` | `P8.F1` | Add real signing path for macOS Developer ID/notarization, Windows Authenticode, Linux signatures | done | S0-S5 cross-cutting / XQ-07 | `plans/evidence/production/M12/PKT-SIGN-evidence.md` | unassessed |
| `P8.F1.T3` | `P8.F1` | Add fresh-VM Gatekeeper/SmartScreen/install smoke evidence | blocked | S0-S5 cross-cutting / XQ-07 | `plans/evidence/production/WS-A-D/phase-4-release/D4-readiness-close.md` | unassessed |
| `P8.F1.T4` | `P8.F1` | Preserve unsigned-beta policy if shipping before credentials exist | done | S0-S5 cross-cutting / XQ-07 | `plans/evidence/production/M12/PKT-SIGN-evidence.md` | unassessed |
| `P8.F1.T5` | `P8.F1` | Procure signing/update/eval external resources and record the key-escrow policy | in-progress | S0-S5 cross-cutting / XQ-07 | `plans/release/procurement-and-key-escrow.md` | unassessed |
| `P8.F2.T1` | `P8.F2` | Choose updater strategy and signed manifest format | done | S1-S5 cross-cutting / XQ-04 | `plans/evidence/production/M12/PKT-UPDATER-evidence.md` | unassessed |
| `P8.F2.T2` | `P8.F2` | Implement stable/preview channels | done | S1-S5 cross-cutting / XQ-04 | `plans/evidence/production/M12/PKT-UPDATER-evidence.md` | unassessed |
| `P8.F2.T3` | `P8.F2` | Test update -> rollback on all three OSes | done | S1-S5 cross-cutting / XQ-04 | `plans/evidence/production/M12/PKT-UPDATER-evidence.md` | unassessed |
| `P8.F3.T1` | `P8.F3` | Add opt-in first-run crash consent | done | S1 / XQ-05 | `plans/evidence/production/M12/PKT-CRASH-evidence.md` | unassessed |
| `P8.F3.T2` | `P8.F3` | Integrate minidump capture and symbol upload | done | S1 / XQ-05 | `plans/evidence/production/M12/PKT-CRASH-evidence.md` | unassessed |
| `P8.F3.T3` | `P8.F3` | Ensure diagnostics exports are metadata-only unless user explicitly includes raw data | done | S1 / XQ-05 | `plans/evidence/production/M12/PKT-CRASH-evidence.md` | unassessed |
| `P8.F4.T1` | `P8.F4` | Replace stand-ins with real input-to-paint, scroll jank, startup, memory ceiling, Legion repo, 100K-file fixture, and 100MB file workloads | done | S1 / XQ-03 | `plans/evidence/production/P8.F4/perf-harness-product-workloads.md` | unassessed |
| `P8.F4.T2` | `P8.F4` | Run on macOS, Windows, Linux CI matrix | done | S1 / XQ-03 | `plans/evidence/production/P8.F4/perf-harness-product-workloads.md` | unassessed |
| `P8.F4.T3` | `P8.F4` | Add dashboard/report trend and regression threshold | done | S1 / XQ-03 | `plans/evidence/production/P8.F4/perf-harness-product-workloads.md` | unassessed |
| `P8.F5.T1` | `P8.F5` | Add AccessKit product pass and screen-reader walkthroughs for GP-1..GP-3 | done | S1 / XQ-02 | `plans/evidence/accessibility/gp-1-manual-walkthrough.md` | unassessed |
| `P8.F5.T2` | `P8.F5` | Test keyboard-only operation, high contrast, reduced motion, focus order, live regions | done | S1 / XQ-02 | `crates/legion-desktop/tests/accessibility.rs` | unassessed |
| `P8.F5.T3` | `P8.F5` | Validate IME/CJK, clipboard, file dialogs, keyring, PTY/ConPTY, watcher behavior on all supported OSes | done | S1 / XQ-02 | `plans/evidence/platform-parity/P8-F5-T3-platform-parity-report.md` | unassessed |
| `P9.F1.T1` | `P9.F1` | Build 20-50 fixture repo tasks scored by tests, diff scope, cost, and turns | done | S3-S5 with final S6 | `plans/evidence/production/BENCH/recorded-execution-gate-v1.md` | unassessed |
| `P9.F1.T2` | `P9.F1` | Add prompt-injection/exfiltration/hostile-file/tool-output fixtures | done | S3-S5 with final S6 | `plans/evidence/production/M10/PKT-EVAL-evidence.md` | unassessed |
| `P9.F1.T3` | `P9.F1` | Run offline in CI with recorded providers and optionally live on schedule | done | S3-S5 with final S6 | `plans/evidence/production/BENCH/recorded-execution-gate-v1.md` | unassessed |
| `P9.F1.T4` | `P9.F1` | Freeze the raw-model bench baseline before any governed-layer code lands (roadmap Phase 0.6) | done | S3-S5 with final S6 | `plans/evidence/production/BENCH/recorded-execution-gate-v1.md` | unassessed |
| `P9.F2.T1` | `P9.F2` | Update public security model to match actual sandbox guarantees and Windows caveats | done | S5 extensions/team | `docs/SECURITY.md` | unassessed |
| `P9.F2.T2` | `P9.F2` | Add secret scanning for proposal content, terminal excerpts, and retained/ejected context | done | S5 extensions/team | `plans/evidence/production/2026-08-17-secret-scanning-ruleset.md` | unassessed |
| `P9.F2.T3` | `P9.F2` | Add signed policy bundles: provider allowlists, MCP/tool allowlists, mode ceilings, budget caps, retention/export rules | done | S5 extensions/team | `plans/evidence/production/P9-F2-T3-signed-policy-bundles.md` | unassessed |
| `P9.F2.T4` | `P9.F2` | Schedule external audit/pen test before strong enterprise claims | in-progress | S5 extensions/team | `plans/evidence/security/P9-F2-T4-external-audit-gate.md` | unassessed |
| `P9.F3.T1` | `P9.F3` | Ratify CRDT/operation-layer ADR over the accepted collaboration substrate | done | S5 extensions/team | `plans/adrs/ADR-0045-collaboration-operation-layer.md` | unassessed |
| `P9.F3.T2` | `P9.F3` | Activate LAN/reference remote transport with reconnect/offline evidence | done | S5 extensions/team | `plans/evidence/remote/P9-F3-T2-reconnect-offline-evidence.md` | unassessed |
| `P9.F3.T3` | `P9.F3` | Productize Cloud Lane with visible upload scope, budget, cancellation, and egress manifest | done | S5 extensions/team | `plans/evidence/production/PR-ENT-002/P9-F3-T3-cloud-lane-egress-and-cancellation.md` | unassessed |
| `P9.F3.T4` | `P9.F3` | Keep these out of GA unless explicitly pulled forward with evidence | done | S5 extensions/team | `plans/evidence/production/WS-P0/2026-08-19-deferred-surface-gate.md` | unassessed |
| `P9.F4.T1` | `P9.F4` | Convert consented acceptance/rejection metadata into eval/training candidates | done | S5 extensions/team | `plans/evidence/production/P9-F4-consented-training-corpus-and-raw-trace-controls.md` | unassessed |
| `P9.F4.T2` | `P9.F4` | Enforce raw trace opt-in, redaction, deletion handles, and export controls | done | S5 extensions/team | `plans/evidence/production/P9-F4-consented-training-corpus-and-raw-trace-controls.md` | unassessed |
| `P9.F4.T3` | `P9.F4` | Run QLoRA/adapter training only from consented corpora and compare to Legion-Bench baseline | done | S5 extensions/team | `plans/evidence/training-flywheel/P9-F4-T3-consented-corpus-legion-bench-comparison.md` | unassessed |
