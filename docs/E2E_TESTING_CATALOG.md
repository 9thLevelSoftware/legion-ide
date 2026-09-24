# Legion IDE — End-to-end testing catalog

**Date:** 2026-09-02  
**Audience:** automated E2E, dogfood, and gap scoring  
**Does not:** promote ledger rows, claim general availability, or treat a green cargo test as a daily-driver proof

This catalog is the product-behavior map for Legion as it exists in this tree and as it is **expected** to exist. Use it to score each case `PASS` / `FAIL` / `HARNESS-ONLY` / `NOT-BUILT` / `DEFERRED` rather than inferring the product from a sequence of PRs.

---

## 0. How to use this document

### 0.1 Status vocabulary (per case)

| Status | Meaning for automated testing |
| --- | --- |
| **CURRENT** | Implemented in the product composition and reachable from a live `legion-desktop` window *or* from a named desktop/app test that drives the same dispatch path. Still score it; “CURRENT” is not “works on your machine.” |
| **PARTIAL** | Code and tests exist, but a named residual, OS gap, or fixture provider means the user-facing path is incomplete. |
| **HARNESS-ONLY** | Proven by `AppComposition`, `golden-path-*`, kittest, `--beta-smoke`, or `--windowed-e2e`. Not proven as a human using the idle native window. |
| **EXPECTED-UNBUILT** | Product contract or design says this should exist; the tree does not ship a working UX path. Treat FAIL as the honest default until built. |
| **DEFERRED** | Explicit cut line (ADR, ledger, or `AGENTS.md`). Do not score as a product defect unless the deferral is lifted. |

### 0.2 Evidence ladder (do not collapse)

A case is only as strong as the level you actually ran:

1. Unit / crate test  
2. Subsystem / workflow test  
3. Desktop composition (kittest / `DesktopEframeApp` without a real OS window)  
4. **Windowed GUI** — `eframe::run_native`, not `--beta-smoke`, not AppComposition binaries  
5. Installed signed artifact on a clean OS  

Headless golden paths (GP-1–4) are level 3. `xtask windowed-gui-e2e` / `--windowed-e2e` is level 4 for **open / edit / save only**. Hosted 3-OS windowed GUI (`legion-windowed-gui.yml`) is level 4 for that same loop. Nothing in this repo is currently level 5 (signed clean-VM install).

### 0.3 Suggested verdict row

Copy this into an automation report:

```text
id: E2E-…
surface:
expected:
status_in_catalog: CURRENT | PARTIAL | HARNESS-ONLY | EXPECTED-UNBUILT | DEFERRED
level_run: 1-5
oracle:
result: PASS | FAIL | SKIP
notes:
```

### 0.4 Launch matrix (what you actually started)

| Launch | Command | Window? | What it is |
| --- | --- | --- | --- |
| Product window | `cargo run -p legion-desktop -- <workspace>` | Yes (`run_native`) | Idle native IDE. This is the cake. |
| Windowed E2E | `cargo run -p xtask -- windowed-gui-e2e` or `legion-desktop --windowed-e2e` | Yes, then exits | Automated open/insert/save. Not a session. |
| Renderer smoke | `legion-desktop --smoke --duration-ms N --evidence path` | Yes, timed | Native window + smoke assertions. |
| Beta smoke | `legion-desktop --beta-smoke` | No real product loop | Headless kittest-style harness. **Not** windowed GUI. |
| CLI proof | `cargo run -p legion-app -- <path>` | No | Trusted workspace + `:w` / `:q` only. Not the renderer. |
| GP-1 | `cargo run -p xtask -- golden-path-1` | No | AppComposition: open, LSP, diagnostics, search, terminal, git. |
| GP-2 | `cargo run -p xtask -- golden-path-2` | No | Assist: inline prediction, provider policy, context manifest, proposal apply/rollback. |
| GP-3 | `cargo run -p xtask -- golden-path-3` | No | Delegate: scope, sandbox, denial, kill-switch, evidence. |
| GP-4 | `cargo run -p xtask -- golden-path-4` | No | Legion Workflows: plan, workers, gates, kill-switch. |
| Update drill | `cargo run -p xtask -- update-drill` | No | Deterministic update/rollback with ephemeral key. |

**Oracle for “the app works”:** product window, not GP-*, not `--windowed-e2e`.

---

## 1. Product identity and standing contracts

These are expected even when a surface is incomplete. A FAIL here is a product-safety FAIL, not a missing feature.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-INV-01 | UI is projection-only: it emits `CommandDispatchIntent` / `DesktopAction` and never owns editor text or session dirty bodies. | CURRENT (architecture + tests). |
| E2E-INV-02 | Saves are proposal-mediated: `AppComposition::save_active_buffer` → `SaveWorkflowService` → `WorkspaceActor::save_file_with_proposal`. Stale/conflict/denial returns `Rejected` and keeps dirty editor text. | CURRENT. |
| E2E-INV-03 | Workspace save requires expected fingerprint, file content version, workspace generation, buffer version, snapshot id, and non-zero correlation/causality. Non-atomic write fallback is fail-closed. | CURRENT. |
| E2E-INV-04 | Manual is the default mode. Assist / Delegate / Legion Workflows require an explicit mode change. No AI feature re-enables itself. | CURRENT (mode policy). Live Assist still often uses `deterministic-local` unless a loopback provider is up. |
| E2E-INV-05 | Manual product features make no network calls (no phone-home, no telemetry, no provider). Git remotes the user invokes are user-initiated egress, not product telemetry. | CURRENT as policy; OS packet-capture (GAP-06.2) is EXPECTED-UNBUILT. |
| E2E-INV-06 | AI may propose; it must not write the workspace until the user approves a proposal. | CURRENT for save/proposal paths. |
| E2E-INV-07 | Observability defaults to metadata-only redaction. Session JSON must not persist dirty buffer bodies or raw secrets. | CURRENT. |
| E2E-INV-08 | Untrusted workspaces deny dangerous capabilities (terminal launch, debug adapter launch, etc.) through the capability broker. | CURRENT (policy). |
| E2E-INV-09 | Docs and UI stay honest about release status (`claim-audit` fails overstated readiness wording). | CURRENT (claim-audit). |
| E2E-INV-10 | Default `ai` desktop build is not a Manual/offline SKU. Signed installers are not shipped. | CURRENT: unsigned-beta. Manual SKU packaging was proposed then **rejected as premature**; do not require `--sku manual` as product E2E. |

---

## 2. Modes

Source of truth: `docs/MODES.md`. Mode switch pills: `M` Manual, `A` Assist, `D` Delegate, `W` Legion Workflows. Typed: `:mode manual|assist|delegate|workflow`.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-MODE-01 | Fresh window starts in Manual. AI panels, hosted providers, cloud lanes, and autonomous workers are absent. | CURRENT / score in the **product window**. |
| E2E-MODE-02 | Switching to Assist shows Assist surfaces (inline prediction, assistant rail) and still forbids direct AI file writes. | PARTIAL: substrate + GP-2. Default GUI Assist often uses fixture provider, not a live model. |
| E2E-MODE-03 | Switching to Delegate shows fleet/task/proposal surfaces; workers cannot mutate the main workspace directly. | HARNESS-ONLY (GP-3, command-center tests). |
| E2E-MODE-04 | Switching to Legion Workflows shows workflow/plan/gate surfaces; merge/apply still needs human authority. | HARNESS-ONLY (GP-4). |
| E2E-MODE-05 | Escalation (Manual → Assist/Delegate/Workflows) asks for confirmation; Escape cancels. | CURRENT (desktop keyboard tests). |
| E2E-MODE-06 | Returning to Manual hides AI/network/worker product surfaces. | CURRENT (filtering tests). |

---

## 3. Daily-driver editing (the cake)

**Expected product:** a person launches the native window on a trusted workspace, opens files from the explorer or palette, types, undoes, saves, restores after a kill, and quits — without `--smoke` or `--windowed-e2e`.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-EDIT-01 | `cargo run -p legion-desktop -- <workspace>` creates a native window titled like `Legion IDE` (or smoke title only when `--smoke`). | CURRENT (`run_native`). Score this first. |
| E2E-EDIT-02 | Opening a file shows its text in the editor canvas (not a headless rope). | PARTIAL: renderer canvas exists; many proofs are kittest/GP. Windowed E2E opens a fixture file. |
| E2E-EDIT-03 | Typing inserts at the caret through editor authority. Backspace/Delete/Enter mutate the buffer. | CURRENT (desktop input tests). Score in the live window. |
| E2E-EDIT-04 | Multi-cursor: Ctrl+Alt+↑ / Ctrl+Alt+↓ add carets; Esc collapses to one caret when extras exist. | CURRENT (keymap). |
| E2E-EDIT-05 | Select all, copy, cut, paste via OS clipboard. | PARTIAL: copy/cut wired; clipboard smoke tests exist. Score paste/IME in the live window. |
| E2E-EDIT-06 | IME composition does not corrupt the buffer. | PARTIAL (ime_smoke). |
| E2E-EDIT-07 | Undo / redo: `:u` / `:redo` and the product undo stack. | CURRENT in composition; score in window. |
| E2E-EDIT-08 | Save active: Ctrl+S / palette `Save Active Buffer` / `:w`. Proposal-mediated. Dirty flag clears only on accepted save. | CURRENT. Windowed E2E covers insert+save on a fixture. |
| E2E-EDIT-09 | Save all: Ctrl+Shift+S / `Save All` / `:wa`. Per-item reject keeps dirty text. | CURRENT (save_all_conflict tests). |
| E2E-EDIT-10 | External overwrite between open and save yields a conflict, not a silent clobber. | CURRENT (workspace_vfs_integration). |
| E2E-EDIT-11 | Close tab: Ctrl+W / `:close`. Unsaved tab prompts; cancel keeps the tab. | CURRENT (snapshots `unsaved-changes-prompt-*`). |
| E2E-EDIT-12 | Quit: `:q` / window close. Dirty session restores on relaunch from `.legion/unsaved/` sidecars, not from session JSON bodies. | CURRENT (session_restore tests + GAP-04). Score by killing the **product window**. |
| E2E-EDIT-13 | Large file (~100MB): editor stays typable; degraded mode banner; binary files refused. | PARTIAL: text-model and some renderer wiring; live 3-OS paint still the promotion gap. |
| E2E-EDIT-14 | Font family setting changes rendered glyphs. | EXPECTED-UNBUILT: `SetEditorFontFamily` is persisted but canvas hard-codes monospace (`intent-reachability.toml`). |
| E2E-EDIT-15 | Find in file: Ctrl+F toggles find bar; next/prev, case, word, regex, replace one/all. | CURRENT as intents (`ToggleFindBar`…). Score in the window. |
| E2E-EDIT-16 | Line wrap, line numbers, minimap, indent/whitespace guides, sticky headers, folding, smooth scroll, zoom, theme. | PARTIAL: settings + intents exist; score visual effect in the window (minimap/folding may be projection-only). |

### 3.1 Vim (opt-in)

Expected: `AppComposition::dispatch_vim_key` / `DesktopAction::VimKey` is a real product path. `CommandDispatchIntent::Vim*` mapping to `Noop` in `CommandDispatcher` is **not** the product path.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-VIM-01 | Enable Vim; hjkl / counts / operators / insert / put / search / `dd` work on the open buffer. | PARTIAL: parser + session path live and opt-in. Score in the window with Vim enabled. |
| E2E-VIM-02 | Undo/redo from Vim. | EXPECTED-UNBUILT in the Vim mapper (`VimAction::Undo/Redo` → `Noop`). |

---

## 4. Explorer, tabs, palette, files

| ID | Expected | Current |
| --- | --- | --- |
| E2E-NAV-01 | File tree lists the trusted workspace; click/Enter opens a file. | CURRENT (explorer snapshots + activation tests). Score depth, ignore rules, and large trees in the window. |
| E2E-NAV-02 | Refresh Explorer (F5 when not debugging) reloads the tree. | CURRENT (palette). |
| E2E-NAV-03 | Reveal Active File in Explorer (⇧⌘E / palette). | CURRENT (palette). |
| E2E-NAV-04 | Command palette modes: no prefix = files; `>` commands; `/` search; `#` structural search; `@` symbols; `^` recent buffers. Fuzzy + recency + frequency (metadata-only). | CURRENT (palette tests). |
| E2E-NAV-05 | Ctrl+P / Ctrl+Shift+P open file vs command palette as documented. | CURRENT (keyboard_nav). |
| E2E-NAV-06 | Opening a path that does not exist fails closed (no fabricated buffer). | CURRENT (composition). |
| E2E-NAV-07 | Tab reorder, switch, close. | CURRENT. |
| E2E-NAV-08 | Canvas activity-rail: pan/zoom, drag file cards, user-drawn edges persist across restart. | PARTIAL: arrangement surface built. Syntax-colored cards, derived import/call edges, minimap, groups: EXPECTED-UNBUILT (`docs/ui/canvas-workspace-direction.md`). |

### 4.1 Command palette — registered commands

Score each as: opens, asks for operands if needed, dispatches, visible result.

| ID | Palette title | Shortcut (projected) |
| --- | --- | --- |
| E2E-PAL-01 | Save All | Ctrl+Shift+S |
| E2E-PAL-02 | Save Active Buffer | ⌘S / Ctrl+S |
| E2E-PAL-03 | Close Active Tab | ⌘W |
| E2E-PAL-04 | Reveal Active File in Explorer | ⇧⌘E |
| E2E-PAL-05 | Refresh Explorer | F5 (idle, no debug configs) |
| E2E-PAL-06 | Refresh Git | — |
| E2E-PAL-07 | Git: Stage Focused Hunk | Ctrl+Shift+G |
| E2E-PAL-08 | Git: Switch Branch | operand |
| E2E-PAL-09 | Git: Create Branch | operand |
| E2E-PAL-10 | Git: Delete Branch | confirm |
| E2E-PAL-11 | Git: Stash Changes | optional message |
| E2E-PAL-12 | Git: Push | policy-gated remote |
| E2E-PAL-13 | Git: Fetch | policy-gated remote |
| E2E-PAL-14 | Git: Pull | policy-gated remote |
| E2E-PAL-15 | Git: Prune Worktrees | — |
| E2E-PAL-16 | Git: Remove Worktree | confirm |
| E2E-PAL-17 | Git: New Worktree | branch + path |
| E2E-PAL-18 | Git: Local History | then restore via proposal |
| E2E-PAL-19 | Git: Export Worktree Evidence | `.legion/evidence/` metadata TOML |
| E2E-PAL-20 | Git: Commit Staged Changes | message; validation |
| E2E-PAL-21 | ACP: Attach Host | program + args; opt-in |
| E2E-PAL-22 | Close Command Palette | Esc |
| E2E-PAL-23 | Preferences: Open Settings | — |
| E2E-PAL-24 | Help: About | version, proprietary, not GA |
| E2E-PAL-25 | Help: Export Support Bundle | metadata-only `.legion/support-bundle.md` |
| E2E-PAL-26 | Preferences: Theme Dark / Light / System | — |
| E2E-PAL-27 | Preferences: Reset Zoom | 100% |
| E2E-PAL-28 | Preferences: Reset Settings | confirm |
| E2E-PAL-29 | Language Server: Start | lazy start |
| E2E-PAL-30 | Language Server: Restart | resets circuit breaker |
| E2E-PAL-31 | Language: Format Document | Shift+Alt+F — **proposal preview**, not silent format |
| E2E-PAL-32 | Language: Rename Symbol | F2 — proposal preview |
| E2E-PAL-33 | Language: Organize Imports | Ctrl+Shift+O — proposal preview |
| E2E-PAL-34 | Language: Code Action | action id — proposal preview |

---

## 5. Typed shell (`:`)

The `:` line is a first-class product surface, not a debug leftover. Many capabilities are **only** here (no default keymap).

| ID | Command | Expected |
| --- | --- | --- |
| E2E-SH-01 | `:q` | Quit |
| E2E-SH-02 | `:w` / `:wa` | Save active / save all |
| E2E-SH-03 | `:u` / `:redo` | Undo / redo |
| E2E-SH-04 | `:mode …` | Set product mode |
| E2E-SH-05 | `:tab` / `:tab <id>` / `:close <id>` | Assist-accept alias / switch / close |
| E2E-SH-06 | `:search` / `:search-workspace` / `:search-cancel` | Find / workspace search / cancel |
| E2E-SH-07 | `:hover` `:completion` `:definition` `:references` `:outline` `:inlayhints` `:codelens` | LSP read-side |
| E2E-SH-08 | `:format` `:rename` `:organize-imports` `:code-action` `:language-cancel` | LSP write-side **proposals** |
| E2E-SH-09 | `:git-refresh` `:git-stage-hunk` `:git-unstage-hunk` `:git-nav-*` `:git-push/fetch/pull` `:git-*-branch` `:git-stash` `:git-*-worktree` `:git-local-history` `:git-restore-history` `:git-export-evidence` `:git-validate-commit` `:git-allow-remote` `:git-revoke-remote` `:git-accept-*-conflict` | Git |
| E2E-SH-10 | `:test-refresh` `:test-run` `:test-run-group` `:test-attach-evidence` | Test explorer (typed only) |
| E2E-SH-11 | `:debug-configs` `:debug-launch` `:debug-breakpoint` `:debug-step` `:debug-run-to-cursor` `:debug-eval` `:debug-watch` | Debug (typed + some keys) |
| E2E-SH-12 | `:term-launch` `:term-input` `:term-resize` `:term-kill` `:term-close` `:term-poll` `:term-search` | Terminal; launch is **not** a default keymap |
| E2E-SH-13 | `:assist-predict` `:assist-accept` `:assist-dismiss` `:assist-cancel` | Assist ghost |
| E2E-SH-14 | `:ai-start` `:ai-explain` `:ai-propose` `:ai-cancel` `:ai-replay` `:ai-inspect` | Assist/AI runs |
| E2E-SH-15 | `:delegate-chat` `:delegate-hunk` `:delegate-permission` | Delegate |
| E2E-SH-16 | `:proposal-*` | Preview/approve/reject/apply/rollback/cancel/details |
| E2E-SH-17 | `:legion-inspect` `:legion-proposal-*` `:legion-verify` `:legion-signoff` `:legion-resolve` `:legion-readiness` `:legion-permission` `:legion-kill` | Workflows |
| E2E-SH-18 | `:plugin` | Plugin command invoke |
| E2E-SH-19 | `:collab-join` `:collab-leave` `:collab-presence` | Collaboration **substrate**; product UX is DEFERRED |
| E2E-SH-20 | `:context-manifest-select` `:context-manifest-clear` | Assist context manifest |
| E2E-SH-21 | `:i` `:d` `:r` | Insert / delete / replace at coordinates |

---

## 6. Keyboard (default map)

Source: `docs/KEYBOARD_REFERENCE.md` plus GAP-05.1 certification.

**Certified in renderer tests (still score in the live window):**

| ID | Gesture | Action |
| --- | --- | --- |
| E2E-KEY-01 | Ctrl/Cmd+Shift+P | Command palette |
| E2E-KEY-02 | Ctrl/Cmd+P | File palette |
| E2E-KEY-03 | Ctrl/Cmd+Shift+F | Workspace search |
| E2E-KEY-04 | Ctrl/Cmd+F | Find in file |
| E2E-KEY-05 | F12 | Go to definition |
| E2E-KEY-06 | Ctrl/Cmd+Shift+G | Stage focused hunk |
| E2E-KEY-07 | F8 / Shift+F8 | Next / previous problem |
| E2E-KEY-08 | F2 | Rename (proposal) |
| E2E-KEY-09 | Shift+Alt+F | Format (proposal) |
| E2E-KEY-10 | Ctrl+Shift+O | Organize imports (proposal) |
| E2E-KEY-11 | F9 | Toggle breakpoint |
| E2E-KEY-12 | F5 | Launch / continue / refresh explorer (context-sensitive) |
| E2E-KEY-13 | Shift+F5 | Stop debug |
| E2E-KEY-14 | F10 / F11 / Shift+F11 | Step over / into / out |
| E2E-KEY-15 | Ctrl+Alt+H / Ctrl+Alt+Shift+H | Incoming / outgoing calls |
| E2E-KEY-16 | Alt+ArrowUp/Down | Debug stack frame |

**Residual (expected to stay typed unless product decides otherwise):**

| ID | Gesture | Why residual |
| --- | --- | --- |
| E2E-KEY-R1 | `:git-nav-*` | No default chord; KEYBOARD_REFERENCE also lists `]h` `[h` `]f` `[f` as **projected** SCM labels — score whether those chords actually bind in the live window. |
| E2E-KEY-R2 | `:term-launch` | Operand is a program string |
| E2E-KEY-R3 | `:test-*` | Not palette entries |

---

## 7. Language tooling (Rust / rust-analyzer)

Expected: trusted workspace with `Cargo.toml`; rust-analyzer starts **lazily** (first `.rs` open or palette Start). Health: Starting / Live / BackingOff+countdown / Unavailable / Failed.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-LSP-01 | Session does **not** start on workspace open (indexer/LSP off the open path). | CURRENT (GAP-09.3). |
| E2E-LSP-02 | Diagnostics appear in Problems; F8 walks them. | PARTIAL: GP-1 s3 + problems panel tests. Score in window. |
| E2E-LSP-03 | Completion popup 50ms debounce; ↑↓ Tab/Enter Esc; stale discarded. | PARTIAL (completion_popup). |
| E2E-LSP-04 | Hover 200ms settle; Esc dismisses until new hover id. | PARTIAL (hover_definition). |
| E2E-LSP-05 | F12 go to definition; NavigateToDefinition for multi-result. | PARTIAL. |
| E2E-LSP-06 | Find references, outline, inlay hints, code lenses (Run/Debug lenses). | PARTIAL / HARNESS-ONLY. |
| E2E-LSP-07 | Format / rename / organize / code action produce a **proposal**, not a silent disk write. Apply uses the proposal workflow. | PARTIAL (write-side translation + apply_activation tests). Score in window. |
| E2E-LSP-08 | Circuit-breaker backoff after crashes; Restart resets budget. | HARNESS-ONLY. |
| E2E-LSP-09 | Stderr ring is redacted in projection (no raw paths/secrets). | HARNESS-ONLY. |
| E2E-LSP-10 | Untrusted workspace does not spawn rust-analyzer as a privilege escalation. | CURRENT (policy). |
| E2E-LSP-11 | Non-Rust language packs / generic LSP SDK. | DEFERRED (P1 parking lot). |
| E2E-LSP-12 | `PrepareCallHierarchy` as its own gesture. | EXPECTED-UNBUILT (allowlisted: prepare is a no-op; incoming/outgoing keys exist). |

Hosted `xtask rust-analyzer-smoke` is merge-blocking **when provisioned**; it is not renderer-backed diagnostics UX.

---

## 8. Search

| ID | Expected | Current |
| --- | --- | --- |
| E2E-SRCH-01 | Workspace search: literal/regex, case, whole-word, glob; skips binaries (NUL in first 8KiB); `skipped_binary_count`. | CURRENT (worker-backed). |
| E2E-SRCH-02 | Dispatch returns immediately; UI shows Running; stale rows tagged `[outdated]`. | CURRENT. |
| E2E-SRCH-03 | Cancel search drops in-flight work. | CURRENT. |
| E2E-SRCH-04 | Structural search `#` pattern + optional rewrite **preview** (not apply). | CURRENT (workflow tests). |
| E2E-SRCH-05 | Multi-file search/replace as a product edit. | EXPECTED-UNBUILT (USER_GUIDE: out of scope until later). |
| E2E-SRCH-06 | Indexed workspace search toggle actually speeds large trees. | PARTIAL: setting exists; score whether it changes behavior. |

---

## 9. Git / SCM

Expected: off-thread `git` CLI inspection; remotes policy-gated; no silent network.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-GIT-01 | Status, syntactic diff, blame, graph, conflicts project in the SCM panel. | PARTIAL / HARNESS-ONLY + GP-1 s6. |
| E2E-GIT-02 | Stage/unstage hunk and path. | CURRENT (intents). |
| E2E-GIT-03 | Focus hunk then Ctrl+Shift+G stages focused hunk. | CURRENT (certified) **if** focus can be set; default focus still `:git-nav-*`. |
| E2E-GIT-04 | Commit: empty summary blocked; missing `user.name`/`user.email` blocked; missing Conventional Commits prefix warns only. | CURRENT. |
| E2E-GIT-05 | Switch/create/delete branch; stash. | CURRENT (palette + shell). |
| E2E-GIT-06 | Push/fetch/pull require host consent (`GrantGitRemoteHost`); untrusted/denied fails closed. | CURRENT (policy). |
| E2E-GIT-07 | Conflict resolve keep current / incoming. | CURRENT (intents). |
| E2E-GIT-08 | Linked worktree create/remove/prune; never touches network. | CURRENT. |
| E2E-GIT-09 | Local history: snapshot on successful proposal save; cap 50 entries or 50 MiB/file; restore is a proposal. Lives in `.legion/local-history/`. | CURRENT (GAP-04.2). |
| E2E-GIT-10 | Export worktree evidence: metadata-only TOML, no file bodies. | CURRENT. |
| E2E-GIT-11 | Native jj, in-IDE review comments, coverage. | DEFERRED (ledger PR-LANG-002 out of scope). |
| E2E-GIT-12 | Forge PR APIs. | DEFERRED (parking lot). |

---

## 10. Terminal

Expected: real PTY; trusted Manual auto-enables on first explicit launch; untrusted denied; shell from workspace → user → platform default; `LEGION_SECRET*` / `LEGION_TOKEN*` always stripped; scrollback default 5 000 rows.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-TERM-01 | `:term-launch <cmd>` in trusted workspace starts a PTY and shows output. | CURRENT runtime; **HARNESS-ONLY / typed-only** for launch. No default keymap. |
| E2E-TERM-02 | Input, resize, poll, search output, kill, close, orphan cleanup. | CURRENT (terminal tests). Score in window. |
| E2E-TERM-03 | Untrusted workspace: visible denial, no session. | CURRENT. |
| E2E-TERM-04 | A default “open terminal” chord (Ctrl+`). | EXPECTED-UNBUILT (residual). |
| E2E-TERM-05 | 3-OS renderer-backed terminal dogfood. | EXPECTED-UNBUILT (PR-LANG-002 still substrate). |

---

## 11. Debug

Expected: Microsoft DAP; `auto` does not fake stacks; live adapter from `LEGION_DAP_ADAPTER` / PATH (`lldb-dap`/`codellldb`) and security allowlist; `LEGION_DAP_MODE=live` fail-closed; fixture mode is tests-only.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-DBG-01 | F9 toggles breakpoint at cursor line. | CURRENT (keys). |
| E2E-DBG-02 | F5 launches first config when configs exist and idle; Continue when in session; Refresh Explorer when neither. | CURRENT (smart F5). |
| E2E-DBG-03 | F10/F11/Shift+F11/Shift+F5 step and stop. | CURRENT keys; live debugee in a window is PARTIAL. |
| E2E-DBG-04 | Conditional / hit / log breakpoints. | EXPECTED-UNBUILT as product UX (parking lot); DTO fields exist on `ToggleDebugBreakpoint`. |
| E2E-DBG-05 | Run to cursor, evaluate, watches, stack navigation. | PARTIAL (intents + tests). |
| E2E-DBG-06 | No adapter: honest “no session” + install hint, not a simulated stack. | CURRENT. |
| E2E-DBG-07 | Untrusted: `debug.adapter.launch` denied. | CURRENT. |
| E2E-DBG-08 | Interactive GUI dogfood against a real debugee on 3 OS. | EXPECTED-UNBUILT. |

---

## 12. Test explorer

| ID | Expected | Current |
| --- | --- | --- |
| E2E-TST-01 | Discover via `cargo test --list`; tree by module path. | HARNESS-ONLY (`:test-refresh`). |
| E2E-TST-02 | Run one (`--exact`) and run group. | HARNESS-ONLY (`:test-run` / `:test-run-group`). |
| E2E-TST-03 | LSP runnable preference when present. | HARNESS-ONLY. |
| E2E-TST-04 | Attach `TestRunSummary` to workflow evidence (metadata-only). | HARNESS-ONLY. |
| E2E-TST-05 | Palette entries / default keys for discover/run. | EXPECTED-UNBUILT. |
| E2E-TST-06 | Generic (non-cargo) test controller. | DEFERRED. |

---

## 13. Assist

Expected: human-in-control; cancellable, dismissible, auditable; proposal-mediated mutation.

**Provider routing (product contract):** `Auto` probes loopback Ollama then llama.cpp; else `deterministic-local`. `Auto` **never** falls back to Anthropic. Anthropic is explicit BYOK (env or keyring). Loopback URLs only.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-ASSIST-01 | Inline ghost prediction; Tab accept; dismiss; cancel. | PARTIAL: GP-2 + tests. Default GUI often fixture strings, not a live model. |
| E2E-ASSIST-02 | Assistant right rail with citations. | PARTIAL (assistant_rail tests). |
| E2E-ASSIST-03 | Explain / propose (`:ai-explain` `:ai-propose`) stay proposal-only. | PARTIAL. Live Assist generation is background-worker + stream poll; fixture Assist is synchronous. |
| E2E-ASSIST-04 | Context manifest before invocation (files, symbols, diagnostics, terminal excerpts, privacy labels). | HARNESS-ONLY (GP-2 s5). |
| E2E-ASSIST-05 | Privacy inspector: egress, redaction, consent visible. | CURRENT (control_trust tests) — inspectability, not “real model in GUI.” |
| E2E-ASSIST-06 | Model picker + Save Anthropic key; key never in session JSON. | PARTIAL (provider_key_entry / BYOK isolation tests). |
| E2E-ASSIST-07 | Unauthorized remote provider route is Refused. | CURRENT (GP-2 s4). |
| E2E-ASSIST-08 | Default GUI Assist uses a real local model when Ollama is up. | EXPECTED; often FAIL today (fixture). Score explicitly. |

---

## 14. Delegate

Expected: disposable worker, explicit scope, sandbox/worktree, tool permission prompts, proposals + evidence, no direct main-tree writes.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-DEL-01 | Start scoped task; worker completes; workspace bytes unchanged until apply. | HARNESS-ONLY (GP-3 s3). |
| E2E-DEL-02 | Forbidden path read → Blocked / ToolCallRejected. | HARNESS-ONLY (GP-3 s4). |
| E2E-DEL-03 | TerminalCommand in sandbox: Completed or Blocked, never silent host escape. | HARNESS-ONLY (GP-3 s5). |
| E2E-DEL-04 | Kill-switch cancels in-flight work. | HARNESS-ONLY (GP-3 s6). |
| E2E-DEL-05 | Orphan sandbox reap on next launch. | CURRENT (startup reap + GP-3 s7). |
| E2E-DEL-06 | Review hunks; apply via proposal; checkpoint/rollback. | HARNESS-ONLY (GP-3 s8). |
| E2E-DEL-07 | Delegate chat with live model vs fixture status + citations. | PARTIAL. |
| E2E-DEL-08 | ACP attach host; failure is an error, not a fake success. | PARTIAL (palette). |
| E2E-DEL-09 | Command-center GUI as the human path (not GP-3). | PARTIAL (desktop tests); score in window. |

---

## 15. Legion Workflows (Automate)

Expected: task graph, plan review, parallel workers, verification/sign-off/conflict/merge-readiness gates, MCP tool permission, kill-switch, replay from metadata.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-WF-01 | Unapproved plan does not produce a DAG. | HARNESS-ONLY (GP-4). |
| E2E-WF-02 | Approve plan → DAG; revise tracked. | HARNESS-ONLY. |
| E2E-WF-03 | Policy/budget/verification/conflict/kill-switch stop safely. | HARNESS-ONLY. |
| E2E-WF-04 | Evidence bundle replay without raw source. | HARNESS-ONLY. |
| E2E-WF-05 | Human uses Workflows mode in the live window to run the same loop. | EXPECTED; treat as FAIL until journaled. |
| E2E-WF-06 | Autonomous apply/merge without approval. | Forbidden. A PASS here is a **product-safety FAIL**. |

---

## 16. Proposals (universal mutation)

Every non-keystroke workspace mutation is expected to be a proposal: save, AI, LSP write, plugin, terminal-generated edit, restore-from-history.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-PROP-01 | Preview / approve / reject / apply / rollback / cancel / details. | CURRENT (intents + tests). |
| E2E-PROP-02 | Reject/rollback reasons recorded as metadata. | CURRENT. |
| E2E-PROP-03 | Apply is atomic enough to restore on failure; dirty editor preserved on reject. | CURRENT. |
| E2E-PROP-04 | Proposal UI in the live window (cards, risk strip, timeline). | PARTIAL (view modules + tests). |

---

## 17. Settings, About, diagnostics

| ID | Expected | Current |
| --- | --- | --- |
| E2E-SET-01 | Settings overlay: appearance, editor, AI providers, extensions, notifications, privacy, advanced. | CURRENT surface; score each control’s **effect**. |
| E2E-SET-02 | Theme / zoom / toast verbosity / crash-report consent persist and reload. | PARTIAL. Crash reports still do not upload. |
| E2E-SET-03 | About: version, proprietary license, not GA, Manual/opt-in privacy, SKU line if present. | CURRENT. Do not treat About as an installer SKU proof. |
| E2E-SET-04 | Support bundle metadata-only (no editor text, secrets, raw crashes). | CURRENT (GAP-10.2). |
| E2E-SET-05 | Setup/welcome checklist. | CURRENT (setup panel). |
| E2E-SET-06 | Font family picker changes rendering. | EXPECTED-UNBUILT (see E2E-EDIT-14). |

---

## 18. Session, crash safety, observability

| ID | Expected | Current |
| --- | --- | --- |
| E2E-SES-01 | Kill while dirty → relaunch restores dirty text from `.legion/unsaved/` SHA-256 sidecars. Disk file unchanged until proposal save. | CURRENT (GAP-04). **Must** be scored by killing the product window. |
| E2E-SES-02 | Session JSON has no dirty bodies and no raw-secret markers. | CURRENT. |
| E2E-SES-03 | Canvas positions persist in `WorkspaceSessionRecord` by canonical path. | CURRENT (canvas). |
| E2E-SES-04 | Native minidump + crash upload. | DEFERRED / local-only by design. |
| E2E-SES-05 | Check for update in the desktop; signed manifest; installer swap; N−1 restore; hosted feed. | EXPECTED-UNBUILT as product UX (update-drill is local/ephemeral; GAP-03). |

---

## 19. Trust, sandbox, plugins, extensions

| ID | Expected | Current |
| --- | --- | --- |
| E2E-TRU-01 | Workspace trust: Trusted vs Untrusted changes terminal/debug/git-remote. | CURRENT. |
| E2E-TRU-02 | Sandbox panel shows honest OS caveats (Windows job-object-only, etc.). | PARTIAL (sandbox tests). |
| E2E-TRU-03 | Plugin invoke is capability-scoped; writes are proposals. | PARTIAL (plugin_management tests). Product WASM execution for user plugins: DEFERRED. |
| E2E-TRU-04 | Install/update/remove **signed** extension; per-capability grant (no “trust all”). | PARTIAL substrate (extensions_panel). Runtime Node/`vscode` host: **DEFERRED (PR-VSC-002)**. |
| E2E-TRU-05 | `package.json` / Open VSX manifest parse, contribution classification, compatibility report — no execution. | CURRENT (PR-VSC-001 substrate). Unwired from shipped product binaries. |
| E2E-TRU-06 | VSIX install as a beta-loop step. | DEFERRED (conflicts with PR-VSC-002). |

---

## 20. Collaboration, remote, Cloud Lane

| ID | Expected | Current |
| --- | --- | --- |
| E2E-ENT-01 | SSH / Dev Container remote IDE: connect, encrypted transport, reconnect/offline, remote FS/LSP/terminal, health. | DEFERRED (PR-ENT-001). `legion-remote` tests are mock/default-deny. |
| E2E-ENT-02 | Presence, shared workspace, CRDT, shared proposals, admin SSO/SCIM. | DEFERRED (PR-ENT-002). |
| E2E-ENT-03 | Cloud Lane: opt-in hosted workers, visible upload scope, budget, cancel. | PARTIAL harness / `CancelCloudLaneTask`. Not product remote UX. |
| E2E-ENT-04 | `:collab-join/leave/presence` in a real shared session. | EXPECTED-UNBUILT as product; commands exist as substrate. |

---

## 21. Accessibility

| ID | Expected | Current |
| --- | --- | --- |
| E2E-A11Y-01 | Keyboard-only path for certified routes (GAP-05.1). | CURRENT (tests). Score in window. |
| E2E-A11Y-02 | Windows screen-reader session (Narrator/NVDA) of a **live** window. | CURRENT for Narrator transcript (GAP-05.2). UIA dump is not a session. |
| E2E-A11Y-03 | macOS VoiceOver notes of a live window. | EXPECTED-UNBUILT (GAP-05.3). AX dump exists (`AX_WALK_OK` on `Legion IDE Smoke`); that is **not** VoiceOver. |
| E2E-A11Y-04 | Linux Orca notes of a live window. | EXPECTED-UNBUILT (GAP-05.4). Hosted xvfb AT-SPI did not see AccessKit. |
| E2E-A11Y-05 | High contrast, SR projection, focus restore, multi-monitor. | PARTIAL (platform smoke). Ledger PR-UI-001 still substrate. |

---

## 22. Performance

| ID | Expected | Current |
| --- | --- | --- |
| E2E-PERF-01 | Input-to-paint p50/p95 budgets on the **renderer**. | PARTIAL: armed paint rows; skeleton rows report-only. |
| E2E-PERF-02 | Open does not wait on `LexicalIndexer`. | CURRENT (GAP-09.3). |
| E2E-PERF-03 | 100MB file paint (not EditorEngine text-model). | PARTIAL; do not treat 269µs-class text-model numbers as paint. |
| E2E-PERF-04 | 100k-file tree open &lt; 10s; watcher burst debounce; search cancel. | HARNESS-ONLY (scale tests). |

---

## 23. Packaging, install, update (expected product vs wrapping)

Score these only after the daily-driver window works. They are not a substitute for E2E-EDIT-*.

| ID | Expected | Current |
| --- | --- | --- |
| E2E-REL-01 | Unsigned-beta portable zip/tar.gz on 3 OS (`package-preview.*`, `legion-preview.yml`). | CURRENT (hosted preview). `production = false`. |
| E2E-REL-02 | Native MSI/DMG/DEB/AppImage via `package-native.*`. | PARTIAL scripts; unsigned-beta `signer_status`. |
| E2E-REL-03 | LICENSE, PRIVACY.md, THIRD_PARTY_NOTICES.md inside the package. | CURRENT (GAP-10.3). |
| E2E-REL-04 | Signed Authenticode / Developer ID+notarize / Linux minisign. | EXPECTED-UNBUILT (GAP-02; QUAL.11 #211/#212/#213). |
| E2E-REL-05 | Fresh-VM SmartScreen / Gatekeeper / Linux trust **without** click-through. | EXPECTED-UNBUILT (GAP-02.3). |
| E2E-REL-06 | Hosted signed update feed; replace; restart; interrupt; rollback; N−1. | EXPECTED-UNBUILT (GAP-03). Update-drill is local ephemeral. |
| E2E-REL-07 | Separate signed Manual/offline SKU + OS packet-capture zero-egress. | EXPECTED-UNBUILT (GAP-06). Offline **compile** exists (`cargo check -p legion-desktop --no-default-features --features offline`). Do not treat SKU packaging PRs as the product. |
| E2E-REL-08 | Windowed GUI 3-OS CI (`legion-windowed-gui.yml`) open/edit/save. | CURRENT as independent job; four green runs + owner sign-off recorded; **not** a PR required check. |

---

## 24. Golden-path harnesses (not the cake)

Use these as regression oracles for composition, not as “the app works.”

| ID | Harness | Steps (expected) |
| --- | --- | --- |
| E2E-GP1 | GP-1 Manual | s1 copy fixture + trusted open; s2 LSP init (skip if no RA); s3 diagnostic introduce/detect/fix/clear; s4 workspace search; s5 terminal `cargo test` (skip if no PTY); s6 edit-save-stage-commit; s7 evidence TOML. |
| E2E-GP2 | GP-2 Assist | s1 open; s2 Assist mode; s3 inline predict accept undo; s4 provider allow/deny; s5 context manifest; s6 proposal apply/restore; s7 evidence. |
| E2E-GP3 | GP-3 Delegate | s1 Delegate mode; s2 scope; s3 worker loop workspace unchanged; s4 forbidden path; s5 sandbox terminal; s6 kill-switch; s7 orphan reap; s8 review-apply. |
| E2E-GP4 | GP-4 Workflows | Plan requires review; revise/approve DAG; workers; policy/budget/verify/conflict/kill-switch; command-center projections; evidence replay (~13 steps). |
| E2E-GP5 | golden-path-5 / `--beta-smoke` | **Must not** be used as GAP-01 windowed GUI. Headless. |
| E2E-UPD | update-drill | Deterministic apply/rollback with ephemeral key. Not installer-swap. |

---

## 25. Expected product that is not built (keep on the scorecard)

These are in the **product target**, ledger beta scenario, pivot, or parking lot. Automation should list them as NOT-BUILT/DEFERRED rather than silently omit them.

| ID | Expected product | Bucket |
| --- | --- | --- |
| E2E-X-01 | A human daily-driver window: open repo, edit, save, search, git, terminal, LSP, quit — journaled, not GP-1. | EXPECTED; USER_GUIDE still says this is **not** true. **Primary cake.** |
| E2E-X-02 | Beta loop: large repo + VSIX + Rust completion + AI multi-file change + context manifest + proposal review + tests + debug + collaborate + save + audit export. | EXPECTED; VSIX + collaborate steps are DEFERRED — scenario is not reachable as written. |
| E2E-X-03 | VoiceOver live-window notes (macOS). | GAP-05.3 |
| E2E-X-04 | Orca live-window notes (Linux). | GAP-05.4 |
| E2E-X-05 | Signed 3-OS installers + fresh VM trust. | GAP-02 |
| E2E-X-06 | Hosted update feed + process restart + N−1. | GAP-03 |
| E2E-X-07 | Manual SKU with OS packet-capture zero egress. | GAP-06 |
| E2E-X-08 | Isolated Node extension host, webviews, notebooks, custom editors, marketplace. | PR-VSC-002 |
| E2E-X-09 | SSH / Dev Containers product UX. | PR-ENT-001 |
| E2E-X-10 | Collaboration + enterprise admin (SSO/SCIM). | PR-ENT-002 |
| E2E-X-11 | Language Pack SDK / non-Rust servers. | P1 parking lot |
| E2E-X-12 | DAP condition/hit/log + windowed real-adapter journal. | P1 |
| E2E-X-13 | Generic test controller. | P1 |
| E2E-X-14 | Merge editor / settings profiles / workbench splits. | P1 |
| E2E-X-15 | Provider catalog / local-model sidecar as a product default. | P1 |
| E2E-X-16 | Native minidump process. | P1 |
| E2E-X-17 | `legion-agentd`, AppContainer/VM isolation beyond current sandbox. | P1 |
| E2E-X-18 | MCP 2026-07-28 / ACP v1 workbench (ACP attach is a thin local bridge only). | P1 |
| E2E-X-19 | Canvas: derived import/call edges, syntax-colored cards, minimap, groups, terminal/test/proposal nodes. | Direction doc |
| E2E-X-20 | Font family actually applied to the code canvas. | Allowlisted no-op |
| E2E-X-21 | Default keymap for terminal launch, test run, git hunk nav. | Residual by policy unless product changes |
| E2E-X-22 | Live-model adversarial evals in CI. | Deferred (keys) |
| E2E-X-23 | Multi-window / per-monitor DPI restore as product UX. | Ledger residual |
| E2E-X-24 | Training flywheel: opt-in redacted traces, specialist training. | Pivot; not daily driver |

---

## 26. Suggested automation order

Run in this order so wrapping cannot masquerade as the product:

1. **E2E-EDIT-01…12** in the idle native window on Windows (this host).  
2. **E2E-NAV**, **E2E-SRCH**, **E2E-KEY** certified set, **E2E-LSP-01…07**, **E2E-GIT-01…10**, **E2E-TERM-01…03**.  
3. **E2E-ASSIST** with Ollama up, then with Ollama down (fixture).  
4. **E2E-DBG** if an allowlisted adapter exists; otherwise E2E-DBG-06.  
5. GP-1…4 as **composition regression**, recorded separately from (1).  
6. Windowed GUI 3-OS job as **open/edit/save only**.  
7. A11y transcripts (Narrator current; VoiceOver/Orca expected).  
8. Packaging/signing last, and only after (1) is honestly PASS.

---

## 27. Sources (do not invent extra product)

- `docs/USER_GUIDE.md`, `docs/MODES.md`, `docs/KEYBOARD_REFERENCE.md`, `docs/PRIVACY.md`, `docs/LEGION_PIVOT.md`  
- `docs/ui/canvas-workspace-direction.md`, `docs/ui/four-mode-prototype-fidelity.md`  
- `plans/product-readiness-ledger.md`  
- `plans/p0-installed-product-sequence-v0.1.md`  
- `crates/legion-ui/src/ui.rs` (`CommandDispatchIntent`)  
- `crates/legion-ui/src/shell_commands.rs`  
- `crates/legion-app/src/lib.rs` (`palette_command_specs`)  
- `xtask/intent-reachability.toml`  
- `crates/legion-desktop/src/workflow.rs` (`run_from_env` / `run_native`)  
- Golden-path binaries under `crates/legion-app/src/bin/golden_path_*.rs`

When code and this catalog disagree, **code plus a named test** wins for CURRENT; **USER_GUIDE / MODES / ledger** win for EXPECTED. Update this file in the same change as a new palette command, keymap, or deferral.
