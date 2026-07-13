# Legion IDE — Full Codebase Map & Release-Readiness Gap Audit

**Date:** 2026-07-13
**Scope:** Entire Rust workspace (29 crates + `xtask`), Python training/eval harness, packaging/CI, and the product-planning docs.
**Method:** Multi-agent static audit — one auditor per crate plus dedicated auditors for build/release, the Python/model story, an IDE table-stakes baseline, and five documentation sets. Blocker/major findings were then re-checked by independent adversarial verifiers. 78 agent passes total; every finding below carries `file:line` evidence, and the highest-severity items were confirmed a second time. Tests were **not** run as part of this audit (an unrelated `libdbus`/keyring build failure blocks the suite in this environment), so all conclusions are from source reading and cross-crate wiring analysis.

> **Terminology.** "Wired" = reachable from a shipped product binary (`legion-desktop`, the GUI; `legion-app`'s line-mode CLI; or `legion-cli`). A crate can be fully implemented and well-tested yet **unwired** — present in the workspace but never constructed by any product binary. Unwired code is dead weight from a user's perspective, and it is the single most common failure mode in this codebase.

---

## 1. Headline verdict

**Legion IDE cannot be released to the public as a fully functional IDE today.** The workspace is an architecturally serious, well-tested *substrate* — but a large fraction of the user-facing product is either simulated, unwired, or launch-only. The gap is not "a few rough edges"; it is that several **core IDE interactions and the flagship AI features do not actually work in the shipped binary**, and there is **no distributable installer for any platform**.

This finding is consistent with the repo's own documentation. The `plans/product-readiness-ledger.md`, `plans/phase-status-ledger.md`, and `HERMESGOAL-GAP-ANALYSIS.md` are candid that Phase 0–8 acceptance is *substrate* acceptance, not product GA, and that exactly one readiness row (`PR-AI-001`) has reached "Product workflow validated." This audit corroborates the docs and adds the concrete, file-level list of what is missing. Where the audit and the docs diverge, it is because a few surfaces are marked "substrate validated" or "Accepted" while the underlying product path is a fabricated placeholder (called out in §6).

The three things that most define the gap:

1. **The editor cannot delete text or insert newlines in the shipped GUI.** Backspace, Delete, and Enter are handled nowhere for the buffer; copy/cut discard the copied text. This alone makes the product unusable as a text editor.
2. **The "AI-native" product ships fake AI by default.** Every UI-reachable AI surface — inline prediction, assist edits, chat — returns hardcoded/deterministic placeholder strings. Real model adapters exist and are compiled in, but no user-facing action reaches them, and API keys entered in the UI are never loaded.
3. **There is no installer, no code signing, and no auto-update channel.** The release pipeline only writes descriptor TOML files; nothing builds a `.dmg`/`.msi`/`.deb`/AppImage, and the updater has no server to talk to.

Below these are a debugger that runs no debugger, a terminal that can launch but not accept input, a file tree capped at depth 2, no on-disk persistence for the shipped app, no real file watcher, and a plugin system that executes no plugin code.

### Completeness map at a glance

| State | Crates |
| --- | --- |
| **Complete** | `legion-text` |
| **Mostly complete** (real, wired, minor gaps) | `legion-editor`, `legion-index`, `legion-lsp`, `legion-platform`, `legion-project`, `legion-protocol`, `legion-security`, `legion-storage`, `legion-ui`, `legion-agent`, `xtask` |
| **Partial** (real code, but key paths simulated or unwired) | `legion-app`, `legion-desktop`, `legion-ai`, `legion-ai-providers`, `legion-cli`, `legion-collaboration`, `legion-memory`, `legion-observability`, `legion-plugin`, `legion-sandbox`, `legion-telemetry`, `legion-terminal`, `legion-tracker`, `legion-vscode-compat`, `legion-remote-transport` |
| **Skeleton** (metadata simulation, no real behavior) | `legion-debug`, `legion-remote` |

Note that "mostly complete" describes the *crate*, not the *feature*: several mostly-complete crates (`legion-index`, `legion-lsp`, `legion-editor`) have their best machinery unwired from the product, so the user-visible feature is far less complete than the crate.

---

## 2. How to read this report

The user's question has two halves, and both matter:

- **(A) Baseline gaps** — things a *general-purpose public IDE* cannot ship without, regardless of Legion's specific plans (edit text, run a debugger, install the app).
- **(B) Planned-feature gaps** — features Legion's own docs promise that are **missing or non-functional** (collaboration, remote dev, VS Code extensions, local-model AI, hosted telemetry).

Several of the (B) items are **deferred by design** with explicit cut lines in the ledger — the team already knows they are not done. They are still listed here because the user asked for "any of the planned features missing or non-functional," but they are clearly separated from the (A) blockers and the *undocumented* simulations, which are the more urgent problem.

Severity: **Blocker** = a public GA IDE cannot ship without it. **Major** = a planned feature is partially implemented, non-functional, or unwired. **Minor** = polish/debt.

---

## 3. Release blockers (confirmed)

These 25 items were confirmed by adversarial verification. They are grouped by theme. Every one is reachable (or conspicuously *unreachable*) in the shipped `legion-desktop` binary.

### 3.1 Core editing is broken in the GUI

| # | Blocker | Evidence |
| --- | --- | --- |
| B1 | **Editor cannot delete text or insert newlines.** Production egui input maps only Text/Paste/IME + arrows/PageUp/Down/Tab/Escape. Backspace, Delete, and unmodified Enter are handled nowhere for the buffer; `DesktopAction::DeleteRange`/`ReplaceRange` are emitted only by tests. Typing works; correcting a typo or adding a line does not. | `crates/legion-desktop/src/workflow.rs:3791-3836`, `:3887-3971` |
| B2 | **Copy/cut lose the copied text.** `ClipboardCopy`/`Cut` return metadata only; no code writes the selection to the OS clipboard (only the terminal-row Copy button calls `ctx.copy_text`). Cut deletes without copying; Ctrl+C→Ctrl+V pastes stale/empty content. | `crates/legion-desktop/src/workflow.rs:3807-3811`; `crates/legion-app/src/lib.rs:16600-16619` |

These two make the editor non-viable on their own. (Ctrl+X-on-selection and paste-with-newlines are the only awkward workarounds.)

### 3.2 The AI-native product ships fake AI

| # | Blocker | Evidence |
| --- | --- | --- |
| B3 | **Inline "next edit" prediction is a canned string.** The only functional `predict_inline` implementation is `DeterministicInlinePredictionProvider`, whose ghost text is literally `" // next edit line N"`. `legion-app` hardcodes `DETERMINISTIC_LOCAL_PROVIDER_ID`; every real adapter (Ollama, OpenAI, llama.cpp, Anthropic, Copilot NES, Mercury, Codestral) returns unavailable. | `crates/legion-ai/src/lib.rs:495-506`; `crates/legion-app/src/lib.rs:23896-23917`; `crates/legion-ai-providers/src/lib.rs:334-340` |
| B4 | **Assist-mode AI produces a hardcoded placeholder edit.** `run_assisted_ai_operation` always routes to the deterministic provider and inserts a fixed `"/* phase4 local AI proposal */\n"` at byte 0. Registered real providers are never selected on this path. Reachable from the desktop "Start AI Proposal" button. | `crates/legion-app/src/lib.rs:21292-21293, 21407-21426`; `crates/legion-desktop/src/view.rs:922` |
| B5 | **Delegate chat returns synthesized status text, not a model reply.** `send_delegate_chat` renders `"Delegate provider answer ready via N citation(s); route=…"` — never actual chat output — even in `ai`-feature builds. | `crates/legion-app/src/lib.rs:23399-23400, 23465-23485` |
| B6 | **`ProviderRouter` can never return model output.** `route_completion`'s prompt is only metadata reference IDs and its response carries only a fingerprint + byte count; completion text is dropped by construction, so every router-backed flow fabricates its result. | `crates/legion-ai/src/lib.rs:727-761`; `crates/legion-app/src/lib.rs:21413-21418` |
| B7 | **BYOK keys entered in the IDE never reach any provider.** Desktop `SetProviderApiKey` writes the key to the OS keyring, but no product code ever calls `SecretStore::load`; every provider reads credentials only from environment variables. The key-entry flow is write-only. | `crates/legion-desktop/src/workflow.rs:974-1023`; `crates/legion-ai-providers/src/lib.rs:619-632` |
| B8 | **No working UI path to activate a real provider.** The view never emits `SetProviderApiKey` (bridge maps it to Noop) and never emits `StartDelegatedTask` (the *one* command that uses a real Anthropic client). The hosted tier is always denied; the only "Available" provider fabricates `deterministic-answer:<hash>`. | `crates/legion-desktop/src/bridge.rs:2147-2148`; `crates/legion-storage/src/secrets.rs:67`; `crates/legion-ai-providers/src/lib.rs:103-148` |

The important nuance (from the verifiers): **real, working model adapters exist** — native Anthropic messages/streaming/batch, OpenAI Responses, OpenAI-compatible, Ollama, llama.cpp — and the delegated-task agent loop (`StartDelegatedTask` → `run_delegated_task_loop`) genuinely calls Anthropic with real tools, budgets, sandboxing, and proposals. The problem is purely **wiring**: no user-facing surface constructs `StartDelegatedTask`, and every button the user *can* press routes to the deterministic fake. This is fixable without new model code, but as shipped the AI is a facade.

### 3.3 Debugger runs no debugger

| # | Blocker | Evidence |
| --- | --- | --- |
| B9 | **The entire DAP client is a simulation.** `DapClientRuntime::launch/step` spawns no adapter and speaks no DAP wire protocol; it fabricates a `"main"` stack frame, a `"count"` variable, verified breakpoints, and canned console lines. `step()` returns frozen sequence numbers; there is no stop/terminate, so sessions never end. Fully wired into `legion-app`'s `DebugWorkflow` and projected into the IDE's variables/stack panes as if real. | `crates/legion-debug/src/dap.rs:1-6, 122-226`; `crates/legion-app/src/lib.rs:7004-7107` |

A real DAP codec exists in `legion-protocol` but is exercised only by tests with scripted in-memory adapters. `legion-terminal` even contains a near-identical `DapAdapterFixtureRuntime` — a fixture apparently promoted to the "production" path.

### 3.4 Terminal is launch-only

| # | Blocker | Evidence |
| --- | --- | --- |
| B10 | **The terminal cannot receive input or stream output in the GUI.** The only production dispatch is a "Run cargo test" button emitting `TerminalLaunch`; no view code, keybinding, palette command, or frame tick ever emits `TerminalInput`/`OutputPoll`/`Resize`/`Close`/`Kill`. Output is limited to the ~50 ms spawn-time PTY read. | `crates/legion-desktop/src/view.rs:3039-3043`; `crates/legion-desktop/src/bridge.rs:798, 2134` |
| B11 | **Every terminal session is hard-killed by a non-refreshing 30 s deadline.** The wall-clock deadline is set once at launch and never extended by input/activity; `timeout_seconds=0` is denied, so unbounded interactive sessions are impossible. Desktop passes `None`, yielding the 30 s default. | `crates/legion-terminal/src/lib.rs:629-630, 801-815`; `crates/legion-app/src/lib.rs:6015` |

The underlying `TerminalRuntime` (real Unix `openpty` + Windows ConPTY, OSC 7/133 parsing, redaction) is genuinely implemented — it is the *UI wiring* and the deadline policy that block it. Separately, the terminal is a line/row projection, not an emulator: raw ANSI escapes reach the renderer, so colored output and TUIs (vim, htop, less) cannot work (major, §5).

### 3.5 Packaging, signing, and updates do not exist

| # | Blocker | Evidence |
| --- | --- | --- |
| B12 | **No installer/package artifacts are ever produced.** `plan_release_pipeline` only records `build_command` strings into descriptor TOMLs; nothing executes them. There is no `cargo-dist` config, no release CI workflow, and `package.rs` states "This intentionally does not create an installer." `.dmg`/`.msi`/`.deb`/`.rpm`/AppImage exist only as descriptor metadata. | `xtask/src/release_pipeline.rs:251-298`; `crates/legion-desktop/src/package.rs:118-122`; `scripts/package-windows.ps1:36-43` |
| B13 | **No OS code signing or notarization.** Only the *update manifest* is Ed25519-signed. There is no Apple `codesign`/notarize or Windows Authenticode path anywhere; the KMS signer is "not yet implemented — honest unavailable," and any signer absence silently degrades to `unsigned-beta` as a first-class success. Descriptor `verification_command` entries (`spctl`, `rpm --checksig`) are never executed. | `xtask/src/release_pipeline.rs:20-22, 495-499`; `xtask/src/signing.rs:162-164`; `crates/legion-desktop/src/health.rs:374-375` |

The auto-updater client (Ed25519-verify-before-parse, staging, journal, rollback) is real and well-tested, but it has only `LocalDirManifestSource` — the HTTP source is "explicitly deferred — no update server currently exists" — and no product binary references it (only the `update-drill` test binary does). So even if installers existed, there is no channel to update them. (Blocker for release; see B14 persistence note below.)

### 3.6 No persistence, no file watching, shallow file tree

| # | Blocker | Evidence |
| --- | --- | --- |
| B14 | **Product storage is memory-only.** `legion-app` composes `InMemoryStorageRepositoryPort` (documented "Test-oriented"); the durable `FileBackedStorage` is only opened-and-dropped by a CLI diagnostic. Trust decisions, all proposal/AI/terminal/debug audit records, plan revisions, breakpoints, and semantic metadata are lost on every restart. | `crates/legion-app/src/lib.rs:14282`; `crates/legion-storage/src/lib.rs:530` |
| B15 | **All runtime audit events are discarded.** The shipped composition wires `NoopEventSink` ("wiring is intentionally deferred"); no persistent `EventSinkPort` implementation exists anywhere. Every proposal/security/AI audit event with `RetentionLabel::Audit` vanishes — there is no durable audit log despite the extensive audit machinery, undercutting the product's "metadata-only evidence" pitch. | `crates/legion-observability/src/lib.rs:258, 357-364`; `crates/legion-app/src/lib.rs:14255` |
| B16 | **No real file watcher.** `NativeWatcherService` does a one-shot, non-recursive `read_dir` of the workspace root; no `notify`/inotify/FSEvents dependency exists anywhere. External edits/creates/deletes below the root's immediate children are undetectable, and rust-analyzer's own watcher is disabled (`files.watcher:"client"`) without Legion ever sending `didChangeWatchedFiles`. | `crates/legion-platform/src/lib.rs:620-627, 2057-2093`; `crates/legion-project/src/lib.rs:4547-4650` |
| B17 | **File tree hard-capped at depth 2.** `MAX_TREE_CHILDREN_DEPTH=2` stops the scan, so files more than three levels below root (e.g. `crates/x/src/lib.rs`) never enter the explorer *or* the quick-open palette *or* the full-text search index. No deepening path exists (`ApplyTreeDelta` is never sent). | `crates/legion-project/src/lib.rs:70, 3970-3972`; `crates/legion-app/src/lib.rs:9030, 16148` |

B16 also has a data-loss corollary (major, §5): the shallow watcher poll clobbers `last_scan`, which breaks rollback of nested-file mutations after any workspace search.

### 3.7 Plugin system executes no plugin code

| # | Blocker | Evidence |
| --- | --- | --- |
| B18 | **"Loading" a plugin runs nothing.** Product binaries wire only `PluginRuntimeHost`, whose `dispatch_host_call` validates metadata and echoes the caller-supplied label; no WASM is loaded or run. A genuine wasmtime executor (`WasmPluginHost`) exists but is fixture/test-only, unwired, behind no feature flag, and cannot even link its one advertised host import (`env::host_log`). Desktop shows "Plugin N loaded" for plugins that can never act. | `crates/legion-plugin/src/lib.rs:135-208`; `crates/legion-app/src/lib.rs:17478-17517`; `crates/legion-plugin/src/host.rs:104-131` |

### 3.8 Sandbox does not enforce what the UI claims

The delegated-task agent loop (default `ai` feature, on) runs `terminal-command` tool calls through `spawn_sandboxed`. Confirmed enforcement holes:

| # | Blocker | Evidence |
| --- | --- | --- |
| B19 | **Windows sandbox enforces only process lifetime.** `spawn_sandboxed` on Windows uses a `KILL_ON_JOB_CLOSE` job object; filesystem-read, filesystem-write, and network enforcement are all `false`. `RestrictedToken`/`AppContainer` are enum variants + prose only. | `crates/legion-sandbox/src/spawn.rs:414-421, 740-748`; `crates/legion-sandbox/src/windows.rs:22-43` |
| B20 | **Linux network egress is not enforced at all.** The spec promises "empty = no network," but Linux applies only Landlock *filesystem-write* rules; `allowed_egress` is ignored and `network_enforced` is hardcoded `false`. The product wires empty egress *expecting* no network, so sandboxed agent commands get unrestricted network + unrestricted reads — an exfiltration path. | `crates/legion-sandbox/src/spawn.rs:31, 272-276`; `crates/legion-app/src/lib.rs:18264` |

Both are acknowledged in `docs/SECURITY.md` (so they are known, not hidden), but they are release blockers for any product that offers delegated/agentic execution as a headline feature. The honest `SandboxEnforcementReport` is built per-spawn and then **discarded** by `legion-app`, while the desktop panel shows static compile-time claims that contradict runtime reality (major, §5).

### 3.9 LSP go-to-definition (cross-file) is a no-op

| # | Blocker→Major (verifier-adjusted) | Evidence |
| --- | --- | --- |
| B21 | **Cross-file go-to-definition never opens a file.** LSP location projections drop the target URI (`path:None`, no backfill), and the LSP response overwrites the semantic-index definitions that *would* navigate. Same-file go-to-def works via the semantic index; cross-file silently fails. Verifiers downgraded this from blocker to **major** because same-file navigation functions. | `crates/legion-lsp/src/lib.rs:1897-1902, 5427`; `crates/legion-desktop/src/workflow.rs:1470-1472` |

---

## 4. Planned features that are missing or non-functional

These are features Legion's docs describe as part of the product. Most are **deferred with an explicit cut line** in `plans/product-readiness-ledger.md` — meaning the team already treats them as not-done — but they are "planned features that are missing or non-functional," so they belong in the answer. They are not undocumented surprises like §3; the risk here is shipping/marketing the product as if these work.

| Feature | State | Evidence |
| --- | --- | --- |
| **Multi-user collaboration** | Non-functional. The OT operation-log crate is real and convergence-tested, but there is **no network transport anywhere**, no API to admit a second participant, the runtime gate is enabled only from tests, and the app seeds sessions with empty text so any real edit conflicts. `:collab-join` always fails "disabled by policy" in shipped binaries. Deferred (`PR-ENT-002`). | `crates/legion-collaboration/src/lib.rs:117-185`; `crates/legion-app/src/lib.rs:17519-17593` |
| **Remote development (SSH/devcontainer)** | Non-functional. `plan_ssh_session` builds a descriptor; no SSH client/socket code exists. Remote file state comes from `seed_remote_fixture_file`. Devcontainer support is `devcontainer.json` label parsing only. The desktop reports "Remote workspace connected" for a fixture. Deferred (`PR-ENT-001`). | `crates/legion-remote/src/lib.rs:486-497, 1083-1125`; `crates/legion-app/src/lib.rs:17749-17817` |
| **Local-model / offline AI** | Non-functional as a "local AI" story. The `offline` build replaces the AI edge with disabled stubs (`offline-ai-disabled`, empty registry). Local inference exists only via Ollama/llama.cpp adapters that need an operator-run server **and** a self-supplied model. No `.gguf`/safetensors artifact ships or is producible: `qlora_train.py` has no training loop (import-checks deps + writes a manifest), and `download-models.sh` output doesn't match the paths `workers.example.yaml` expects. | `crates/legion-app/src/offline_ai.rs:1-5`; `training/qlora_train.py:125-171`; `scripts/models/download-models.sh:60-74` |
| **VS Code extension compatibility** | Metadata-only *and unwired*. `legion-vscode-compat` classifies `package.json`/Open VSX metadata into tiers, but **no crate depends on it** — the beta e2e test hand-builds the DTOs instead. Nothing fetches Open VSX or extracts a VSIX; no extension-host process exists. Runtime execution deferred (`PR-VSC-002`); manifest ingestion (`PR-VSC-001`) is marked "substrate validated" but is unreachable from any binary. | `crates/legion-vscode-compat/src/lib.rs:63-136, 207-242`; `Cargo.toml:102` |
| **Hosted telemetry** | Unwired. `legion-telemetry` has a real rustls exporter and durable spool, but **no crate depends on it**; suggestion-telemetry records built elsewhere are never spooled. Consent expiry is never enforced; no air-gap/endpoint-policy check; blocking single-shot export only. The master plan itself labels it "Bronze (fixture/stub)." | `Cargo.toml:108`; `crates/legion-telemetry/src/lib.rs:281, 585-621` |
| **Encrypted raw-source retention** | Unwired. `legion-retention` implements real ChaCha20-Poly1305 vault, key rotation, TTL purge, tombstoned deletion — but **no crate depends on it**, and the privacy-inspector deletion UI it documents does not exist (`privacy_inspector.rs` is absent). | `Cargo.toml:109`; `crates/legion-retention/src/privacy.rs:54-59` |
| **Production remote transport (mTLS)** | Unwired dead code. `legion-remote-transport` has real rustls mTLS + a replay/flow-control state machine, but **no crate imports it**, the carrier drops the TLS stream after handshake (returns diagnostics only), and nothing binds the state machine to IO. | `crates/legion-remote-transport/src/lib.rs:141-142, 297-349`; `Cargo.toml:105` |
| **Multi-language LSP** | Rust-only. Only `rust-analyzer` is ever launched (hardcoded `LanguageServerId(1)`). The tier-2 registry (TypeScript, pyright, gopls, tailwind) is test-only; `DownloadedArtifact` servers have placeholder URIs (`registry.example.invalid`) and no download path. rust-analyzer itself has no product fetch path (download is "decision + verify," live fetch only in an `#[ignore]` smoke). | `crates/legion-lsp/src/lib.rs:611-665`; `crates/legion-app/src/language/download.rs:3-6` |
| **LSP write features (format, organize imports, code actions)** | No-op previews. `run_language_proposal` builds a self-identical whole-buffer edit with the warning "represented as a safe no-op preview until live LSP edits are wired." Only rename has a live path. Completion acceptance ignores `textEdit` and duplicates the typed prefix (`pri`+`println!`→`priprintln!`). Diagnostic messages are redacted to generic strings ("LSP error diagnostic") so the Problems panel never shows the real compiler error. | `crates/legion-app/src/lib.rs:24404-24459`; `crates/legion-lsp/src/lib.rs:1994, 2092-2099` |
| **MCP client** | Non-functional in product. Transports are never constructed by any binary; the client skips the mandatory `initialize` handshake; the streamable-HTTP transport is not spec-conformant (no SSE/session headers). MCP passthrough is advertised to the model but the production tool host stub-errors it, terminating the run. | `crates/legion-ai-providers/src/lib.rs:2662-2726`; `crates/legion-app/src/lib.rs:1186-1193, 18845-18855` |
| **Opt-in long-term memory** | Permanent no-op. `legion-app` only ever proposes candidates with hardcoded `NotGranted` consent and never calls `retain`/`delete`; the persisted snapshot is always empty. No recall/retrieval path, no embeddings ("vector.deferred"). | `crates/legion-app/src/lib.rs:20379, 21555`; `crates/legion-memory/src/lib.rs:313` |
| **Whole-repo semantic index / AI retrieval** | Current-file-scoped. `IndexingActor`/`SemanticFabricScheduler`/`RepositoryDiscoveryImporter` are test-only; `legion-app` indexes only opened files synchronously, and delegate-chat retrieval filters to the active buffer, so citations can never come from other files. "Embeddings" are hash-bucketed token counts, not model vectors. | `crates/legion-index/src/lib.rs:284, 4997-5016`; `crates/legion-app/src/lib.rs:5286, 23345-23350` |
| **Crash reporting** | Consent toggle captures nothing. `install_panic_hook` is called only from tests; the desktop "Crash reports" checkbox just flips a boolean. Minidump support hardcodes `minidump_captured=true` with no writer. Native (non-panic) crashes are uncaptured. | `crates/legion-observability/src/crash_capture.rs:63`; `crates/legion-observability/src/minidump.rs:26-50` |
| **AI response streaming** | Absent. `complete`/`complete_with_tools` are blocking; `streaming.rs` only re-splits assembled markdown. Anthropic "streaming" buffers the whole SSE body. Assistant responses can only render all-at-once. | `crates/legion-ai/src/lib.rs:349-356`; `crates/legion-ai-providers/src/lib.rs:1919-1949` |

---

## 5. Notable majors (real code, broken or unwired behavior)

Selected from ~146 major findings; these have the clearest user-facing or trust impact. Full evidence in the appendix data (`audit-reports/…` companion JSON is not committed; see PR description).

- **Fabricated evidence in the agent path.** The workflow coordinator mints `CommandRun` audit records with `exit_status: Some(0)` for commands that never ran, and labels literal strings (`"legion-evidence-hash:<id>"`, `"legion-fingerprint:<target>"`) as `sha256`. A real Sha256 helper exists and is bypassed. This taints the exported evidence bundle — directly against the product's "metadata-only evidence" positioning. `crates/legion-agent/src/lib.rs:362-380, 499-514`.
- **Agent loop is brittle by construction.** Only `InvalidArguments` is retryable; a read of a nonexistent path, a command timeout, or any host error maps to `RuntimeFailure` → `Blocked`, **discarding all accumulated proposals**. The loop also advertises all 7 tools to the model regardless of scope, and one call to an out-of-scope tool is non-retryable `ScopeDenied` → whole run dies. `crates/legion-agent/src/agent_loop.rs:139-148, 275-282, 995-997`.
- **Sandbox UI lies.** `AppDelegatedToolHost` drops the honest `SandboxEnforcementReport`; the desktop panel shows static prose ("filesystem scope limited to workspace root", "process-isolated") that contradicts runtime enforcement (esp. on Windows where nothing is enforced). `crates/legion-app/src/lib.rs:1172-1183`; `crates/legion-desktop/src/view/sandbox_panel.rs:144-222`.
- **Desktop renders fabricated data as real.** Hardcoded "Context Packs" list; "AI Assistance" toggles hardwired on and non-interactive; resource-usage strip synthesized from a per-mode constant (12/28/42/82); agent cards render fixed 0.55/0.72 progress bars. `crates/legion-desktop/src/view.rs:1716-1818, 4494-4502`.
- **Async LSP results stall.** `refresh_projection` runs only inside `handle_action`; `render_app_frame` has no unconditional refresh, so completions/hover/diagnostics arriving while the user is idle never appear until the next keystroke/click. `crates/legion-desktop/src/workflow.rs:2628-2648`.
- **No UI trigger for debug, refactor, go-to-def, or hunk staging.** These bridge actions exist but are emitted only by integration tests — no view, keybinding, or palette entry. `crates/legion-desktop/src/bridge.rs:257-301, 686-700`.
- **Terminal is not an emulator; cwd ignored.** Raw ANSI/SGR passes through as literal bytes; no cell grid or alternate screen. `cwd_policy` is a label — the PTY always spawns with `cwd: None`, so the terminal opens in the IDE process cwd, not the workspace root. `crates/legion-terminal/src/osc.rs:145-162`, `:617-624`.
- **Unguarded O(m·n) diff can OOM.** `compute_line_diff` allocates a full `(m+1)×(n+1)` table with no size cap; an AI proposal rewriting a ~50k-line file needs ~10 GB, hanging/OOMing proposal review. `crates/legion-editor/src/diff.rs:190-207`.
- **Code folding is unrepresentable end-to-end.** `ViewportFoldRange`/`ViewportDecorationSpan` are fieldless placeholder structs; `legion-editor` always emits empty vecs; the desktop folding UI permanently shows "0 ranges." `crates/legion-protocol/src/lib.rs:527-533, 647-653`.
- **Protocol validators forbid recording real activity.** `delegated_task_audit_linkage_record` and the assisted-AI audit validator reject any `runtime_activation`/`runtime_invocation_state` other than `NotEncoded` — yet the shipped agent really executes and providers really make HTTP calls. Persisted audit records therefore systematically assert "no activity occurred." `crates/legion-protocol/src/lib.rs:10856-10861, 11579-11588`.
- **`legion-cli` fails against its own repo.** The default `doctor` command and four others require `.github/workflows/ci.yml` (renamed away) and stale ledger markers; the only passing GA gate works via `legion-`→`devil-` name aliasing on stale artifacts. Confirmed by execution. `crates/legion-cli/src/main.rs:448, 1178-1187`.
- **Hardened `ProcessService` is dead code.** `legion-platform`'s timeout/group-kill/secret-stripping process runner has zero consumers; ~26 files spawn via raw `std::process::Command`, bypassing the tested hardening. `crates/legion-platform/src/lib.rs:994-1112`.
- **`OrgPolicyBundle` "signature" is a label.** The enterprise admin-policy struct's `signature_label` is a free-form string with no crypto verification and no loader/wiring anywhere. `crates/legion-security/src/lib.rs:936-1010`.
- **`legion-security` path-scope sibling-prefix bypass.** `path_scope_risk` uses `starts_with` without a separator check, so root `/repo` classifies `/repo-evil/file` as in-scope and can auto-approve it. `PathPolicy` fixed this class; `risk.rs` was not. `crates/legion-security/src/risk.rs:80-84`.

---

## 6. Documentation / truth-drift findings

The planning docs are, on balance, unusually honest — the ledger's own gate rules state that substrate acceptance ≠ product readiness. But this audit found specific places where a doc claims more than the code delivers:

1. **`PR-VSC-001` overclaims.** The ledger says "activation-event routing, enable/disable/update metadata, API coverage reporting … are implemented." The crate only *classifies* activation events (no routing), and no enable/disable/update-metadata or API-coverage types exist in the crate or protocol. It is also unreachable from any binary. `plans/product-readiness-ledger.md:55`.
2. **Cloud Lane "Accepted" evidence overstates productization.** `HttpLegionCloudLaneTransport` is constructed only in tests; `submit_task` fabricates a `Submitted` status with no network call. No product surface reaches it. `crates/legion-app/src/lib.rs:13805-13828`.
3. **`legion-bench` "recorded" scores are fabricated.** `score_task` hardcodes `tests_passed=true` and derives diff/turns/cost arithmetically from budgets; no fixture repo is opened, no agent runs. The four cited fixture repos don't exist (only `fixtures/gp1-rust` does). The weekly `legion-bench.yml` CI "verifies" this. `xtask/src/legion_bench.rs:360-418`.
4. **Perf gates are synthetic and non-blocking.** The input-to-paint/line-galley skeletons measure a byte-mutation loop and a HashMap lookup, not `legion-editor`. CI sets `LEGION_PERF_FAIL_ON_BUDGET_MS=0`, reclassifying budget failures as report-only, so no perf regression can fail PR CI. `xtask/src/perf_harness.rs:1-28`; `.github/workflows/legion-gates.yml:93-101`.
5. **Phase-8 evidence cites a nonexistent workflow.** `check-deps` and `legion-cli` evidence checks require the marker `Workflow: .github/workflows/ci.yml`, which no longer exists; the recorded Run URL points at a *different* repo (`9thLevelSoftware/devil-ide`). `plans/evidence/phase-8/platform-matrix-evidence.txt:3-4`.
6. **`HERMESGOAL-GAP-ANALYSIS.md` already flagged (2026-07-01):** missing `claim-audit` script, the dogfood-path drift, the Kanban backlog lacking a status field, orphaned delegated-task sandbox copies under `crates/legion-app/target/delegated-tasks/`, and README's stale "no CI configured" claim. Those remain valid and overlap this audit.

The pattern across 1–5: **CI is green because the gates check descriptor byte-integrity, prose markers, and self-consistent synthetic reports — not real behavior.** This is why the substrate can be "accepted" while the product is far from shippable, and it is the highest-leverage thing to fix, because it is what allows the other gaps to persist unnoticed.

---

## 7. What actually works (so the report is balanced)

Real, wired, and covered by tests:

- **Text substrate** (`legion-text`): rope buffers, snapshots, UTF-8/16 conversions, 100 MB degraded mode, binary detection — complete.
- **Editor engine core** (`legion-editor`): atomic transactions, undo/redo stacks, retention/eviction — solid (the *grouping* and overlay/fold projections are the gaps).
- **Syntax highlighting**: `syntect` across the Sublime default set (Rust, C/C++, Python, JS, HTML, CSS, Markdown, …), per-extension, visible-range only. (Tree-sitter is Rust-only.)
- **Git**: real `git` CLI + `gix` shell-out — status, diff hunks, blame, stage/unstage, commit, branch, stash, worktrees, conflicts. (Gaps: no pull/fetch wired to UI; `gix` backend is a CLI facade; no subprocess timeout.)
- **rust-analyzer LSP lifecycle**: real stdio launch, framing with size caps, correlation/timeout/cancel, restart backoff/circuit-breaker; completion, hover, same-file definition, rename-as-proposal, publishDiagnostics wired to the UI.
- **PTY backends** (`legion-platform`): genuine Unix `openpty`/`setsid` and Windows ConPTY with quoting/env/kill-tree.
- **Security policy engine** (`legion-security`): deny-by-default broker across 14 policy families, graduated approval ladder, redaction scanner — 96 tests. (Two flaws noted in §5.)
- **Delegated-task agent loop** (`legion-agent`): real tool execution, git-worktree sandbox with lease-based reaping and careful TOCTOU handling, containment validation — genuinely strong (marred by the evidence-fabrication and brittleness issues in §5, and blocked from users by wiring).
- **Settings** (theme, zoom, fonts, consent, shell) persist and restore; **session restore** survives crashes (metadata-only — unsaved buffer content is intentionally dropped).
- **Scale engineering**: documented, real limits (chunk budgets, degraded mode, 256 KB search cap, 512 KB diff cap, Tantivy index).

---

## 8. Prioritized path to a shippable, fully-functional IDE

Ordered by "cannot ship without" first. This mirrors the repo's own doctrine (truth first, then the manual daily-driver bar, then outward).

**Tier 0 — Truth & gates (days).** Make CI test behavior, not markers. Replace the synthetic `legion-bench` scorer and report-only perf gates with real (even if small) executed workloads; fix the `legion-cli`/`check-deps` stale markers and the phase-8 evidence pointing at a nonexistent workflow/foreign repo; correct the `PR-VSC-001` and Cloud Lane overclaims in the ledger. Without this, every later fix can regress invisibly.

**Tier 1 — Make the editor usable (the GA floor).**
1. Wire Backspace/Delete/Enter and real OS clipboard copy/cut (B1, B2).
2. Deepen the file tree beyond depth 2 and index deep files for explorer/quick-open/search (B17).
3. Replace the one-shot watcher with a real `notify`-based recursive watcher and fix the `last_scan`-clobber rollback bug (B16).
4. Persist storage and audit events to disk (`FileBackedStorage` + a real `EventSinkPort`) (B14, B15).
5. Make the terminal interactive (wire input/poll/resize/close, refresh the deadline on activity, add ANSI rendering) (B10, B11, §5).

**Tier 2 — Make the AI real (the product thesis).**
6. Wire a user-facing path to `StartDelegatedTask` and load BYOK keys from the keyring into providers; route assist/chat/inline through real adapters instead of the deterministic fake (B3–B8).
7. Add token streaming to the provider abstraction and UI.
8. Fix the agent loop's non-retryable failure/scope behavior and stop fabricating evidence records (§5).
9. Enforce the sandbox the UI advertises — Windows filesystem/network, Linux egress — or narrow the advertised guarantees to what is enforced (B19, B20).

**Tier 3 — Debugger & language breadth.**
10. Implement a real DAP client (adapter launch + wire protocol) or ship an explicit, user-visible "debugger not available in this build" cut line instead of a simulated one (B9).
11. Wire LSP format/organize-imports/code-actions to real requests; fix cross-file go-to-def and completion `textEdit`; stop redacting diagnostic messages (B21, §4).

**Tier 4 — Packaging.**
12. Configure `cargo-dist`, build real installers for all three OSes, add a release CI workflow, and stand up code signing/notarization + a hosted update manifest server (B12, B13).

**Tier 5 — Planned features (decide: build, or mark deferred in-product).** Collaboration, remote dev, VS Code extension runtime, local-model AI, hosted telemetry, raw-source retention. These are documented cut lines; the requirement for a *fully-functional* release is that the product must not present them as working. Either finish them or gate them behind honest "coming soon / not in this build" UI (several currently show "connected"/"loaded"/"verified" for fixtures — that must not ship).

---

## 9. Coverage & confidence

- **Coverage:** all 29 crates + `xtask`, the Python/model harness, packaging/CI, and 5 documentation sets were audited. 32 area reports + 5 doc reports + a 20-point IDE baseline check were produced.
- **Findings:** 29 blockers, 146 majors, 107 minors after cross-crate dedupe.
- **Verification:** the 40 highest-severity findings were re-checked by independent adversarial verifiers (blocker items with an "implementation-hunt" lens explicitly trying to *refute* by finding a real, wired implementation elsewhere). Result: **0 refuted, 33 confirmed, 7 partial** (all 7 partials were real but had severity or scope adjusted — e.g. same-file go-to-def works, or a gap is a documented deferral rather than a silent break). The remaining ~135 majors are reported unverified; they carry `file:line` evidence but were not independently re-checked, so treat their exact severity as provisional.
- **Not done:** tests were not executed (environment `libdbus` build failure), so this is a static + wiring audit. Runtime behavior claims are inferred from source; the Windows ConPTY path in particular cannot be compiled/verified on the Linux checkout.
