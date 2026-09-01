# Legion User Guide

This guide is the end-user entry point for the current Legion product paths.
A packaged desktop app is not required to read it: a local `cargo run -p legion-desktop` build is enough to follow the smoke-oriented steps, and many surfaces below are test-exercised only.

> **Current state caveat.** The repo currently proves a validated substrate and a deterministic desktop projection workflow (CLI `:w` / `:q`, projection-only UI, headless desktop smoke harness). It is **not** a renderer-backed daily-driver product yet. Treat anything below as a description of the design and the gated surfaces that are exercised by tests, not as a claim of a shipped user experience. For the readiness matrix and remaining product gaps, see `plans/product-readiness-ledger.md` and `README.md` "Current Status".

> **Product areas that are currently projection-only, gated, or otherwise not yet productized.** The following are explicitly *not* full product paths today: debug productization (product `auto` does not fabricate simulated stacks: if no live adapter resolves it reports no session plus an install hint; explicit `LEGION_DAP_MODE=fixture` is reserved for tests; live spawn uses **Microsoft DAP** wire via `LEGION_DAP_ADAPTER`, `PATH` (`lldb-dap`/`codelldb` preferred-name-first), or `LEGION_DAP_USE_FAKE` for the in-tree fake adapter; every resolved binary must also be named in the security policy's debug adapter allowlist (`lldb-dap`, `lldb-vscode`, `codelldb` by default), so `LEGION_DAP_ADAPTER` selects where an adapter is, not what may be launched; `LEGION_DAP_MODE=live` fails closed; untrusted workspaces deny `debug.adapter.launch` through the capability broker; system-adapter handshake dogfood is optional via `cargo test -p legion-debug --test system_adapter_dogfood` with `LEGION_DAP_DOGFOOD=1` to require a real binary; full launch/step against a host debugee and interactive GUI dogfood remain residual), runtime plugin execution (product composition does not run plugin WASM; a wasmtime host exists for boundary/fixture tests only; marketplace/VSIX runtime remains deferred under `PR-VSC-002`), collaboration GUI / production collaboration, remote workspace / Cloud Lane UX (substrate harness and transport contracts only; not SSH/devcontainer product UX), and signing / notarization / auto-update / crash reporting (dry-run descriptors and local drills only; no private signing credentials may be committed). Autonomous apply/merge is unsupported outside explicitly approved proposal paths. See `docs/LEGION_PIVOT.md` and `plans/legion-production-master-plan-v0.2.md` for the path to activating these surfaces.

> **Terminal runtime (real PTY; `PR-LANG-002` still Substrate validated).** The terminal is backed by a real PTY via the workspace trust and capability policy gate. Trusted workspaces in Manual mode auto-enable the terminal on the first explicit launch intent; untrusted workspaces are denied unconditionally. Shell selection (PowerShell Core / cmd / bash / zsh) follows workspace → user → platform-default precedence. The `LEGION_SECRET*` and `LEGION_TOKEN*` environment variable deny-list is always applied regardless of trust state. Scrollback is bounded (default 5 000 rows). This is the runtime, not a ledger promotion: product-workflow validation (dogfood journal, 3-OS, renderer-backed) has not landed. See `plans/evidence/production/PR-LANG-002/terminal-trusted-pty-workflow.md` and `docs/TROUBLESHOOTING.md` for terminal failure states.

## Start here

1. Read `docs/INDEX.md` for the canonical documentation map.
2. Use `docs/MODES.md` to understand what each product mode allows and forbids.
3. Use `docs/KEYBOARD_REFERENCE.md` for the current projected shortcut labels.
4. Use `docs/TROUBLESHOOTING.md` when a smoke, package, or release path fails.

## Core product paths

### Manual

Manual mode is the deterministic local editing path.
Use it when you want the projection-only UI, workspace navigation, and trusted local file operations without any AI or worker surfaces.

#### Integrated terminal

An explicit terminal launch in a trusted Manual workspace starts the real PTY runtime and projects a running session. Input is sent through the terminal intent path and returned output is polled into the terminal projection; resize, kill, and bounded scrollback are part of the same lifecycle. An untrusted workspace receives a visible denial before a session is created, even if a test-only runtime override is present.

The local evidence packet covers launch, input/output, kill/orphan cleanup, and scrollback limits with named tests. It is evidence for the runtime and workflow seams, not a claim of renderer-backed 3-OS dogfood or a promotion of `PR-LANG-002`.

#### Workspace search

Search operates on the active file or across the entire workspace without mutating any files.
Dispatch returns immediately; the scan runs on an app-owned worker and the desktop drains the
latest completed generation without blocking the frame. Repeated queries coalesce to the newest
request, so an older result may be discarded rather than replacing the latest query.
Multi-file search/replace is explicitly out of scope until M9; the search surface is read-only.

Options available in workspace search:

- **Literal / Regex** — search using a plain string or a regular expression.
- **Case-sensitive** — match uppercase and lowercase exactly as typed.
- **Whole-word** — restrict matches to word boundaries.
- **Glob filter** — restrict which files are walked (e.g. `*.rs`, `src/**`).

Binary files are detected by a NUL-byte heuristic (first 8 KiB window) and skipped automatically;
the search report includes a `skipped_binary_count` field that records how many were bypassed.

When a new query begins, results from the previous query are marked **stale** until the new
results arrive. The projection reports **Running** while work is in flight, and stale rows remain
observable and rendered de-emphasised (tagged `[outdated]` in the desktop projection) until a
newer generation settles.

#### Command palette

The command palette (opened from the app bar) supports three modes: file opener, symbol finder,
and command dispatcher.  Results are ranked by fuzzy score (consecutive-run, word-boundary,
camelCase, path-segment, and filename-region bonuses) blended with a recency signal and a
frequency bonus.  The frequency bonus accumulates metadata-only usage counts per workspace;
no raw query text, AI context, or network I/O is involved.

#### Optional ACP host bridge

The command palette exposes **ACP: Attach Host** as an opt-in local adapter bridge. Enter the
host program and any arguments after the command, for example `> acp attach host claude --print`.
The configured host is invoked only from the delegated proposal workflow, with the sandbox,
target proposal path, and plan id supplied through `LEGION_ACP_*` environment variables.

This does not add an ACP workbench or completion-class MCP behavior: the host remains outside
the editor and its work is still proposal/evidence mediated. If no host is attached, Manual mode
and the existing delegated workflow are unchanged. If the host cannot start or exits unsuccessfully,
the app reports an error rather than fabricating a successful run. Use the command again with the
desired program to replace the attached host; restart the app to clear the opt-in configuration.

The command dispatcher is the reachable entry point for the product loops covered by the current
desktop tests. The command palette registry exposes the commands listed by the app projection,
including `Git: Push`, `Git: Fetch`, `Git: Pull`, `Language Server: Start`,
`Language Server: Restart`, `Help: About`, and `Help: Export Support Bundle`;
other product loops use typed shell commands that become app-owned dispatch intents.

Git hunk navigation/staging, test discovery/run, and language write requests are typed shell
surfaces rather than command-palette entries:

- **Git:** `:git-nav-next-hunk`, `:git-nav-prev-hunk`, `:git-nav-next-file`,
  `:git-nav-prev-file`, and `:git-stage-hunk <hunk-id>`.
- **Tests:** `:test-refresh`, `:test-run <item-id>`, and `:test-run-group <label>`;
  discovery and execution remain worker-backed.
- **Language:** `:format`, `:rename <position>,<name>`, `:organize-imports`, and
  `:code-action <id>`; each remains proposal-mediated.

These are typed-intent/projection contracts, not claims of palette reachability, default-keymap
bindings, renderer-backed 3-OS dogfood, or automatic mutation outside an approved proposal path.

### Assist

Assist mode keeps the human in control while exposing AI-backed suggestions.
Use it when you want previews, explanations, and proposal-mediated edits without giving the model direct mutation authority.

> **Assist / Delegate provider routing today.** Product composition is **local-first by default** (`Auto`): a fast loopback probe prefers **Ollama** (`OLLAMA_BASE_URL`, default `http://127.0.0.1:11434`, model `OLLAMA_MODEL` or `llama3.2`) when reachable; failing that it probes a **llama.cpp** server (`LEGION_LLAMA_CPP_BASE_URL`, default `http://localhost:8080/v1`, model `LEGION_LLAMA_CPP_MODEL`); failing both it uses the offline `deterministic-local` fixture (CI / zero-egress). Both local backends are loopback-only and are refused if the configured URL is not a loopback address. **`Auto` never routes to Anthropic**, whether or not BYOK credentials are present: sending the buffer excerpt to a metered remote provider is a choice somebody makes rather than a fallback they discover on the invoice. Select **Anthropic** explicitly for that (env `ANTHROPIC_API_KEY` / `LEGION_ANTHROPIC_API_KEY` or OS keyring via the Model Picker **Save Anthropic key** control). Override with Model Picker route buttons or `LEGION_AI_PROVIDER=auto|ollama|llama-cpp|anthropic|deterministic`. Anthropic uses progressive Messages **SSE**. **Live Assist proposals and Delegate chat** run generation on a **background worker** (UI returns immediately with streaming status; frame `poll_product_ai_stream` finalizes the assistant turn or registers the Assist proposal). Offline/fixture (`deterministic`) Assist remains synchronous so tests and CI keep receiving `proposal_id` in-call. Results remain proposal-mediated / accept-gated.

### Delegate

Delegate mode is for bounded worker execution.
Use it when a task should run in a disposable lane with explicit scope, evidence, and review before anything reaches the main workspace.

> **Delegate chat replies.** With Anthropic BYOK credentials (env or keyring), Delegate chat asks the live model and shows the reply text. Without credentials, chat returns an offline fixture status line that still attaches retrieval citations. Policy route records remain metadata-only (fingerprints / byte counts); raw model prose is not stored in the assisted-AI route DTO.

### Legion Workflows

Legion Workflows coordinates multi-step product workflows.
Use it for task graphs, approval gates, risk tracking, and release-oriented orchestration.

### Language tooling (Rust LSP — read-side)

Language tooling is available for trusted Rust workspaces that contain a `Cargo.toml`.

**When does rust-analyzer start?** The session starts *lazily*: it is not spawned on
workspace open. Instead it starts when either:

1. You open the first `.rs` buffer in a trusted workspace (automatic lazy trigger), or
2. You run the **"Language Server: Start"** command from the command palette (`>`).

This avoids paying the rust-analyzer spawn cost for workspaces where you never open a Rust
file, and for non-Rust workspaces that happen to be trusted.

**Palette commands for lifecycle control:**

| Command | Description |
| --- | --- |
| Language Server: Start | Start rust-analyzer for the current workspace (no-op if already starting or live). |
| Language Server: Restart | Force-restart rust-analyzer, resetting the circuit-breaker restart budget. |

**What is currently wired (read-side):**

- **Diagnostics panel** — workspace errors and warnings from rust-analyzer appear in the
  problems panel (`language_tooling_projection.problems`) and refresh on every file change.
- **Completion popup** — triggered on text edits with a 50 ms debounce. Navigate with
  `↓`/`↑`, accept with `Tab` or `Enter`, dismiss with `Esc`. Stale results (from before the
  last edit) are automatically discarded by the snapshot gate.
- **Hover tooltip** — appears after a 200 ms settle period when the cursor rests over a symbol.
  Dismiss with `Esc`. The tooltip stays closed after explicit dismiss until a new response
  arrives with a different hover id.
- **Go to definition** — available with the default `F12` shortcut and through the typed
  shell/dispatch-intent path. Use `NavigateToDefinition { index }` to open a specific result.
- **Language health status** — the language status panel projects `Starting`, `Live`,
  `BackingOff` (with countdown), `Unavailable`, or `Failed` states from
  `lsp_session_status` in the `LanguageToolingProjection`.

**What is deferred (write-side, P2.F1.T5):**
Rename, format, code actions, and organize imports are typed shell/dispatch-intent proposal
surfaces rather than command-palette entries. They generate proposal previews, but they are not
direct edits. Apply activation remains gated by the existing proposal workflow and kanban task
P3.F1.T2.
See `plans/product-readiness-ledger.md` PR-LANG-001 for the current gate status.

## Support and release surfaces

- **Help: About** (command palette) opens the About overlay: version, proprietary license, Manual/opt-in AI privacy posture, and crash-report consent. This is not a general-availability claim.
- **Help: Export Support Bundle** writes a metadata-only file to `.legion/support-bundle.md` through app authority. It does not include editor text, secrets, or raw crash bodies. Settings → Privacy has the same export.
- Privacy policy: `docs/PRIVACY.md`. License: `LICENSE` (proprietary, not OSI). Third-party notices: `THIRD_PARTY_NOTICES.md`.
- For packaging and release preparation, start with `docs/OPERATOR_RUNBOOK.md`.
- For diagnostic exports, session state, and bug-report payloads, use `docs/TROUBLESHOOTING.md`.
- For release-readiness status, check `plans/product-readiness-ledger.md`.

## Source control (SCM)

Legion integrates with Git through the command palette's Git lifecycle/remote entries, typed
shell intents, and the SCM projection.

### Diff review navigation

When a diff is open, the typed shell commands below emit projection-only navigation intents:

| Typed command | Description |
| --- | --- |
| `:git-nav-next-hunk` | Move focus to the next changed hunk in the current file. |
| `:git-nav-prev-hunk` | Move focus to the previous changed hunk in the current file. |
| `:git-nav-next-file` | Move focus to the next changed file in the diff. |
| `:git-nav-prev-file` | Move focus to the previous changed file in the diff. |
| `:git-stage-hunk <hunk-id>` | Stage the focused unstaged hunk through the existing policy path. |

Focus state is tracked in the application layer; the desktop projection reflects the current `focused_hunk_id` from `GitProjection`.

### Commit validation

Before committing, Legion validates:

- **Summary line** — must be non-empty (hard error; the commit action is blocked).
- **Author identity** — name and email are read from `git config`; missing values are a hard error.
- **Conventional Commits prefix** — if the summary does not start with a recognised CC prefix (`feat`, `fix`, `refactor`, etc.) an advisory warning is surfaced, but the commit is not blocked.

### Local file history

Legion records a content snapshot every time a file is successfully saved through the proposal workflow.
Snapshots are bounded to 50 entries or 50 MiB per file (whichever limit is reached first).

Metadata (timestamps, content hash, file identity) is stored in memory by `LocalHistoryMetadataStore`.
Content blobs are written to `.legion/local-history/<path-key>/` inside the workspace and are workspace-local — they are never pushed to remote.

To browse or restore from local history:

1. Open the command palette and run **Git: Local History**.
2. Select the entry to restore.
3. The restore goes through the standard proposal/save workflow — it inherits fingerprint, version, generation, and correlation/causality tracking; no direct writes occur.

### Git worktrees

Use the **Git: New Worktree** palette command to create a linked worktree for an existing branch.
Worktree creation never touches the network.

### Evidence export

Run **Git: Export Worktree Evidence** to write a metadata-only TOML snapshot of the current worktree state to `.legion/evidence/`.
The export contains only metadata (branch, HEAD, path); no file content is included.

## What this guide does not cover

- low-level architecture ownership rules: see `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`;
- mode-policy details: see `docs/MODES.md`;
- historical rename context: see `docs/LEGION_RENAME.md`.
