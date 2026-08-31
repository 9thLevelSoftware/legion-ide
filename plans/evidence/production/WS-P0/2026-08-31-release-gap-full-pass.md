# 2026-08-31 release-gap full pass

**Kind:** evidence capture, not a promotion.  
**Workflow:** `.grok/workflows/release-gap-analysis.rhai` (`scope=full`, `claim=preview`).  
**Scored head:** `4ed8617142086c45f1b33b94503bf08ff314be27`.  
**Baseline audit head:** `7f67795d87fbc728d1e62851ba9fd451ca47f3fa` (2026-08-24).  
**Sequenced close-out:** [`plans/p0-installed-product-sequence-v0.1.md`](../../../p0-installed-product-sequence-v0.1.md).

This file is the verbatim report from the full-pass run. It does **not** change any product-readiness ledger row. Ten P0 blockers remain below installed-artifact evidence. Fail-closed: missing rendered or installed proof cannot raise a claim.

---

# Legion IDE release gap analysis

Scope: `full`  
Intended claim: `preview`  
Baseline audit head: `7f67795d87fbc728d1e62851ba9fd451ca47f3fa`  
Current head (snapshot): `4ed8617142086c45f1b33b94503bf08ff314be27`  
Commits since baseline: GitHub/origin main is still the baseline 7f67795d87fbc728d1e62851ba9fd451ca47f3fa (0 hosted commits after 2026-08-24). Local HEAD is merge 4ed86171 of that baseline plus ~37 local cherry-pick/fix commits (Vim/Git worker/DAP honesty/search-off-thread/MCP stdio/a11y/PR-17 notes) then 'Merge branch main of github.com/9thLevelSoftware/legion-ide'.

## Executive verdict

The intended preview claim is not honest: ten open P0 rows sit at evidence levels 1–3, and none of them is a GUI-driven, installed-artifact E2E on Windows, macOS, and Linux. Standing gates and recorded bench are merge-blocking cargo/xtask evidence; golden-path 1–5, update-drill, and preview GUI --beta-smoke remain independent, continue-on-error, or AppComposition/headless rather than a packaged desktop. The gap is the same in the ledger: the sole Product-workflow-validated row (PR-AI-001) names cargo and projection-string tests of a deterministic-local fixture, not a rendered session or 3-OS installer, while PR-UI-001/002, PR-LANG-001, and PR-REL-001 stay substrate or in-progress. Unsigned-beta zip/tar.gz layout smoke exists and is required, but GUI smoke is continue-on-error, native-package verify is unsigned headless --beta-smoke, and plans/evidence/dogfood has no filled installed-preview journal. Authenticode, Developer ID, and notarization are blocked on open EXT-CERT-* items; the updater only rewrites a TOML journal; hot-exit still drops unsaved buffer text; and AGENTS.md/ledger prose still contradict hosted legion-gates.yml and legion-release.yml. Origin main is unchanged at 7f67795; local HEAD 4ed86171 is not hosted evidence. Preview and every higher claim stay No-go until those P0s reach installed, renderer-backed level-5 evidence; even source-tree dogfood is Not yet.

## Release-claim table

| Intended release claim | Verdict |
| --- | --- |
| Internal developer dogfood | Not yet |
| Invitation-only technical preview | No-go |
| Public alpha | No-go |
| Focused Rust-first IDE GA | No-go |
| General-purpose all-languages IDE GA | No-go |
| Fully autonomous IDE GA | No-go |
| Enterprise-ready IDE | No-go |

## Evidence ladder

1. Unit  2. Subsystem  3. Desktop reachability  4. Rendered GUI  5. Installed signed artifact on clean 3-OS.
No feature is product-validated until level 5. Missing skeptic evidence fails closed.

## Blocker scores

| ID | Gap | Level | Status | Overstated | Remaining |
| --- | --- | --- | --- | --- | --- |
| P0-01 | Installed-product truth | 3 application reachability | in-progress | true | The required GUI-driven installed-artifact E2E on Windows, macOS, and Linux is absent: golden-path-5 and legion-smoke.yml are cargo-run AppComposition, preview GUI smoke is continue-on-error, and native-package verify is unsigned headless --beta-smoke. |
| P0-02 | Release signing | 2 subsystem integration | blocked | true | Authenticode, Developer ID + notarization, Linux package signatures, and trusted production release metadata are still absent; CI publishes only unsigned-beta/no-os-code-signing artifacts. |
| P0-03 | Update safety | 2 subsystem integration | in-progress | false | Real binary replacement, process restart, interruption resume, hosted feed, and N-1 artifact restore are still unimplemented; apply/rollback only rewrite a TOML journal. |
| P0-04 | Data safety | 3 application reachability | in-progress | true | Hot-exit still drops unsaved buffer text (session JSON is metadata-only), local-history indexes stay in-memory across restart, and ADR-0005’s SQLite/corruption-repair baseline is still JSON quarantine rather than a packaged 3-OS recovery workflow. |
| P0-05 | Accessibility | 3 application reachability | substrate | true | NVDA/Narrator, VoiceOver, and Orca transcripts, committed macOS/Linux OS-tree probes, and a renderer-backed keyboard-only/focus certification walk are still missing. |
| P0-06 | Manual / no-AI proof | 3 application reachability | substrate | true | There is still no separate signed Manual/offline installer, and plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md still only claims AppComposition smoke rather than OS-level no-egress. |
| P0-07 | Governance | 1 unit | in-progress | true | Protected main, GitHub required checks, and a release-blocker issue queue are still unmet: `.github/CODEOWNERS` is missing, branch-protection/rulesets are not in the tree (GitHub protection API 401), QUAL.11 has no taxonomy, and `.github/ISSUE_TEMPLATE/` only has `bug_report.md` labeled `bug`. |
| P0-08 | Documentation truth | 2 subsystem integration | overstated | true | claim-audit, docs-hygiene, and readiness-consistency still pass while AGENTS.md, ledger evidence cells, ADR-0046 citations, Appendix A, USER_GUIDE, and the SmallCode attribution table contradict hosted workflows, retired freeze text, search-worker code, and PR-AI-001/002 statuses. |
| P0-09 | Performance | 3 application reachability | substrate | true | Renderer-backed 3-OS startup, 100MB edit, search, indexing, and process-memory budgets are still unproven: product_perf is headless AppComposition, m9 has no committed large_file_manual_renderer_perf.toml, search/indexing remain report-only or a 30s open-path catastrophe guard, and hosted renderer failures stay fail-open. |
| P0-10 | Support / legal | 2 subsystem integration | in-progress | false | There is still no LICENSE or user-facing privacy policy, cargo/third-party notices are not generated or bundled, diagnostics export is a launch-arg/operator artifact rather than an in-app support bundle, and nothing ships on a signed 3-OS installer. |
| P1-01 | Core workbench completeness | 2 subsystem integration | in-progress | false | Editor splits never leave the empty `layout_splits` DTO, conflict handling is whole-file Use Current/Incoming rather than a three-way merge editor, and accessibility/keybinding “profiles” stay serde-only with no user profile switch. |
| P1-02 | Language platform | 2 subsystem integration | substrate | true | There is no Language Pack SDK or product toolchain manager: LanguageServerAdapterRegistry is test-only, evaluate_rust_analyzer_download is never called from composition, plugin register_lsp_adapter is a WIT fixture, and the desktop path only PATH-discovers rust-analyzer. |
| P1-03 | Hero language matrix | 2 subsystem integration | in-progress | true | Certified TS/JS, Python, Go, and Java/C# workflows are missing: product launch is rust-analyzer-only (Cargo.toml-gated), Java/C# have no adapters or grammars, and no renderer-backed or signed 3-OS matrix evidence exists. |
| P1-04 | Debug platform | 3 application reachability | substrate | false | Live DAP still sends line-only setBreakpoints (dropping condition/hit/log), F9 and the toolbar never collect advanced breakpoint fields, and there is no windowed or signed 3-OS GUI proof of a real adapter. |
| P1-05 | Test platform | 3 application reachability | substrate | true | There is still no pluggable TestController implementation or coverage API—only a cargo-test/LSP-runnable explorer—and PR-LANG-002 still treats coverage as out of scope without a windowed or 3-OS GUI packet. |
| P1-06 | SCM | 3 application reachability | substrate | true | Hosting-provider extensions are github.com/gitlab.com compare/MR URL builders with no GitHub/GitLab API adapters, no self-hosted hosts, and no windowed or signed 3-OS SCM session. |
| P1-07 | AI setup | 3 application reachability | substrate | true | Settings can select an already-running Ollama/llama.cpp server or Anthropic BYOK, but there is no in-app model catalog, downloader, or sidecar, first-run does not complete provider setup, Assist still falls back to deterministic-local without an external server, and live-model evals remain skip/deferred. |
| P1-08 | Diagnostics | 1 unit | in-progress | true | Native out-of-process fault capture, symbolication, safe mode, and a composed support-bundle path are still missing: the panic hook is unwired, minidump.rs only emits metadata, and SupportBundleAssembler is unused by AppComposition. |
| P2-01 | VSIX management | 3 application reachability | in-progress | true | Open VSX/VSIX packages are still metadata-only and unwired, the shipping catalog is an in-memory first-party grammar signed with a committed development seed, and install does not persist, unpack a VSIX, or load the wasm host. |
| P2-02 | Node extension host | 0 unverified / absent | deferred | false | There is still no versioned Node sidecar, vscode API/RPC bridge, or remote extension-host process; WASM isolation in legion-plugin is a different local engine and is not composed as WasmPluginHost. |
| P2-03 | Web extension host | 0 unverified / absent | deferred | false | No web-worker sidecar, vscode API facade, or RPC bridge exists; WebWorkerSidecar is only a planning DTO and PR-VSC-002 stays deferred without ADR-0052. |
| P2-04 | Contribution API | 1 unit | in-progress | true | Views, tasks, tests, debuggers, SCM, notebooks, webviews, and custom editors are only classified in legion-vscode-compat (Tier3 notebooks/webviews/customEditors stay Deferred); they are not PluginContribution/WIT host APIs and that crate is unwired from product binaries. |
| P2-05 | Remote development | 1 unit | deferred | true | There is still no SSH remote-server process or peer schema/version negotiation; connect is an in-memory planner plus handshake DTO checks. |
| P2-06 | Dev Containers | 2 subsystem integration | deferred | false | Standards-based Dev Containers (read `.devcontainer/devcontainer.json`, start image/Dockerfile/features/mounts, attach FS/LSP/PTY) are still unimplemented; later local commits did not close it. |
| P2-07 | Remote extension placement | 1 unit | deferred | false | Ui versus workspace kinds are classified, but no path runs UI extensions locally and workspace extensions on a remote host; PR-VSC-002 and PR-ENT-001 stay deferred. |
| P3-01 | Durable agent runtime | 0 unverified / absent | not-started | false | No independent persistent legion-agentd exists: agent work is an in-process library plus an AppComposition process-lifetime worker thread that dies with the desktop (orphaned sandboxes are reaped on startup). |
| P3-02 | Multi-agent orchestration | 2 subsystem integration | substrate | true | Automate still cannot create or execute a multi-session DAG from the desktop, parent/child subagent budget inheritance is an unused protocol field, and GP-4 remains a headless AppComposition harness rather than a 3-OS signed GUI workflow. |
| P3-03 | Strong isolation | 2 subsystem integration | in-progress | true | Windows spawn is still job-object-only with no AppContainer, restricted-token, or VM filesystem/network isolation, and no backend reports OS-level read confinement or Linux selective egress, so the required strong-isolation outcome is not met. |
| P3-04 | Autonomous PR workflow | 2 subsystem integration | substrate | false | A merge-ready Automate session still never creates a git branch or a forge pull request: GitForge only builds a compare URL, GP-4 stops at evidence-bundle replay, and execute_legion_workflow is not invoked from legion-desktop. |
| P3-05 | Current ACP/MCP | 2 subsystem integration | in-progress | true | The tree still speaks a 2025-11-25-shaped MCP subset (no client initialize, no MCP-Protocol-Version/_meta version, extra schema_version on envelopes) plus an env-var subprocess ACP bridge, not MCP 2026-07-28 or ACP v1 JSON-RPC (initialize/session/new/session/prompt). |
| P3-06 | Enterprise administration | 1 unit | deferred | false | SSO, SCIM, enterprise RBAC, desktop fleet-policy installation, and a real audit-export workflow are still missing, and PR-ENT-002 remains deferred. |
| P3-07 | Production collaboration | 2 subsystem integration | deferred | true | There is no authenticated durable multi-user collaboration service: sessions and OT live in-process, default-off, with metadata-only audit maps rather than persisted op-logs, identity, or a network service. |

P0 rows below level 5: **10**  
Overstated rows: **21**

## Ledger notes

- Status vocabulary in plans/product-readiness-ledger.md: Not started; In progress; Substrate validated; Product workflow validated (complete e2e user-facing workflow with named evidence and targeted tests); Deferred with explicit cut line; Blocked.
- Readiness Matrix gates: PR-UI-001/002 Substrate validated; PR-LANG-001/002 Substrate validated; PR-AI-001 Product workflow validated; PR-AI-002 Substrate validated (proposal safety + adversarial evals); PR-VSC-001 Substrate validated; PR-VSC-002/PR-ENT-001/PR-ENT-002 Deferred with explicit cut line; PR-REL-001 In progress.
- Sole Product-workflow-validated row is PR-AI-001. Evidence names cargo tests only (control_trust_surfaces 8, control_trust_view 4, retention tombstone 1, assist_inline_prediction app 6 / desktop 1). control_trust_view asserts DesktopProjectionViewModel string rows from shell_projection_snapshot with ProductAiProviderPreference::Deterministic; it does not name a rendered GUI session or installed 3-OS artifacts.
- PR-AI-001 remaining-gap prose: default Assist/inline/chat still routes through deterministic-local fixture; 'Do not read this row as real model by default in the GUI.' Promotion evidence file plans/evidence/production/M3/WS14-T5-privacy-inspector-productization.md is the same test list.
- PR-UI-001/002 PR-17 decision (2026-08-24) keeps both at Substrate validated: Windows-only a11y probe, no NVDA/VoiceOver/Orca; no measured large_file_manual_renderer_perf.toml / no 3-OS paint.
- PR-REL-001 cites unsigned-beta 3-OS preview run 29887799213 and portable zip/tar.gz; remaining gaps include signed installers, MSI/DMG/deb, fresh-VM Gatekeeper/SmartScreen, hosted update feed.
- Beta Acceptance Scenario still requires VSIX install and collaborate-on-review while PR-VSC-002 and PR-ENT-002 are deferred; consistency note 2026-08-14 says that scenario is not reachable as written.

## CI notes

- .github/workflows/legion-gates.yml: on push main + pull_request + workflow_dispatch; jobs gates (ubuntu/windows/macos, timeout 180) and deny (ubuntu cargo-deny). Header: a red run is a merge blocker. Explicitly excludes golden-path smokes and evals/training pytest.
- gates job runs xtask check-deps/docs-hygiene/claim-audit/no-egui-textedit/extract-before-modify/intent-reachability/deferred-surfaces/verify-kanban-backlog/verify-readiness-consistency/release-pipeline dry-run/verify-release-pipeline, fmt, check (incl. legion-app no-default-features and legion-desktop --no-default-features --features offline), cargo test --workspace, governors-off retest, clippy -D warnings, perf-harness + verify-perf-harness with LEGION_PERF_FAIL_ON_BUDGET_MS=0 (product budgets/coverage armed; skeleton budgets report-only; continue-on-error forbidden on those steps), rust-analyzer-smoke if rustup component add succeeds (provision is continue-on-error).
- .github/workflows/legion-bench.yml: push main + PR; recorded replay execution + verify-legion-bench (+ raw governors-off arm) on windows-latest only. legion-bench-live.yml: schedule/dispatch, continue-on-error, no push/PR, cannot gate a provider.
- .github/workflows/legion-smoke.yml: independent (failures do not block PR merges); workflow_dispatch + weekly Monday 06:00 UTC; 3-OS jobs smoke (GP-1), smoke-gp2/3/4, smoke-manual-loop (xtask golden-path-5), update-drill. Header states GP-5 name collision: this job is the manual editing loop, not roadmap Phase 6 extension-constrained GP-5.
- .github/workflows/legion-preview.yml: independent weekly/dispatch unsigned-beta 3-OS zip/tar.gz; layout smoke required; 'Smoke — beta workflow (best-effort headless)' is continue-on-error. legion-release.yml is workflow_dispatch verify-only/publish, not a PR gate. legion-dap-dogfood.yml is independent except path-filtered PRs on legion-debug/workflow.
- docs/OPERATOR_RUNBOOK.md: do not promote smoke to required checks until four consecutive green 3-OS scheduled runs + owner sign-off. AGENTS.md standing-gate list includes GP-1..4 and update-drill locally; gates.yml does not run those on PRs.

## Claim drift

- PR-AI-001 is Product workflow validated on green cargo/projection-row tests, not a packaged desktop or 3-OS installed GUI; default path is deterministic-local fixture.
- docs/USER_GUIDE.md line 3 assumes 'a working build or a packaged desktop app' while the caveat says the repo is not a renderer-backed daily-driver.
- AGENTS.md WS18.T1 still says no hosted validate job runs perf-harness and describes a skeleton stand-in; legion-gates.yml now runs product-workload perf-harness + verify-perf-harness on the 3-OS PR matrix.
- AGENTS.md WS17.T1 says 'No hosted release workflow is currently configured'; .github/workflows/legion-release.yml exists as manual unsigned-beta native installer release.
- Ledger PR-LANG-001 still says '3-OS hosted CI smoke is deferred pending CI infrastructure'; rust-analyzer-smoke is a merge-blocking gates.yml step when provisioning succeeds.
- Ledger PR-UI-001/GP-1 prose still says '3-OS CI pending via legion-smoke.yml'; the workflow is already present but independent, so a green PR is not 3-OS golden-path evidence.
- plans/legion-production-master-plan-v0.2.md Appendix A says PR-AI-002 'adversarial evals deferred'; the ledger status is Substrate validated with hostile-eval tests and xtask hostile-evals named.
- README/AGENTS 21 local standing gates include golden-path-1..4; CI green on legion-gates.yml + recorded bench is not the packaged desktop (preview GUI smoke is continue-on-error; smoke/update-drill do not block merges).

## Next actions

1. [P0-01] Add merge-blocking 3-OS jobs that install or extract native packages and drive a windowed GUI E2E to hard-fail, instead of cargo-run golden-path-5 or headless --beta-smoke.
1. [P0-08] Widen claim-audit and verify-readiness-consistency to fail on AGENTS.md, ledger evidence cells, Appendix A, and workflow/ADR mismatches, then rewrite the stale sentences so a green gates.yml run means those artifacts agree.
1. [P0-07] Add .github/CODEOWNERS, a GitHub ruleset that requires the Standing gates / cargo-deny and Legion bench recorded checks on main, and a release-blocker issue template plus label for QUAL.11.
1. [P0-04] Add crash-safe unsaved-buffer snapshots restored on relaunch, persist/reload local-history metadata from .legion/local-history, and cover a killed dirty desktop session in legion-desktop tests.
1. [P0-02] Issue and wire EXT-CERT-WIN/MAC/LIN into legion-release.yml (signtool/codesign/notarytool/minisign) so signer_status is no longer forced unsigned-beta, then archive real 3-OS fresh-VM Gatekeeper/SmartScreen evidence.
1. [P0-03] Wire a signed HttpManifestSource plus installer swap/restart/N-1 restore through AppComposition and legion-desktop, then extend update-drill past the journal FSM.
1. [P0-09] Commit measured 3-OS renderer reports (including large_file_manual_renderer_perf.toml), add github-hosted trend baselines, move LexicalIndexer off the open path, and fail-closed armed budgets for search/indexing/memory instead of LEGION_PERF_FAIL_ON_BUDGET_MS=0 skips.
1. [P0-05] Record NVDA/Narrator, VoiceOver, and Orca sessions on a live desktop window, commit AXUIElement and AT-SPI probes beside scripts/a11y-uia-walk.ps1, and finish the renderer-backed keyboard-only path named in plans/evidence/accessibility/PR-15-manual-keyboard-path.md.

## Per-blocker remaining work

### P0-01 — Installed-product truth

- Required: GUI-driven, installed-artifact E2E tests on all three operating systems
- Level: 3 application reachability
- Status: in-progress
- Overstated: true
- Unit: crates/legion-desktop/src/smoke.rs (eframe::run_native --smoke); crates/legion-desktop/src/beta.rs (DesktopRuntime --beta-smoke, no window); crates/legion-desktop/tests/packaging.rs (plan/manifest only).
- Subsystem: crates/legion-app/src/bin/golden_path_5.rs + xtask/src/golden_path_5.rs (AppComposition edit/save/highlight/terminal/git, no legion-desktop); crates/legion-desktop/tests/beta_workflow.rs and tests/beta_acceptance_e2e.rs; scripts/test-native-package-verifiers.ps1.
- Reachability: crates/legion-desktop/src/workflow.rs run_from_env: --beta-smoke -> beta::run_beta_workflow (DesktopRuntime+AppComposition), --smoke -> smoke::run_smoke, else run_native; .github/workflows/legion-gates.yml does not run golden-path or installed GUI E2E.
- Rendered: plans/evidence/dogfood/2026-08-17-interactive-gui-journal.md is Windows cargo run -p legion-desktop, not an installed artifact and not 3-OS; crates/legion-desktop/tests/explorer_activation.rs and tests/headless_input.rs are headless AccessKit/egui seams; scripts/gui-smoke.sh --beta still launches --beta-smoke.
- Installed: .github/workflows/legion-release.yml (workflow_dispatch) plus scripts/verify-native-package.sh/.ps1 extract MSI/DMG/deb/AppImage and hard-fail headless --beta-smoke (policy=hard-fail-beta-workflow-is-headless); .github/workflows/legion-preview.yml 3-OS zip/tar.gz layout smoke is required, GUI --beta-smoke is continue-on-error with || true; plans/evidence/dogfood/INSTALLED-PREVIEW-CHECKLIST.md has no filled YYYY-MM-DD-installed-preview-journal.md; PR-REL-001 in plans/product-readiness-ledger.md remains In progress (unsigned-beta, no signed clean-VM).
- Remaining: The required GUI-driven installed-artifact E2E on Windows, macOS, and Linux is absent: golden-path-5 and legion-smoke.yml are cargo-run AppComposition, preview GUI smoke is continue-on-error, and native-package verify is unsigned headless --beta-smoke.
- Next: Add merge-blocking 3-OS jobs that install or extract native packages and drive a windowed GUI E2E to hard-fail, instead of cargo-run golden-path-5 or headless --beta-smoke.

### P0-02 — Release signing

- Required: Authenticode, Developer ID, notarization, trusted release metadata
- Level: 2 subsystem integration
- Status: blocked
- Overstated: true
- Unit: xtask/src/signing.rs and xtask/tests/manifest_sign.rs implement isolated Ed25519 sign/verify plus env/keyring/kms resolvers; KMS is honest-unavailable. No Authenticode, codesign, notarytool, or minisign unit path exists.
- Subsystem: xtask/src/release_pipeline.rs, crates/legion-protocol/src/release_manifest.rs, crates/legion-app/src/updater.rs, crates/legion-app/tests/upd_tests.rs, and crates/legion-app/src/bin/update_drill.rs collaborate on unsigned-beta or Ed25519 release-manifest metadata below the desktop shell.
- Reachability: No matches for updater/check_for_update in crates/legion-desktop or AppComposition; scripts/package-native.ps1, scripts/package-native.sh, .github/workflows/legion-release.yml, and .github/workflows/legion-preview.yml hardcode signer_status=unsigned-beta/no-os-code-signing.
- Rendered: No GUI update or signed-install workflow; docs/OPERATOR_RUNBOOK.md tells testers to bypass SmartScreen/Gatekeeper on unsigned packages.
- Installed: plans/evidence/release/P8-F1-T3-fresh-vm-gatekeeper-smartscreen-install-smoke.md is a descriptor checkpoint, not a fresh-VM Gatekeeper/SmartScreen proof; plans/release/procurement-and-key-escrow.md leaves EXT-CERT-WIN/LIN open and Apple certs unissued.
- Remaining: Authenticode, Developer ID + notarization, Linux package signatures, and trusted production release metadata are still absent; CI publishes only unsigned-beta/no-os-code-signing artifacts.
- Next: Issue and wire EXT-CERT-WIN/MAC/LIN into legion-release.yml (signtool/codesign/notarytool/minisign) so signer_status is no longer forced unsigned-beta, then archive real 3-OS fresh-VM Gatekeeper/SmartScreen evidence.

### P0-03 — Update safety

- Required: Real replacement, restart, rollback, interruption and N-1 upgrades
- Level: 2 subsystem integration
- Status: in-progress
- Overstated: false
- Unit: crates/legion-app/src/updater.rs (Ed25519-before-parse, LocalDirManifestSource only, stage SHA-256 copy, apply_update/rollback journal toggle; binary swap/restart explicitly out of scope) plus crates/legion-app/tests/upd_tests.rs (version compare, sig/hash/channel/downgrade, journal apply/rollback/double-rollback).
- Subsystem: crates/legion-protocol/src/release_manifest.rs; crates/legion-app/src/bin/update_drill.rs (upd-drill s1–s11, zero-egress); xtask/src/update_drill.rs and xtask/src/main.rs (`cargo run -p xtask -- update-drill`); independent 3-OS job in .github/workflows/legion-smoke.yml (not a PR merge gate).
- Reachability: No legion-desktop or AppComposition caller: updater is only `pub mod updater` in crates/legion-app/src/lib.rs and is used by upd_tests.rs and update_drill.rs; no HttpManifestSource (plans/evidence/production/WS-A-D/phase-4-release/D3-update-channel-staging.md D3.1 still open).
- Rendered: None: legion-ui has no auto-update UI; docs/USER_GUIDE.md lists signing/notarization/auto-update as local drills only.
- Installed: None: plans/product-readiness-ledger.md PR-REL-001 remains In progress (unsigned-beta zip/tar.gz; hosted update feed and installer-swap/process-restart ADR-0042 D5 deferred); plans/evidence/production/M12/PKT-UPDATER-evidence.md documents the same cut line.
- Remaining: Real binary replacement, process restart, interruption resume, hosted feed, and N-1 artifact restore are still unimplemented; apply/rollback only rewrite a TOML journal.
- Next: Wire a signed HttpManifestSource plus installer swap/restart/N-1 restore through AppComposition and legion-desktop, then extend update-drill past the journal FSM.

### P0-04 — Data safety

- Required: Hot exit, unsaved recovery, atomic save, crash and corruption recovery
- Level: 3 application reachability
- Status: in-progress
- Overstated: true
- Unit: crates/legion-platform/src/lib.rs NativeFileSystem::write_text_file_atomic (temp+replace+sync) and atomic_write_* tests; crates/legion-storage/src/lib.rs FileBackedStorage::open quarantine; crates/legion-storage/src/local_history.rs in-memory LocalHistoryMetadataStore (cross-session persistence deferred); crates/legion-storage/src/checkpoint.rs skips unparsable checkpoint files; crates/legion-project/src/lib.rs save_file_with_proposal fail-closed non-atomic path.
- Subsystem: crates/legion-app/tests/workspace_vfs_integration.rs external-overwrite stale save keeps dirty text; crates/legion-app/tests/checkpoint_restore.rs durable proposal checkpoints; crates/legion-app/tests/local_history_workflow.rs save-time blobs + proposal restore; crates/legion-app/src/lib.rs save_active_buffer / restore_workspace_session_record (reopens disk, does not reapply dirty body).
- Reachability: crates/legion-desktop/src/workflow.rs DesktopRuntime::open enables .legion persistence and default .legion/session.json; DesktopAction::SaveActive / SaveDirtyClose / RestoreCheckpoint (Alt+Z); crates/legion-desktop/src/bridge.rs Save* intents; crates/legion-desktop/tests/desktop_workflow.rs and save_all_conflict.rs; crates/legion-desktop/src/session.rs crash-safe metadata JSON that rejects raw buffer markers.
- Rendered: crates/legion-desktop/src/view.rs render_close_dirty_prompt_modal; crates/legion-desktop/tests/shell_snapshots.rs unsaved-changes-prompt plus snapshots/unsaved-changes-prompt-*.png; crates/legion-desktop/tests/daily_editing_controls.rs dirty-close prompt. No desktop local-history panel (legion-ui local_history_entries unused in view). crates/legion-desktop/tests/session_restore.rs proves SECRET_DIRTY_BODY is not persisted and restore reloads disk text.
- Installed: No signed 3-OS installer proof of hot-exit/unsaved recovery. plans/product-readiness-ledger.md PR-REL-001 still In progress (unsigned-beta zip/tar.gz only).
- Remaining: Hot-exit still drops unsaved buffer text (session JSON is metadata-only), local-history indexes stay in-memory across restart, and ADR-0005’s SQLite/corruption-repair baseline is still JSON quarantine rather than a packaged 3-OS recovery workflow.
- Next: Add crash-safe unsaved-buffer snapshots restored on relaunch, persist/reload local-history metadata from .legion/local-history, and cover a killed dirty desktop session in legion-desktop tests.

### P0-05 — Accessibility

- Required: NVDA/Narrator, VoiceOver, Orca, keyboard-only and focus certification
- Level: 3 application reachability
- Status: substrate
- Overstated: true
- Unit: crates/legion-protocol/src/lib.rs WorkbenchAccessibilityProfile; crates/legion-desktop/src/view.rs and view/canvas_workspace.rs accesskit_node_builder roles; crates/legion-desktop/tests/accessibility.rs accessibility_profile_round_trips_high_contrast_and_reduced_motion_flags.
- Subsystem: crates/legion-desktop/tests/accessibility.rs (AccessKit roles/names/28px targets, live-region projection, Windows UIA parse/status honesty, PR-15 unobserved-platform contract) plus crates/legion-desktop/tests/keyboard_nav.rs (headless Tab/arrow/Enter mode switch, modal Confirm/Escape, focus restore).
- Reachability: crates/legion-desktop/src/workflow.rs DesktopRuntime holds AppComposition and DesktopEframeApp::ui calls render_app_frame; run_native/desktop_native_options launch the same adapter; handle_keyboard consumes AccessKit Focus; crates/legion-desktop/src/platform.rs probe_windows_uia_tree is invoked from accessibility_tree_status outside a test-only stub; scripts/a11y-platform-probe.sh delegates Windows to scripts/a11y-uia-walk.ps1 and exits unobserved on macOS/Linux.
- Rendered: plans/evidence/production/PR-UI-001/2026-08-16-windows-uia-tree.txt is a live Windows UIA dump of Legion IDE Smoke (138 descendants), not NVDA/Narrator; plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md is a one-off unreproducible macOS AX dump; plans/evidence/accessibility/README.md and gp-1/gp-2/gp-3 walkthroughs admit they are reconstructions, not screen-reader sessions; docs/KEYBOARD_REFERENCE.md plus PR-15 still leave keymap-only routes pending.
- Installed: No signed 3-OS packaged accessibility certification; .github/workflows has no accessibility job; legion-desktop --smoke in plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md still records OS tree not observed.
- Remaining: NVDA/Narrator, VoiceOver, and Orca transcripts, committed macOS/Linux OS-tree probes, and a renderer-backed keyboard-only/focus certification walk are still missing.
- Next: Record NVDA/Narrator, VoiceOver, and Orca sessions on a live desktop window, commit AXUIElement and AT-SPI probes beside scripts/a11y-uia-walk.ps1, and finish the renderer-backed keyboard-only path named in plans/evidence/accessibility/PR-15-manual-keyboard-path.md.

### P0-06 — Manual / no-AI proof

- Required: Separate signed Manual build plus OS-level no-egress evidence
- Level: 3 application reachability
- Status: substrate
- Overstated: true
- Unit: crates/legion-app/Cargo.toml (ai default vs offline=dep:legion-ai without legion-ai-providers); crates/legion-app/src/offline_ai.rs; crates/legion-app/src/product_ai_policy.rs cfg(not(feature="ai")) live-inline stub; crates/legion-protocol/tests/manual_mode_silence.rs
- Subsystem: crates/legion-app/tests/manual_zero_egress.rs (AppComposition Manual open/edit/save/search, no hosted-provider records); plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md (explicitly not OS-level network denial)
- Reachability: crates/legion-desktop/src/workflow.rs DesktopRuntime::open uses AppComposition::new() which defaults to AppProductMode::Manual (crates/legion-app/src/lib.rs); crates/legion-desktop/Cargo.toml offline feature; .github/workflows/legion-gates.yml cargo check -p legion-desktop --no-default-features --features offline; crates/legion-desktop/src/view.rs authority ribbon paints “Manual · AI off · Workspace tools only”
- Rendered: Trust-boundary strings in crates/legion-desktop/src/view.rs::manual_control_rows are asserted only by crates/legion-desktop/tests/manual_renderer_evidence.rs and projection_rendering.rs and are not painted; crates/legion-desktop/tests/shell_snapshots.rs is headless kittest, which is not a user-completed no-egress GUI proof.
- Installed: scripts/package-native.sh and scripts/package-native.ps1 run cargo build --release -p legion-desktop (default ai); xtask/release-pipeline.example.toml and .github/workflows/legion-release.yml have no Manual/offline channel; plans/product-readiness-ledger.md PR-REL-001 remains unsigned-beta; crates/legion-desktop still always depends on legion-ai-providers (reqwest).
- Remaining: There is still no separate signed Manual/offline installer, and plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md still only claims AppComposition smoke rather than OS-level no-egress.
- Next: Add a signed 3-OS `--no-default-features --features offline` Manual package channel and record firewall or packet-capture no-egress on a clean machine per OS.

### P0-07 — Governance

- Required: Protected main, required checks, release-blocker issue queue
- Level: 1 unit
- Status: in-progress
- Overstated: true
- Unit: .github/workflows/legion-gates.yml (push main + pull_request; header claims a red run is a merge blocker) and .github/workflows/legion-bench.yml; xtask/tests/legion_bench_ci_contract.rs only asserts recorded-mode PR wiring, not required checks.
- Subsystem: Standing xtask/cargo/clippy/deny/perf/rust-analyzer-smoke steps in .github/workflows/legion-gates.yml plus recorded bench in .github/workflows/legion-bench.yml run on PRs; .github/workflows/legion-smoke.yml, legion-preview.yml, and legion-release.yml are independent and not merge gates.
- Reachability: Repo/GitHub process, not legion-desktop/AppComposition.
- Rendered: No GUI workflow; only .github/ISSUE_TEMPLATE/bug_report.md.
- Installed: No in-tree CODEOWNERS or ruleset; https://api.github.com/repos/9thLevelSoftware/legion-ide/branches/main/protection returned 401; rust-analyzer-smoke in legion-gates.yml is skippable when rustup provision fails.
- Remaining: Protected main, GitHub required checks, and a release-blocker issue queue are still unmet: `.github/CODEOWNERS` is missing, branch-protection/rulesets are not in the tree (GitHub protection API 401), QUAL.11 has no taxonomy, and `.github/ISSUE_TEMPLATE/` only has `bug_report.md` labeled `bug`.
- Next: Add `.github/CODEOWNERS`, a GitHub ruleset that requires the `Standing gates`/`cargo-deny` and `Legion bench recorded` checks on `main`, and a release-blocker issue template plus label for QUAL.11.

### P0-08 — Documentation truth

- Required: Ledger, ADR, release artifacts, tests, and claims automatically reconciled
- Level: 2 subsystem integration
- Status: overstated
- Overstated: true
- Unit: xtask/src/claim_audit.rs (forbidden-phrase + README caveat only); xtask/tests/claim_audit.rs; xtask/src/docs_hygiene.rs (links/ADR numbers/latest-plan); xtask/tests/docs_hygiene.rs; xtask/src/readiness_consistency.rs unit tests for Pn.Fm.Tk vs kanban.
- Subsystem: xtask/src/main.rs wires docs-hygiene, claim-audit, verify-readiness-consistency, deferred-surfaces; .github/workflows/legion-gates.yml runs them on ubuntu/windows/macos PRs; xtask/deferred-surfaces.toml still keys unfreeze to PR-UI-001 Product workflow validated.
- Reachability: Not invoked by legion-desktop or AppComposition; only xtask/CI. Desktop composition is not a documentation-truth path.
- Rendered: docs/USER_GUIDE.md is the user-facing guide (line 3 assumes a packaged app, then caveats substrate-only); that is not a GUI workflow that lets a user complete ledger/ADR/claim reconciliation.
- Installed: No signed 3-OS packaged app ships reconciled docs; AGENTS.md still denies a hosted release while .github/workflows/legion-release.yml is a manual unsigned-beta installer workflow.
- Remaining: claim-audit, docs-hygiene, and readiness-consistency still pass while AGENTS.md, ledger evidence cells, ADR-0046 citations, Appendix A, USER_GUIDE, and the SmallCode attribution table contradict hosted workflows, retired freeze text, search-worker code, and PR-AI-001/002 statuses.
- Next: Widen claim-audit and verify-readiness-consistency to fail on AGENTS.md, ledger evidence cells, Appendix A, and workflow/ADR mismatches, then rewrite the stale sentences so a green gates.yml run actually means those artifacts agree.

### P0-09 — Performance

- Required: Renderer-backed startup, edit, search, indexing and memory budgets
- Level: 3 application reachability
- Status: substrate
- Overstated: true
- Unit: xtask/src/perf_harness.rs (ADR-0048 16/32ms budgets, synthetic m0/m1 tripwires, m2 1MB TextBuffer ceiling, m8 50k search budget_millis=0); xtask/tests/perf_harness.rs; crates/legion-app/src/bin/large_file_perf.rs (headless EditorEngine 100MB open/viewport/edit, not paint).
- Subsystem: crates/legion-app/src/bin/product_perf.rs + xtask/src/perf_workloads.rs (p8.startup 30s guard with sync LexicalIndexer in crates/legion-app/src/lib.rs bind_opened_file; p8.input_to_paint/scroll/memory/search/100k as AppComposition, not GPU); crates/legion-app/src/search.rs SearchWorker; plans/evidence/production/WS-MANUAL-02/WS-MANUAL-02-evidence.md and large-file-typing-fix.md (text-model only); plans/evidence/perf-harness-trend/PR-24-search-budget.md (50k remains report-only).
- Reachability: legion-desktop main --manual-perf (crates/legion-desktop/src/workflow.rs run_from_env) drives DesktopRuntime + AppComposition through crates/legion-desktop/src/manual_perf.rs render_projection_once_for_perf (headless egui::Context::run_ui); xtask/src/main.rs append_product_workload_measurements / append_manual_renderer_measurement / append_large_file_measurement spawn those binaries; crates/legion-desktop/tests/manual_perf.rs; .github/workflows/legion-gates.yml runs perf-harness + verify-perf-harness on ubuntu/windows/macos with product ceilings armed and skeleton/renderer override 0.
- Rendered: No user-facing GUI path measures these budgets: --manual-perf is a CLI harness, not a windowed native run; plans/product-readiness-ledger.md PR-UI-001/002 stay Substrate validated (PR-17: no measured large_file_manual_renderer_perf.toml, no 3-OS paint); the only archived paint-adjacent row is plans/evidence/perf-harness-trend/entries/windows-91e707a89430-20260819-062416Z.toml on a Windows reference host (manual p50 4.0ms; m9 269us is the old text-model edit, not paint); required_measured_workloads() in xtask/src/main.rs exempts renderer coverage on headless hosts.
- Installed: None: PR-REL-001 remains In progress (unsigned-beta zip/tar.gz only); no signed 3-OS installer run of renderer-backed startup/edit/search/indexing/memory budgets; trend baseline.toml has windows/reference only, no github-hosted rows.
- Remaining: Renderer-backed 3-OS startup, 100MB edit, search, indexing, and process-memory budgets are still unproven: product_perf is headless AppComposition, m9 has no committed large_file_manual_renderer_perf.toml, search/indexing remain report-only or a 30s open-path catastrophe guard, and hosted renderer failures stay fail-open.
- Next: Commit measured 3-OS renderer reports (including large_file_manual_renderer_perf.toml), add github-hosted trend baselines, move LexicalIndexer off the open path, and fail-closed armed budgets for search/indexing/memory instead of LEGION_PERF_FAIL_ON_BUDGET_MS=0 skips.

### P0-10 — Support / legal

- Required: Privacy, licensing, support, diagnostics and third-party notices
- Level: 2 subsystem integration
- Status: in-progress
- Overstated: false
- Unit: crates/legion-desktop/src/diagnostics.rs (metadata-only markdown export); crates/legion-app/src/diagnostics.rs (SupportBundleAssembler, same-file tests only); crates/legion-desktop/src/view.rs SettingsSection::Privacy crash-reports checkbox; Cargo.toml license=Proprietary; README.md license notice; THIRD_PARTY_NOTICES.md + docs/legal/smallcode-attribution.md (SmallCode MIT only); docs/TROUBLESHOOTING.md; docs/USER_GUIDE.md Support and release surfaces; .github/ISSUE_TEMPLATE/bug_report.md; plans/evidence/production/M5/WS17-T6-docs-support-surface.md. No LICENSE or PRIVACY.md in the tree.
- Subsystem: crates/legion-desktop/tests/diagnostics_export.rs writes metadata-only files via DesktopRuntime; crates/legion-observability/tests/crash_capture_tests.rs + plans/evidence/production/M12/PKT-CRASH-evidence.md (local panic bundles, no upload); crates/legion-app/tests/settings.rs SetCrashReportsEnabled; crates/legion-desktop/tests/control_trust_view.rs privacy-inspector projection strings; crates/legion-retention/tests/privacy_deletion.rs. cargo-deny [licenses] in deny.toml is an allowlist, not a notices generator.
- Reachability: crates/legion-desktop/src/main.rs -> workflow::run_from_env; DesktopRuntime::persist_diagnostics_if_configured only when --diagnostics-export is set (workflow.rs); DesktopAction::OpenSettings / SetCrashReportsEnabled map through bridge.rs into AppComposition (lib.rs OpenSettings / SetCrashReportsEnabled). SupportBundleAssembler is never called from AppComposition. No desktop/app path loads LICENSE, a privacy policy, or THIRD_PARTY_NOTICES.md.
- Rendered: crates/legion-desktop/tests/projection_rendering.rs headless AccessKit asserts a Settings dialog with a Privacy pill; the Diagnostics rail (view.rs render_diagnostics_panel) shows internal console rows, not a support-bundle or legal-notices workflow. Headless fixture tests are not a user-completed GUI legal/support path; no About/Help/license/notices UI exists.
- Installed: scripts/package-preview.ps1 and crates/legion-desktop/src/package.rs copy the unsigned exe plus UNSIGNED-BETA.toml/manifest only; packaging/Packager.toml has copyright and no license/notices files. PR-REL-001 remains In progress (unsigned-beta zip/tar.gz). Not a signed 3-OS installed app.
- Remaining: There is still no LICENSE or user-facing privacy policy, cargo/third-party notices are not generated or bundled, diagnostics export is a launch-arg/operator artifact rather than an in-app support bundle, and nothing ships on a signed 3-OS installer.
- Next: Add LICENSE plus a privacy policy, generate and ship full third-party notices with the package, and wire a GUI Help/About path that exports the metadata-only support bundle through AppComposition.

### P1-01 — Core workbench completeness

- Required: Splits, restoration, merge editor, settings, profiles, input compatibility
- Level: 2 subsystem integration
- Status: in-progress
- Overstated: false
- Unit: crates/legion-protocol/src/lib.rs (`SessionLayoutSplit`/`SessionTabGroup`/`WorkbenchSettingsRecord`/`WorkbenchAccessibilityProfile`/`WorkbenchLayoutSettings.keybinding_profile_label`); crates/legion-protocol/tests/dto_contracts.rs; crates/legion-project/src/lib.rs (`parse_and_resolve_conflict`); crates/legion-editor/src/diff.rs (proposal LCS, not merge UI); crates/legion-ui/src/ui.rs (`default_keymap` has no split binding).
- Subsystem: crates/legion-app/src/lib.rs (`restore_workspace_session_record`, capture always `layout_splits: Vec::new()` + one `main` tab group); crates/legion-app/tests/settings.rs; crates/legion-app/tests/git_workflow.rs (`ResolveGitConflict` AcceptCurrent/Incoming); crates/legion-storage/src/lib.rs (session record); plans/adrs/ADR-0040-concurrent-edit-substrate.md (three-way view still WS-07.T2).
- Reachability: crates/legion-desktop/src/workflow.rs (`AppComposition::restore_workspace_session_record`, default `.legion/session.json`); crates/legion-desktop/src/view/dock_geometry.rs (panel splitters only); crates/legion-desktop/src/view.rs (`render_settings_panel`); crates/legion-desktop/src/view/source_control.rs (Use Current/Incoming); crates/legion-desktop/src/bridge.rs; crates/legion-desktop/tests/session_restore.rs; crates/legion-desktop/tests/git_workflow.rs; crates/legion-desktop/tests/ime_smoke.rs; crates/legion-desktop/tests/clipboard_smoke.rs; crates/legion-desktop/src/main.rs.
- Rendered: Headless AccessKit/clicks exist (crates/legion-desktop/tests/command_palette_behavior.rs Settings overlay; crates/legion-desktop/tests/source_control_reachability.rs) but are not a windowed user workflow; no split/merge-editor/profile GUI; plans/product-readiness-ledger.md PR-UI-001 remains Substrate validated; docs/USER_GUIDE.md still denies a renderer-backed daily-driver.
- Installed: None: plans/product-readiness-ledger.md PR-REL-001 is In progress (unsigned-beta zip/tar.gz, no signed 3-OS installer).
- Remaining: Editor splits never leave the empty `layout_splits` DTO, conflict handling is whole-file Use Current/Incoming rather than a three-way merge editor, and accessibility/keybinding “profiles” stay serde-only with no user profile switch.
- Next: Ship editor split/group commands that persist `SessionLayoutSplit`, a side-by-side merge editor on conflicted buffers, and a settings-profile switch, then prove them on the real desktop composition and a windowed GUI packet.

### P1-02 — Language platform

- Required: Language Pack SDK and real LSP/toolchain manager
- Level: 2 subsystem integration
- Status: substrate
- Overstated: true
- Unit: crates/legion-lsp/src/lib.rs (LspFramer, LanguageServerAdapterRegistry::tier_two hardcoded rust/ts/pyright/gopls, RustAnalyzerDiscovery PATH scan); crates/legion-app/src/language/download.rs (broker decision + sha256 verify, no HTTP fetch); crates/legion-plugin/wit/lsp.wit plus crates/legion-plugin/tests/wit_abi.rs RecordingHost; ADR-0034 still names lsp-types, but crates/legion-lsp/Cargo.toml does not depend on it.
- Subsystem: crates/legion-lsp/tests/{registry_contract,discovery_contract,lifecycle_contract,stdio_transport_contract,read_side_contract,write_side_contract,rust_analyzer_smoke}.rs; crates/legion-app/tests/{rust_analyzer_download_policy,rust_analyzer_session_handshake,rust_analyzer_doc_sync,rust_analyzer_read_requests,language_edit_proposal_routing,app_lsp_composition}.rs; product startup in crates/legion-app/src/language/app_lsp.rs uses RustAnalyzerDiscovery PATH only and never LanguageServerAdapterRegistry.
- Reachability: legion-desktop + AppComposition invoke the rust-analyzer session (crates/legion-app/src/lib.rs LspStartSession/LspRestartSession; crates/legion-desktop/src/workflow.rs drain_lsp_session + tick_lsp_debounces) but do not invoke the adapter registry, download/install manager, or plugin LSP SDK; crates/legion-app/src/lib.rs t1_open_rs_file_does_not_trigger_lsp_start contradicts docs/USER_GUIDE.md lazy-start.
- Rendered: GUI tests inject LSP rows rather than installing a language pack (crates/legion-desktop/tests/completion_popup.rs, problems_panel_rendering.rs, language_health_view.rs); no install/select-language-server workflow is rendered.
- Installed: No signed 3-OS packaged manager; .github/workflows/legion-gates.yml rust-analyzer-smoke (when rustup component add succeeds) is a merge-blocking real-server smoke, not an installed Language Pack SDK.
- Remaining: There is no Language Pack SDK or product toolchain manager: LanguageServerAdapterRegistry is test-only, evaluate_rust_analyzer_download is never called from composition, plugin register_lsp_adapter is a WIT fixture, and the desktop path only PATH-discovers rust-analyzer.
- Next: Wire LanguageServerAdapterRegistry and a live checksummed install path into AppComposition/desktop (not PATH-only rust-analyzer), with a plugin/SDK contribution that can register a non-Rust server end-to-end.

### P1-03 — Hero language matrix

- Required: Certified Rust, TS/JS, Python, Go and Java/C# workflows
- Level: 2 subsystem integration
- Status: in-progress
- Overstated: true
- Unit: crates/legion-app/src/language/{mod,download,session,app_lsp}.rs is rust-analyzer-only (hardcoded LanguageId rust, Cargo.toml gate). crates/legion-lsp/src/lib.rs LanguageServerAdapterRegistry::tier_two plus crates/legion-lsp/tests/registry_contract.rs stub rust, typescript-language-server, pyright (registry.example.invalid), and gopls — no java/csharp. crates/legion-index/src/lib.rs maps/highlights rust/python/ts/js/go (no .java/.cs grammars); crates/legion-project/src/lib.rs language_hint_for_path has java but not csharp; Cargo.toml has tree-sitter-go/python/typescript/javascript, not java/c#.
- Subsystem: Collaborating rust path only: crates/legion-app/tests/{rust_analyzer_session_handshake,rust_analyzer_doc_sync,rust_analyzer_read_requests,rust_analyzer_workflow,language_stale_snapshot,language_edit_proposal_routing,app_lsp_composition}.rs; xtask/src/main.rs rust-analyzer-smoke; plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md. No pyright/tsserver/gopls/jdtls/omnisharp integration tests.
- Reachability: legion-desktop DesktopRuntime dispatches through AppComposition (crates/legion-desktop/src/workflow.rs); CommandDispatchIntent::LspStartSession maps in crates/legion-app/src/intent_routing.rs and crates/legion-app/src/lib.rs try_start_lsp_session_for_current_workspace. Palette coverage in crates/legion-desktop/tests/palette_coverage.rs. Session still only rust-analyzer. crates/legion-app/src/lib.rs t1_open_rs_file_does_not_trigger_lsp_start contradicts docs/USER_GUIDE.md lazy-on-first-.rs claim. rust_analyzer_product_composition_smoke is AppComposition #[ignore], not a DesktopEframeApp live-server path.
- Rendered: crates/legion-desktop/tests/problems_panel_rendering.rs drives DesktopEframeApp clicks on injected diagnostics; crates/legion-desktop/tests/completion_popup.rs injects completions without a server. plans/evidence/production/WS-LANG-01/problems-panel-rendered-evidence.md keeps PR-LANG-001 at Substrate validated. No GUI evidence for TS/JS, Python, Go, or Java/C#.
- Installed: None. plans/product-readiness-ledger.md PR-REL-001 is unsigned-beta zip/tar.gz only. .github/workflows/legion-gates.yml rust-analyzer-smoke is a 3-OS CI step when rustup provision succeeds, not a signed packaged app completing language workflows on clean Windows/macOS/Linux.
- Remaining: Certified TS/JS, Python, Go, and Java/C# workflows are missing: product launch is rust-analyzer-only (Cargo.toml-gated), Java/C# have no adapters or grammars, and no renderer-backed or signed 3-OS matrix evidence exists.
- Next: Add real supervised sessions, smokes, and desktop start/read/write paths for typescript-language-server, pyright, gopls, and a Java/C# server, then capture GUI and installed 3-OS evidence beyond rust-analyzer.

### P1-04 — Debug platform

- Required: Real adapters, launch UX, advanced breakpoints and cross-OS fixtures
- Level: 3 application reachability
- Status: substrate
- Overstated: false
- Unit: crates/legion-debug/src/{live_session.rs,adapter_resolve.rs,dap.rs,framing.rs,bin/fake_dap_adapter.rs}; crates/legion-debug/tests/{dap_runtime.rs,live_dap_handshake.rs,adapter_resolution_policy.rs}; live set_breakpoints at crates/legion-debug/src/live_session.rs maps only {line}.
- Subsystem: crates/legion-app/src/debug_workflow.rs (launch_live, cargo configs, trust/capability, C4 spawn_sandboxed_stdio); crates/legion-app/tests/debug_workflow.rs; crates/legion-debug/tests/{system_adapter_dogfood.rs,system_adapter_launch_step_dogfood.rs}; plans/evidence/production/WS-A-D/phase-2-dap/B20-three-platform-dogfood.md; .github/workflows/legion-dap-dogfood.yml (independent 3-OS fail-closed dogfood, not a PR merge gate).
- Reachability: crates/legion-desktop/src/{main.rs,workflow.rs,bridge.rs,debug_auto_poll.rs,view.rs,view/keymap_dispatch.rs}: DesktopEframeApp/DesktopRuntime compose AppComposition; Launch/F5 enable the runtime without enable_debug_* seams (ensure_product_enabled); auto-poll in the product frame loop; crates/legion-desktop/tests/debug_reachability.rs (fixture path no runtime seam; live path still enable_debug_live_fake_for_tests).
- Rendered: Headless eframe only: crates/legion-desktop/tests/{debug_reachability.rs,debug_keyboard.rs,debug_workflow.rs,breakpoint_hit.rs,live_continue_auto_poll.rs} click Launch/Continue/Stop and F5/F9/F10; F9 always condition=None (keymap_dispatch.rs); no gutter; plans/evidence/dogfood/INTERACTIVE-GUI-CHECKLIST.md still requires a human windowed journal.
- Installed: No signed packaged debug loop on clean 3-OS machines; PR-LANG-002 in plans/product-readiness-ledger.md remains Substrate validated; USER_GUIDE.md still lists debug as not productized; legion-dap-dogfood.yml and legion-preview.yml are independent/unsigned and do not prove an installed debugger.
- Remaining: Live DAP still sends line-only setBreakpoints (dropping condition/hit/log), F9 and the toolbar never collect advanced breakpoint fields, and there is no windowed or signed 3-OS GUI proof of a real adapter.
- Next: Send condition, hitCondition, and logMessage on the live wire, add a GUI editor (not just F9/None), then fill a windowed real-adapter journal on Windows, macOS, and Linux.

### P1-05 — Test platform

- Required: Generic test-controller API and coverage
- Level: 3 application reachability
- Status: substrate
- Overstated: true
- Unit: crates/legion-app/src/test_explorer.rs (cargo --list/--exact/--group, unit tests) and protocol newtypes TestControllerId/TestItemDescriptor/TestRunSummary in crates/legion-protocol/src/lib.rs (TestItemDescriptor used only in crates/legion-protocol/tests/dto_contracts.rs).
- Subsystem: crates/legion-app/tests/test_explorer_workflow.rs drives AppComposition Refresh/Run/RunGroup/Attach intents on a trusted cargo fixture, including untrusted deny and metadata-only evidence.
- Reachability: crates/legion-desktop/src/view.rs Tests panel emits DesktopAction::{Refresh,Run}TestExplorer*; crates/legion-desktop/src/bridge.rs maps those to CommandDispatchIntent; crates/legion-desktop/src/workflow.rs dispatch_intent calls AppComposition; crates/legion-ui/src/shell_commands.rs :test-refresh/:test-run.
- Rendered: Headless only: crates/legion-desktop/tests/projection_rendering.rs clicks Tests/Refresh tests; crates/legion-desktop/tests/layout_region_coverage.rs injects snapshot rows. Ledger plans/product-readiness-ledger.md still calls the explorer substrate-only with no windowed journal.
- Installed: No signed 3-OS packaged-app evidence for a test-controller or coverage workflow (PR-REL-001 remains unsigned-beta/portable archives).
- Remaining: There is still no pluggable TestController implementation or coverage API—only a cargo-test/LSP-runnable explorer—and PR-LANG-002 still treats coverage as out of scope without a windowed or 3-OS GUI packet.
- Next: Add a real TestController registry that can discover/run non-cargo suites and emit coverage, then prove it through the Tests panel outside headless fixtures.

### P1-06 — SCM

- Required: Complete Git workflows and hosting-provider extensions
- Level: 3 application reachability
- Status: substrate
- Overstated: true
- Unit: crates/legion-project/src/lib.rs (GitForge/GitHubForge/GitLabForge URL builders; collect_git_snapshot; stage/commit/push/fetch/pull; gix path still falls back to CLI in git_status_entries_gix/git_blame_lines_gix); crates/legion-project/tests/git_workflow.rs (git_pull_request_url_builds_github_and_gitlab_links; status/hunk/conflict/worktree); crates/legion-app/src/git_inspection.rs (GitWorker snapshot/mutate/remote, no in-module tests).
- Subsystem: crates/legion-app/tests/git_workflow.rs (stage/commit/conflict via AppComposition+GitWorker); crates/legion-app/tests/git_remote_policy_workflow.rs (policy-gated push/fetch/pull, denied_remote_does_not_start_a_git_worker_job); crates/legion-app/tests/git_nav_workflow.rs, commit_validation_workflow.rs, worktree_creation_workflow.rs, local_history_workflow.rs; crates/legion-app/src/bin/golden_path_1.rs s6 edit-stage-commit (headless, no forge PR).
- Reachability: crates/legion-desktop/src/view.rs (Source Control rail dispatches DesktopAction::RefreshGit); crates/legion-desktop/src/view/source_control.rs (Refresh/Fetch/Pull/Push/Open PR/Commit…/hunk+path stage/conflict); crates/legion-desktop/src/bridge.rs OpenGitPullRequestUrl → git_pull_request_url → OpenExternalUrl; crates/legion-desktop/src/workflow.rs open_url_in_system_browser; crates/legion-app/src/lib.rs git_worker enqueue/drain; crates/legion-desktop/tests/git_workflow.rs (DesktopRuntime handle_action + URL translate, not a browser/API PR).
- Rendered: crates/legion-desktop/tests/source_control_reachability.rs proves rail click, status rows, stage/unstage/commit through DesktopEframeApp.run_headless_full_frame and then asks git; the file states it was never exercised in a windowed session. plans/evidence/dogfood/INTERACTIVE-GUI-CHECKLIST.md row 8 is still an empty operator cell.
- Installed: No signed 3-OS packaged run of SCM. PR-REL-001 remains In progress (unsigned-beta zip/tar.gz only). legion-smoke.yml GP-1 is independent and not a merge blocker; GP-4 has no create-PR step.
- Remaining: Hosting-provider extensions are github.com/gitlab.com compare/MR URL builders with no GitHub/GitLab API adapters, no self-hosted hosts, and no windowed or signed 3-OS SCM session.
- Next: Add policy-gated GitHub/GitLab (and self-hosted) API adapters behind GitForge so Open PR creates a real PR, then record a windowed GUI dogfood of stage/commit/push/open-PR.

### P1-07 — AI setup

- Required: Real provider onboarding, local-model management and model evaluation
- Level: 3 application reachability
- Status: substrate
- Overstated: true
- Unit: crates/legion-ai-providers/src/lib.rs (make_provider_registry, can_activate_provider, provider_setup_rows); crates/legion-ai-providers/tests/provider_activation.rs; crates/legion-app/src/local_ai_diagnosis.rs (Ollama/llama.cpp/Anthropic fallback copy); crates/legion-ai/src/lib.rs (ChatCompletionRequest, governors seam).
- Subsystem: crates/legion-app/src/product_ai_completion.rs and crates/legion-app/src/lib.rs product_ai_selection (Auto = Ollama then llama.cpp, never Anthropic); crates/legion-app/src/ai_route_descriptor.rs; crates/legion-ai-providers/tests/local_provider_tool_calling.rs and provider_smoke.rs (live Ollama skip if unpulled); evals/legion-bench/ plus xtask hostile-evals/verify-hostile-evals and HostileEvalLive skip-if-no-server.
- Reachability: crates/legion-desktop default feature ai; crates/legion-desktop/src/workflow.rs DesktopAction::SetPreferredAiProvider / SetProviderApiKey / DeleteProviderApiKey via OsKeyringSecretStore and AppComposition; crates/legion-desktop/src/view/interactive_fields.rs picker+BYOK form; crates/legion-desktop/src/view.rs SettingsSection::AiProviders and Assist “AI provider settings”.
- Rendered: Headless only: crates/legion-desktop/tests/projection_rendering.rs clicks Settings→AI Providers; crates/legion-desktop/tests/byok_field_isolation.rs (comments “Never exercised in a windowed session”); first-run render_setup_panel is a four-item checklist whose Review Settings opens Privacy, not AI; assist_delegate_reachability.rs proves Assist Predict without any provider setup.
- Installed: No signed 3-OS installer or clean-machine “Enable Local AI” wizard; PR-REL-001 remains In progress (unsigned-beta archives). Roadmap Phase 4 sidecar/catalog/downloader is not in crates/legion-ai-providers.
- Remaining: Settings can select an already-running Ollama/llama.cpp server or Anthropic BYOK, but there is no in-app model catalog, downloader, or sidecar, first-run does not complete provider setup, Assist still falls back to deterministic-local without an external server, and live-model evals remain skip/deferred.
- Next: Add a GUI Enable Local AI path that installs or verifies a managed local runtime, retire the fixture default with windowed desktop evidence, and record live hostile/bench evals instead of recorded-only skips.

### P1-08 — Diagnostics

- Required: Native fault capture, symbols, safe mode, support bundles
- Level: 1 unit
- Status: in-progress
- Overstated: true
- Unit: crates/legion-observability/src/crash_capture.rs (consent-gated std::panic::set_hook, panic.txt/summary.toml/audit.json); crates/legion-observability/src/export.rs; crates/legion-observability/src/minidump.rs (always sets minidump_captured=true with no dump file); crates/legion-app/src/first_run.rs; crates/legion-app/src/diagnostics.rs unit tests.
- Subsystem: crates/legion-observability/tests/crash_capture_tests.rs (catch_unwind bundles + double-opt-in export); crates/legion-app/src/diagnostics.rs SupportBundleAssembler wrapping DiagnosticsExportBuilder — never called from AppComposition.
- Reachability: install_panic_hook and SupportBundleAssembler have no callers in crates/legion-desktop or AppComposition; DesktopRuntime::persist_diagnostics_if_configured in crates/legion-desktop/src/workflow.rs writes metadata markdown only when --diagnostics-export is set; DesktopAction::SetCrashReportsEnabled reaches AppComposition::set_crash_reports_enabled but does not install a hook.
- Rendered: crates/legion-desktop/src/view.rs Privacy checkbox “Crash reports” is a consent toggle only; no GUI export-support-bundle or safe-mode command; crates/legion-desktop/tests/diagnostics_export.rs is headless DesktopRuntime, not a painted support-bundle workflow.
- Installed: plans/product-readiness-ledger.md PR-REL-001 and plans/evidence/production/M12/PKT-CRASH-evidence.md D7: native minidump/crash-handler deferred; no signed 3-OS installer evidence for fault capture, symbols, or safe mode.
- Remaining: Native out-of-process fault capture, symbolication, safe mode, and a composed support-bundle path are still missing: the panic hook is unwired, minidump.rs only emits metadata, and SupportBundleAssembler is unused by AppComposition.
- Next: Install a consent-gated out-of-process minidump path from legion-desktop startup, add a GUI/safe-mode support-bundle export, and stop marking P8.F3.T2 done until those exist.

### P2-01 — VSIX management

- Required: Registry, package lifecycle and extension security
- Level: 3 application reachability
- Status: in-progress
- Overstated: true
- Unit: crates/legion-vscode-compat/src/lib.rs and tests/compat_report.rs parse Open VSX JSON and require https:// VSIX URLs without fetching or unpacking; crates/legion-plugin/src/registry.rs plus tests/tampered.rs implement Ed25519 verify/install/update/remove and fail-closed tamper refusals; crates/legion-plugin/src/manifest.rs itemises per-capability review.
- Subsystem: crates/legion-app/src/extension_management.rs ExtensionCatalog offers legion.bundled.json-grammar, grants one capability at a time, and installs only after SignedExtensionRegistry::verify; app unit tests refuse unsigned/tampered/undecided entries.
- Reachability: AppComposition owns extension_catalog and routes CommandDispatchIntent::{SetExtensionPermission,InstallExtension,UpdateExtension,RemoveExtension} in crates/legion-app/src/intent_routing.rs and lib.rs; legion-desktop DesktopCommandBridge and DesktopRuntime handle those actions (crates/legion-desktop/src/bridge.rs, workflow.rs); crates/legion-desktop/tests/extensions_panel.rs drives the real composition. legion-vscode-compat has zero fan-in (CODEBASE.md; crate Cargo.toml).
- Rendered: crates/legion-desktop/src/view.rs paints SettingsSection::Extensions via render_extensions_panel, but the only workflow proof is headless DesktopRuntime/view-model tests, not a rendered GUI session.
- Installed: No signed packaged 3-OS evidence that a clean machine can browse a registry or install a VSIX; PR-REL-001 remains unsigned-beta portable archives only.
- Remaining: Open VSX/VSIX packages are still metadata-only and unwired, the shipping catalog is an in-memory first-party grammar signed with a committed development seed, and install does not persist, unpack a VSIX, or load the wasm host.
- Next: Replace the in-memory bundled-only catalog with a persistent signed registry (and optional ADR-0047 Tier-0 Open VSX ingest) that loads artifacts, then prove Settings > Extensions install/remove in a renderer-backed session.

### P2-02 — Node extension host

- Required: Versioned, isolated local and remote runtime
- Level: 0 unverified / absent
- Status: deferred
- Overstated: false
- Unit: crates/legion-vscode-compat/src/lib.rs and tests/compat_report.rs only label VsCodeExtensionHostRuntime::NodeSidecar and assert no Node process; crates/legion-protocol/src/lib.rs DTOs and crates/legion-observability/src/minidump.rs crash-report fixtures do the same. crates/legion-plugin/src/host.rs plus tests/hostile.rs, tests/quotas.rs, tests/wit_abi.rs implement an isolated Wasmtime guest (PHASE5_PLUGIN_ABI_VERSION=1), not Node.
- Subsystem: crates/legion-vscode-compat/Cargo.toml depends only on legion-protocol and is not a legion-app/legion-desktop dependency; xtask/deferred-surfaces.toml and plans/product-readiness-ledger.md (PR-VSC-002) keep runtime execution deferred. SignedExtensionRegistry in crates/legion-plugin/src/registry.rs verifies bytes and never instantiates them.
- Reachability: crates/legion-app/src/lib.rs composes PluginRuntimeHost (metadata-only host calls / load_plugin_manifest) and never WasmPluginHost; plans/adrs/ADR-0050-wasmtime-runtime-ratification.md states the engine is not reachable from a product binary. No legion-remote extension-host path.
- Rendered: crates/legion-desktop/src/view/extensions_panel.rs and tests/extensions_panel.rs drive signed WASM catalog install/permission review, not a Node host; plans/evidence/production/M5/WS15-T2-launch-extension-set.md uses a VSIX metadata fixture with no sidecar.
- Installed: No packaged 3-OS Node or remote extension-host; PR-VSC-002 evidence dir is missing and plans/adrs/ADR-0052-isolated-extension-host.md is the deferred-surface pointer to a file that does not exist.
- Remaining: There is still no versioned Node sidecar, vscode API/RPC bridge, or remote extension-host process; WASM isolation in legion-plugin is a different local engine and is not composed as WasmPluginHost.
- Next: Keep PR-VSC-002 deferred, or write ADR-0052 plus sandboxed local/remote host evidence instead of treating NodeSidecar DTO labels as a runtime.

### P2-03 — Web extension host

- Required: Worker-based extension runtime
- Level: 0 unverified / absent
- Status: deferred
- Overstated: false
- Unit: crates/legion-vscode-compat/src/lib.rs extension_host_session_for_manifest maps Tier2+Web to WebWorkerSidecar (passive descriptor; crate docs forbid execution); tests in crates/legion-vscode-compat/tests/compat_report.rs never assert WebWorkerSidecar; crates/legion-protocol/src/lib.rs documents WebWorkerSidecar as a sidecar that would be required.
- Subsystem: crates/legion-plugin/src/host.rs WasmPluginHost is an in-crate wasmtime fixture host (hostile/quotas/tampered tests) and is not a VS Code web-worker runtime; AppComposition uses PluginRuntimeHost envelopes only (crates/legion-plugin/src/lib.rs, crates/legion-app/src/lib.rs).
- Reachability: No legion-app/legion-desktop dependency on legion-vscode-compat; crates/legion-desktop/tests/beta_acceptance_e2e.rs constructs VsCode DTOs in-test with runtime NoneRequired and load_plugin_manifest; xtask/deferred-surfaces.toml still names missing ADR-0052.
- Rendered: crates/legion-desktop/src/cut_lines.rs plugin_registered_status and PLUGIN_EXECUTION_UNAVAILABLE; extensions panel (crates/legion-desktop/src/view/extensions_panel.rs) is signed-catalog/WASM install UX, not a web extension host.
- Installed: plans/product-readiness-ledger.md PR-VSC-002 Deferred; no 3-OS packaged worker host.
- Remaining: No web-worker sidecar, vscode API facade, or RPC bridge exists; WebWorkerSidecar is only a planning DTO and PR-VSC-002 stays deferred without ADR-0052.
- Next: Write plans/adrs/ADR-0052-isolated-extension-host.md and implement a sandboxed worker that runs browser-entrypoint extensions with a versioned vscode API, then wire it through AppComposition and legion-desktop instead of DTO labels.

### P2-04 — Contribution API

- Required: Views, tasks, tests, debuggers, SCM, notebooks, webviews, custom editors
- Level: 1 unit
- Status: in-progress
- Overstated: true
- Unit: crates/legion-vscode-compat/src/lib.rs classify_contribution maps views|viewsContainers (Tier2), taskDefinitions/testing|tests/debuggers/scm|sourceControl (Tier1 SupportedWithPolicy), and webviews/notebooks/customEditors (Tier3 Deferred); unit tests cover commands+debuggers and webview deferral; crates/legion-vscode-compat/tests/compat_report.rs covers views; crates/legion-protocol/src/lib.rs VsCodeContributionKind lists all eight; crates/legion-plugin/wit/{grammars,themes,lsp}.wit is only those three registration imports.
- Subsystem: crates/legion-plugin/tests/wit_abi.rs proves guest→host registration only for grammars/themes/lsp; crates/legion-protocol/src/lib.rs PluginContribution has Command/Menu/Panel/grammar/LSP metadata and no View/Task/Test/Debugger/Scm/Notebook/Webview/CustomEditor variants; product usages of PluginContribution are Command and TreeSitterGrammar only.
- Reachability: No shipped Cargo.toml depends on legion-vscode-compat (workspace member only); legion-app/src/lib.rs load_plugin_manifest and legion-desktop/src/workflow.rs load_plugin_manifest do not call it; crates/legion-desktop/tests/beta_acceptance_e2e.rs builds VsCodeExtensionManifest by hand and maps it to a Command-only PluginManifest.
- Rendered: crates/legion-desktop/src/view.rs plugin_rows is projection-only command labels plus crates/legion-desktop/src/cut_lines.rs PLUGIN_EXECUTION_UNAVAILABLE; no GUI path registers or completes contributed views, tasks, tests, debuggers, SCM, notebooks, webviews, or custom editors.
- Installed: No signed 3-OS packaged app evidence for these contribution APIs; PR-VSC-002 remains Deferred with explicit cut line in plans/product-readiness-ledger.md and xtask/deferred-surfaces.toml.
- Remaining: Views, tasks, tests, debuggers, SCM, notebooks, webviews, and custom editors are only classified in legion-vscode-compat (Tier3 notebooks/webviews/customEditors stay Deferred); they are not PluginContribution/WIT host APIs and that crate is unwired from product binaries.
- Next: Add WIT/protocol contribution APIs and AppComposition/DesktopRuntime registration for views, taskDefinitions, testing, debuggers, and scm first, leaving notebooks/webviews/customEditors behind the existing PR-VSC-002 cut line.

### P2-05 — Remote development

- Required: SSH remote server and version negotiation
- Level: 1 unit
- Status: deferred
- Overstated: true
- Unit: crates/legion-remote/src/lib.rs plan_ssh_session (no spawn) and ssh_connection_plan_activates_remote_runtime; crates/legion-protocol/src/lib.rs validate_remote_transport_handshake + RemoteTransportSchemaCompatibility and dto_contracts.rs incompatible-handshake denial; crates/legion-remote-transport/src/lib.rs accept_handshake/selected_schema_version.
- Subsystem: crates/legion-app/tests/workspace_vfs_integration.rs connect_remote_workspace_session uses the SSH planner as a local harness; crates/legion-remote/tests/transport_reconnect_offline.rs binds RemoteSessionTransport handshake/resume without sockets; no collaborating test talks to an SSH server.
- Reachability: crates/legion-desktop/src/workflow.rs DesktopAppRequest::ConnectRemoteWorkspace -> AppComposition::connect_remote_workspace_session (hardcoded legion-remote-ssh-agent/1 in crates/legion-app/src/lib.rs); crates/legion-desktop/src/cut_lines.rs remote_fixture_session_active; RemoteSessionTransport is unused outside legion-remote tests.
- Rendered: crates/legion-desktop/src/view.rs remote_rows string projection; crates/legion-desktop/tests/remote_workspace_gui.rs headless ConnectRemoteWorkspace; crates/legion-ui PanelId::RemoteWorkspace is a dock id only — no SSH host/key UI.
- Installed: None: PR-ENT-001 in plans/product-readiness-ledger.md is Deferred; no signed 3-OS package runs SSH remote connect.
- Remaining: There is still no SSH remote-server process or peer schema/version negotiation; connect is an in-memory planner plus handshake DTO checks.
- Next: Ship a fail-closed SSH remote-agent server that negotiates schema/agent versions with a real peer, then wire it through AppComposition instead of plan_ssh_session fixtures.

### P2-06 — Dev Containers

- Required: Standards-based container workflow
- Level: 2 subsystem integration
- Status: deferred
- Overstated: false
- Unit: crates/legion-remote/src/lib.rs (`RemoteDevcontainerConfig`, `plan_devcontainer_session_from_json`, metadata-only `parse_devcontainer_config`); unit tests `devcontainer_connection_plan_parses_config_and_activates_runtime` and `devcontainer_connection_plan_fails_closed_without_image_or_dockerfile` in the same file. No Docker/bollard/devcontainer-cli usage in crates/legion-remote.
- Subsystem: crates/legion-app/src/lib.rs `AppComposition::connect_devcontainer_workspace_session_from_json` plus crates/legion-app/tests/workspace_vfs_integration.rs `workspace_vfs_integration_devcontainer_remote_session_uses_policy_planner` (JSON fixture → in-memory RemoteSessionRuntime). plans/evidence/production/M6/WS16-T2-remote-transport-activation.md names that test; plans/product-readiness-ledger.md PR-ENT-001 still Deferred.
- Reachability: crates/legion-desktop/src/workflow.rs `DesktopAppRequest::ConnectRemoteWorkspace` calls `connect_remote_workspace_session` (SSH planner `legion-remote-ssh-agent/1`), not `connect_devcontainer_workspace_session_from_json`; crates/legion-desktop/src/cut_lines.rs labels it a fixture (`PR-ENT-001 deferred`). No legion-ui/desktop action for Dev Containers.
- Rendered: No GUI Reopen-in-Container/discoverable workflow; crates/legion-desktop/tests/remote_workspace_gui.rs is a headless `edge:test` fixture (`Remote fixture session active`). docs/USER_GUIDE.md states remote workspace is not SSH/devcontainer product UX.
- Installed: No signed 3-OS packaged Dev Containers attach; PR-REL-001 remains unsigned-beta/portable archives only, and PR-ENT-001 is still the deferred cut line in plans/product-readiness-ledger.md and xtask/deferred-surfaces.toml.
- Remaining: Standards-based Dev Containers (read `.devcontainer/devcontainer.json`, start image/Dockerfile/features/mounts, attach FS/LSP/PTY) are still unimplemented; later local commits did not close it.
- Next: Wire a policy-gated Reopen in Container path that starts a real container runtime from workspace `devcontainer.json` and attach it through `legion-desktop`, instead of the SSH/fixture connect.

### P2-07 — Remote extension placement

- Required: Local/UI versus workspace/remote execution
- Level: 1 unit
- Status: deferred
- Overstated: false
- Unit: crates/legion-protocol/src/lib.rs (VsCodeExtensionKind::{Ui,Workspace,Web}); crates/legion-vscode-compat/src/lib.rs (extension_kind maps "ui"/"workspace"/"web"; host session only special-cases Web vs Node sidecar); crates/legion-protocol/tests/dto_contracts.rs (Workspace kind roundtrip).
- Subsystem: crates/legion-plugin is local-only (WasmPluginHost tests in crates/legion-plugin/tests/{hostile,quotas,tampered}.rs; AppComposition uses PluginRuntimeHost envelopes, not WasmPluginHost); crates/legion-remote has no plugin/extension payloads and no legion-plugin dependency (crates/legion-remote/Cargo.toml).
- Reachability: crates/legion-desktop does not depend on legion-vscode-compat; DesktopRuntime::load_plugin_manifest (crates/legion-desktop/src/workflow.rs) is local PluginRuntimeHost; crates/legion-desktop/src/cut_lines.rs states WASM execution is unavailable; crates/legion-desktop/tests/beta_acceptance_e2e.rs plugin_manifest_from_vsix drops extension_kinds.
- Rendered: crates/legion-desktop/src/view/extensions_panel.rs is signed-install UI with no UI/workspace placement control; crates/legion-desktop/tests/remote_workspace_gui.rs is a fixture remote session without extension placement.
- Installed: No 3-OS signed-installer evidence for this outcome; ledger PR-VSC-002 and PR-ENT-001 remain Deferred with explicit cut line in plans/product-readiness-ledger.md.
- Remaining: Ui versus workspace kinds are classified, but no path runs UI extensions locally and workspace extensions on a remote host; PR-VSC-002 and PR-ENT-001 stay deferred.
- Next: Add a fail-closed placement planner with tests mapping VsCodeExtensionKind::Ui to local UI and Workspace to remote, refusing execution until both hosts exist.

### P3-01 — Durable agent runtime

- Required: Independent persistent legion-agentd
- Level: 0 unverified / absent
- Status: not-started
- Overstated: false
- Unit: crates/legion-agent/Cargo.toml has no [[bin]]; workspace-wide grep finds zero `legion-agentd`. crates/legion-agent/src/state.rs AgentRuntime and crates/legion-agent/src/agent_loop.rs run_delegated_task_loop (tests in crates/legion-agent/tests/agent_loop_integration.rs) are in-process library units. crates/legion-protocol/src/delegate_loop.rs is budget/audit DTOs only.
- Subsystem: ADR-0031 places coordination in legion-agent with AppComposition owning lifecycle; ADR-0043 explicitly rejects a separate long-lived ACP/daemon authority. crates/legion-app/src/lib.rs start_delegated_task_background comments a process-lifetime owner and std::thread worker. crates/legion-storage/src/lib.rs SaveAgentReplayManifest is metadata replay, not a live independent runtime.
- Reachability: crates/legion-desktop/src/bridge.rs and crates/legion-app/src/intent_routing.rs map StartDelegatedTask into AppComposition, which calls the in-process loop (crates/legion-app/src/lib.rs). crates/legion-desktop/src/workflow.rs reap_orphaned_delegated_task_sandboxes_at_startup assumes prior runs do not survive desktop restart. No desktop path talks to an agentd process.
- Rendered: crates/legion-desktop/src/view.rs and crates/legion-desktop/tests/sandbox_reachability.rs can start a Delegate task in the GUI; that workflow is still the in-process worker, not an independent persistent daemon a user can keep after quitting the IDE.
- Installed: No legion-agentd crate, packaging script, or installer layout; preview/release artifacts do not ship a signed 3-OS agent daemon.
- Remaining: No independent persistent legion-agentd exists: agent work is an in-process library plus an AppComposition process-lifetime worker thread that dies with the desktop (orphaned sandboxes are reaped on startup).
- Next: If the product still requires legion-agentd, write an ADR (ADR-0043 currently keeps ACP as an app-owned local adapter, not a daemon) and add a supervised out-of-process binary with durable resume independent of legion-desktop.

### P3-02 — Multi-agent orchestration

- Required: Parallel sessions, subagents, budgets and task DAG
- Level: 2 subsystem integration
- Status: substrate
- Overstated: true
- Unit: crates/legion-agent/src/{dag.rs,scheduler.rs,budget.rs,coordinator.rs,lib.rs}; crates/legion-agent/tests/{dag.rs,scheduler.rs,coordinator.rs} (three_task_dag_keeps_independent_workers_in_the_first_parallel_lane); crates/legion-protocol/src/lib.rs DelegatedTaskLineage.
- Subsystem: crates/legion-app/src/lib.rs execute_legion_workflow_internal + spawn_legion_workflow_worker_run + ensure_no_active_worker; crates/legion-app/tests/legion_workflow_integration.rs legion_workflow_parallel_lane_executes_lane_mates_concurrently_and_delays_dependents; crates/legion-app/tests/legion_workflow_plan_lifecycle.rs; crates/legion-app/src/bin/golden_path_4.rs; xtask/src/golden_path_4.rs; plans/evidence/production/M11/{PKT-WORKERS,PKT-LANES,PKT-PLAN,PKT-GP4}-evidence.md.
- Reachability: crates/legion-desktop/src/workflow.rs approve/revise/reject plans and stubbed kill/conflict/verify AppRequests; crates/legion-desktop/src/bridge.rs TriggerLegionWorkflowKillSwitch -> CommandDispatchIntent; crates/legion-app/src/intent_routing.rs; seed_legion_workflow_sessions is tests/harness-only; no desktop call to create_legion_workflow_session_from_plan or execute_legion_workflow.
- Rendered: crates/legion-desktop/src/view.rs render_fleet_canvas/render_fleet_console empty state 'Start a workflow to see its progress here.' with no start action; crates/legion-desktop/tests/legion_workflow_command_center.rs and projection_rendering.rs are headless egui/AccessKit clicks on synthetic snapshots; GP-4 is not GUI.
- Installed: No signed 3-OS packaged Automate path; .github/workflows/legion-smoke.yml GP-4 is independent/non-blocking; PR-REL-001 remains unsigned-beta portable archives.
- Remaining: Automate still cannot create or execute a multi-session DAG from the desktop, parent/child subagent budget inheritance is an unused protocol field, and GP-4 remains a headless AppComposition harness rather than a 3-OS signed GUI workflow.
- Next: Wire legion-desktop Automate to AppComposition plan/session/execute (not seed_legion_workflow_sessions), add parent-child budget lineage tests, and replace GP-4 with a user-completable GUI path before claiming product-workflow.

### P3-03 — Strong isolation

- Required: Windows VM/AppContainer tier, stronger Linux/macOS isolation
- Level: 2 subsystem integration
- Status: in-progress
- Overstated: true
- Unit: crates/legion-sandbox/src/spawn.rs (Linux Landlock write + optional bwrap --unshare-net bind-mounting /, macOS sandbox-exec SBPL, Windows job-object-kill-on-close with filesystem_write_enforced=false); crates/legion-sandbox/src/windows.rs (compile-time RestrictedToken notes, never CreateAppContainerProfile); crates/legion-sandbox/src/lib.rs os_read_enforcement BrokerOnly for every backend including AppContainer; crates/legion-sandbox/tests/compile_profiles.rs; crates/legion-sandbox/tests/escape_attempts.rs Windows tests assert outside-root WRITE_OK.
- Subsystem: crates/legion-app/src/lib.rs AppDelegatedToolHost probes spawn_sandboxed then require_delegated_terminal_enforcement (write+read+network), which no backend can satisfy because filesystem_read_enforced is always false; crates/legion-agent/src/worktree.rs plus tests/worktree_sandbox.rs and tests/containment_canonicalization.rs for disposable-worktree/app-layer containment; crates/legion-security is capability/policy only, not OS isolation; plans/evidence/production/WS-A-D/phase-3-sandbox/C2-windows-fs-residual.md and docs/SECURITY.md document the Windows residual.
- Reachability: AppComposition start_delegated_task constructs AppDelegatedToolHost (crates/legion-app/src/lib.rs ~21524); legion-desktop view.rs emits sandbox_rows only in Delegate mode; golden_path_3.rs s5 treats Blocked as acceptable sandbox teeth; spawn never selects SandboxBackend::AppContainer.
- Rendered: crates/legion-desktop/tests/sandbox_reachability.rs drives headless DesktopEframeApp AccessKit text for the honesty panel (Windows job-object limits visible); crates/legion-desktop/src/view/sandbox_panel.rs still labels AppContainer os-enforced. Headless fixture/a11y tree is not a user-completed AppContainer/VM isolation workflow.
- Installed: No signed packaged 3-OS installer evidence that a clean Windows/macOS/Linux machine runs VM/AppContainer-tier isolation; PR-REL-001 remains unsigned-beta; PR-ENT-001 keeps the ADR-0038 devcontainer strong tier deferred.
- Remaining: Windows spawn is still job-object-only with no AppContainer, restricted-token, or VM filesystem/network isolation, and no backend reports OS-level read confinement or Linux selective egress, so the required strong-isolation outcome is not met.
- Next: Ship a real Windows AppContainer or VM spawn path that sets filesystem_write_enforced and network_enforced, add Landlock/Seatbelt read confinement plus Linux allowlisted egress, and make require_delegated_terminal_enforcement pass on all three OSes.

### P3-04 — Autonomous PR workflow

- Required: Isolated worktrees, evidence, review, branch and pull request
- Level: 2 subsystem integration
- Status: substrate
- Overstated: false
- Unit: crates/legion-agent/src/worktree.rs (git worktree add + directory-copy sandbox); crates/legion-agent/src/evidence.rs and merge_readiness.rs; crates/legion-agent/tests/worktree_sandbox.rs; crates/legion-project/src/lib.rs GitForge::pull_request_url (GitHub compare / GitLab new-MR URL only, no create API); crates/legion-app/src/delegate_workflow.rs (proposal hunk review, not git PR).
- Subsystem: crates/legion-app/src/lib.rs StartDelegatedTask + DelegatedTaskSandboxOrchestrator and execute_legion_workflow (tests + crates/legion-app/src/bin/golden_path_4.rs only); crates/legion-app/tests/legion_workflow_integration.rs, worktree_creation_workflow.rs, worktree_evidence_workflow.rs; crates/legion-agent/tests/coordinator.rs; crates/legion-app/src/bin/golden_path_3.rs s7/s8 sandbox+review. GP-4 s11–s13 are merge-ready + evidence-bundle replay with no branch/PR. Ledger: plans/product-readiness-ledger.md PR-AI-002 Substrate (GP-4 command-center), PR-LANG-002 Substrate (git worktrees), PR-ENT-002 Deferred. ADR-0031 forbids autonomous merge.
- Reachability: legion-desktop does not call execute_legion_workflow. DesktopAction::StartDelegatedTask (crates/legion-desktop/src/bridge.rs) and CreateGitBranch reach AppComposition; DesktopAction::OpenGitPullRequestUrl only opens a compare URL via OpenExternalUrl (crates/legion-desktop/src/workflow.rs). No api.github.com/GitLab PR create path in-tree.
- Rendered: crates/legion-desktop/src/view/source_control.rs renders an Open PR button; Git: New Worktree / Export Worktree Evidence are palette specs in crates/legion-app/src/lib.rs. docs/USER_GUIDE.md caveats this as gated test-exercised surfaces, not a completable autonomous PR GUI.
- Installed: No signed 3-OS packaged path creates a pull request; PR-REL-001 remains in-progress unsigned-beta.
- Remaining: A merge-ready Automate session still never creates a git branch or a forge pull request: GitForge only builds a compare URL, GP-4 stops at evidence-bundle replay, and execute_legion_workflow is not invoked from legion-desktop.
- Next: Add a policy-gated branch/push/create-PR (or real open-PR) terminus on AppComposition for merge-ready workflow sessions and drive it from DesktopRuntime, with a test repo proving GP-4 ends in an actual PR.

### P3-05 — Current ACP/MCP

- Required: Conformance to present protocol versions
- Level: 2 subsystem integration
- Status: in-progress
- Overstated: true
- Unit: crates/legion-ai-providers/src/lib.rs (McpClient tools/list|resources/list|prompts/list|tools/call, StdioMcpTransport, StreamableHttpMcpTransport POST without MCP-Protocol-Version); crates/legion-ai-providers/src/mcp_server.rs initialize advertises protocolVersion 2025-11-25 only; crates/legion-protocol/src/lib.rs McpJsonRpcEnvelope adds non-MCP schema_version; crates/legion-app/src/lib.rs AcpHostCommand sets LEGION_ACP_* and Command::output with no ACP methods.
- Subsystem: crates/legion-ai-providers/tests/mcp_ga_conformance.rs and src/bin/mcp_stdio_fixture.rs echo list/call against Legion fixtures (no initialize/version negotiation); crates/legion-app/tests/legion_workflow_integration.rs AppMcpClientToolRuntime; crates/legion-app/tests/delegated_task_integration.rs ACP spawn→proposal; plans/evidence/production/M3/WS09-T6-mcp-client-ga.md froze 2025-11-25/rmcp keep.
- Reachability: legion-desktop DesktopWorkflow::dispatch_intent forwards CommandDispatchIntent into AppComposition; acp-attach-host is registered in crates/legion-app/src/lib.rs and routed in intent_routing.rs, but crates/legion-desktop has no ACP protocol code; register_legion_workflow_mcp_tool_runtime is test-injected (empty HashMap on default AppComposition).
- Rendered: docs/USER_GUIDE.md documents palette ACP: Attach Host as a local adapter, not an ACP workbench; crates/legion-desktop/src/view.rs render_legion_workflow_tool_permission_controls and legion_workflow_command_center.rs paint MCP permission/registry rows from projections only.
- Installed: No signed 3-OS packaged run of MCP 2026-07-28 or ACP v1 against a real server/agent.
- Remaining: The tree still speaks a 2025-11-25-shaped MCP subset (no client initialize, no MCP-Protocol-Version/_meta version, extra schema_version on envelopes) plus an env-var subprocess ACP bridge, not MCP 2026-07-28 or ACP v1 JSON-RPC (initialize/session/new/session/prompt).
- Next: Add dual-version MCP (2025-11-25 handshake plus 2026-07-28 _meta protocolVersion) and a real ACP v1 host, then fail CI unless those speak a live or official-schema peer rather than in-tree echo fixtures.

### P3-06 — Enterprise administration

- Required: SSO, SCIM, RBAC, fleet policy and audit export
- Level: 1 unit
- Status: deferred
- Overstated: false
- Unit: Signed org policy bundles and default-deny `audit_export_enabled` live in `crates/legion-security/src/policy.rs` and `crates/legion-security/src/lib.rs` with tests in `crates/legion-security/tests/signed_policy_bundle.rs`, `crates/legion-security/tests/policy_bundle_surfaces.rs`, and `crates/legion-security/tests/org_policy_bundle.rs`; collaboration “RBAC” is only `CollaborationParticipantRole` in `crates/legion-protocol/src/lib.rs`; no SSO/SCIM code exists.
- Subsystem: Org-policy enforcement below the shell is proven by `crates/legion-app/tests/org_policy_mode_ceiling.rs`, `AppComposition::set_org_policy_bundle` in `crates/legion-app/src/lib.rs`, `crates/legion-agent/tests/mcp_tool_allowlist_bridge.rs`, `crates/legion-retention/src/lib.rs`, and `plans/evidence/production/P9-F2-T3-signed-policy-bundles.md` — that is not SSO, SCIM, enterprise RBAC, or audit export.
- Reachability: `crates/legion-desktop` never calls `set_org_policy_bundle` or loads `xtask/legion-policy.example.toml`; `render_fleet_console` in `crates/legion-desktop/src/view.rs` is the Automate workflow command center (`plans/evidence/production/M11/PKT-CONSOLE-evidence.md`), not fleet-policy administration.
- Rendered: No GUI discovers SSO, SCIM, RBAC, policy-bundle install, or audit export; `docs/USER_GUIDE.md` lists collaboration GUI as not productized and `plans/product-readiness-ledger.md` keeps PR-ENT-002 Deferred.
- Installed: No signed 3-OS packaged path completes enterprise administration; PR-REL-001 remains In progress with unsigned-beta archives only (`plans/product-readiness-ledger.md`).
- Remaining: SSO, SCIM, enterprise RBAC, desktop fleet-policy installation, and a real audit-export workflow are still missing, and PR-ENT-002 remains deferred.
- Next: Keep PR-ENT-002 deferred until SSO/SCIM/RBAC plus a desktop-reachable signed policy install and metadata-only audit export exist with product evidence.

### P3-07 — Production collaboration

- Required: Authenticated durable collaboration service
- Level: 2 subsystem integration
- Status: deferred
- Overstated: true
- Unit: crates/legion-collaboration/src/lib.rs (CollaborationSessionRuntime OT/replay, default CollaborationRuntimeConfig.runtime_enabled=false, in-crate tests); plans/adrs/ADR-0020-collaboration-operation-model.md; plans/adrs/ADR-0045-collaboration-operation-layer.md
- Subsystem: crates/legion-app/src/lib.rs CollaborationComposition + join_collaboration_session/receive_collaboration_transport_envelope/persist_collaboration_audit; crates/legion-app/tests/workspace_vfs_integration.rs workspace_vfs_integration_collaboration_presence_is_app_owned_projection; crates/legion-security/src/lib.rs CollaborationCapabilityPolicy defaults (runtime_sessions_enabled=false); crates/legion-storage/src/lib.rs SaveCollaborationAuditRecord HashMap
- Reachability: crates/legion-desktop/src/bridge.rs DesktopAction::JoinCollaborationSession; crates/legion-desktop/src/workflow.rs enable_local_collaboration_runtime (doc: test/launch harnesses only; callers are tests); crates/legion-ui/src/shell_commands.rs :collab-join; no product launch enables the runtime
- Rendered: crates/legion-desktop/src/view.rs collaboration_rows is view-model strings never passed to render_compact_rows; crates/legion-desktop/tests/collaboration_gui.rs is headless; docs/USER_GUIDE.md and crates/legion-desktop/src/health.rs still list Collaboration GUI unsupported; plans/evidence/gui-productization/phase-8-collaboration-gui.md claims supported
- Installed: No signed 3-OS package path for collaboration; plans/product-readiness-ledger.md PR-ENT-002 Deferred; xtask/deferred-surfaces.toml gate=PR-ENT-002
- Remaining: There is no authenticated durable multi-user collaboration service: sessions and OT live in-process, default-off, with metadata-only audit maps rather than persisted op-logs, identity, or a network service.
- Next: Keep PR-ENT-002 deferred until an identity-backed durable collab service is composed through AppComposition, painted in legion-desktop, and evidenced beyond headless enable_local_collaboration_runtime tests.

## Method

Work-list is the ranked P0-P3 register from the 2026-08-31 audit. Agents re-read the current tree. Rendered/installed claims get an independent skeptic. Failed or empty verification cannot raise a row.
