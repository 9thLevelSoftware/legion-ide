# Design Exploration — Legion IDE Production Completion

## Initial Ask

Construct a final, comprehensive project to take Legion IDE from its current state (~55–60% complete) to 100% production completion across all features and functionalities. The project uses a fresh plan structure (not inheriting the v0.2 milestone hierarchy), attacks all fronts in parallel, and focuses on the local IDE + AI product — deferring VS Code extension host, remote development, and collaboration/admin controls to post-v1.

## Research Summary

### Facts
- 30-crate Rust workspace, ~196k LOC, 374 Rust source files, 193 test files
- 6 features fully functional: text engine, editor engine, desktop GUI, workspace/VFS, git integration, semantic indexing
- 11 features partially functional: terminal, debugger, AI providers, agent workflows, collaboration OT, remote, updater, telemetry, retention, AI orchestration, search UX
- 1 feature scaffold-only: plugin WASM execution
- Product readiness ledger: 1/11 gates at "product workflow validated" (PR-AI-001), 6 substrate-validated, 1 in-progress, 3 deferred
- 45 ADRs accepted covering all architecture decisions
- 12 milestones of accepted evidence (M0–M12)
- 21 standing gates run via xtask on every PR
- 3-OS CI matrix (ubuntu/windows/macos) for gates, smoke, bench, preview
- v0.2 master plan identifies evidence drift as the primary risk
- Current Rust edition 2024, MSRV 1.92, egui/eframe 0.34, tree-sitter 0.26, wasmtime 46, tantivy 0.26
- WASI 0.3.0 (Feb 2026) added native async I/O — relevant for plugin host ABI
- Cross-platform PTY libraries (portable-pty, rust-pty) provide production ConPTY + Unix PTY

### Inferences
- Core IDE foundations are solid — the "last mile" is wiring production runtimes as defaults and building product UX
- The dual-mode (fixture/production) architecture means most features have production code that just needs to become the default path
- AI provider adapters have real HTTP/SSE code — the gap is product UX (credential management, provider selection, cost visibility)
- WASM plugin execution is the largest greenfield work item (wasmtime host ABI, component model, WIT interfaces)
- Cross-platform parity requires systematic 3-OS testing infrastructure, not just code
- The security model is a genuine differentiator — proposal-mediated mutation, metadata-only egress, honest enforcement reporting

### Assumptions
- VS Code extension host sidecar, remote dev (SSH/containers), and collaboration/admin controls are explicitly deferred to post-v1
- All 6 implemented AI providers (deterministic-local, Ollama, llama.cpp, OpenAI, OpenAI-compatible, Anthropic) get full product UX
- Full WASM plugin execution (not just metadata validation) is in scope
- Terminal and debugger ship with user-selectable mode (production-default, fixture available)
- Full 3-OS parity required — no platform-specific feature gaps
- Quality-driven timeline — ship when every gate passes, not before

## Product Definition

### Target users
- Security-conscious enterprise teams needing local/self-hosted AI with audit trails
- Senior engineers wanting agent leverage with real diff, test, terminal, LSP, debugger, and git workflows
- Platform/tooling teams needing policy-controlled provider/tool/agent integration
- AI-forward teams wanting multi-agent orchestration with reproducibility, review, and cost control

### Primary outcome
A native, control-first IDE that is daily-drivable for professional Rust development with integrated, inspectable AI assistance across manual → assist → delegate → automate modes — production-quality on all three platforms.

### Value proposition
The most trustworthy native control surface for manual-to-autonomous development: proposal-mediated mutation, metadata-first evidence, default-deny capabilities, air-gap/local-first options, context manifests, privacy inspector, and auditable decision surfaces.

### Non-goals for v1
- VS Code extension host sidecar / runtime extension execution
- Remote development (SSH, devcontainers, cloud workspaces)
- Real-time collaboration / shared editing
- Admin policy management console
- Marketplace / extension store
- Proprietary next-edit prediction models
- Multi-language LSP parity (Rust-first; other languages via LSP config)

## Recommended Approach

### Parallel Wave Architecture

Organize work into 7 parallel waves + 1 integration gate, each targeting a domain. Waves execute concurrently with explicit interface contracts between them. Each wave has its own acceptance criteria and can ship independently.

**Why this approach:** The codebase already has clean domain boundaries (hexagonal ports, separate crates). The dual-mode architecture means most production paths exist — they need wiring, testing, and UX. Parallel execution maximizes throughput because the domains have minimal cross-dependencies after the protocol layer.

**Risk mitigation:** Each wave has a "production gate" — a golden-path smoke test that exercises the full end-to-end flow. Cross-wave integration is validated by a final integration gate that runs the full beta acceptance scenario.

## Alternatives Considered

| Approach | Strengths | Tradeoffs | Decision |
|----------|-----------|-----------|----------|
| Sequential phases (Manual → AI → Release) | Lower coordination cost, predictable | Slow — 12+ months for full completion | Rejected — user wants parallel |
| Feature-flag progressive activation | Ship early, activate incrementally | Complex flag management, testing matrix explosion | Elements adopted within waves |
| Monolithic sprint | Simple planning, single deadline | All-or-nothing delivery risk | Rejected — too brittle at this scale |
| **Parallel waves with integration gate** | **Maximum throughput, domain isolation, independent shipping** | **Coordination overhead, interface contracts needed** | **Selected** |

## Feature Scope

### Wave 1 — Terminal & Process Runtime (Production PTY)
- [ ] Wire `NativePtyService` as default terminal runtime on all 3 platforms
- [ ] Implement real ConPTY on Windows, Unix PTY on macOS/Linux
- [ ] Shell detection and configuration (bash/zsh/powershell/cmd)
- [ ] Terminal resize, scrollback, ANSI/VT100 rendering in egui
- [ ] Output redaction in production mode (credentials, API keys)
- [ ] Process cleanup on session close / app exit
- [ ] User-selectable mode toggle (production/fixture) in settings
- [ ] 3-OS PTY integration tests with real shell processes
- [ ] GP-1 terminal step passes with real PTY on all 3 OSes
- [ ] Performance: terminal output throughput benchmark

### Wave 2 — Debug & Test Runtime (Production DAP)
- [ ] Wire `LiveDapSession` as default debug path
- [ ] Zero-config Rust debug: auto-discover lldb-dap/codelldb/rust-gdb
- [ ] Breakpoint set/hit/clear with real adapter
- [ ] Variables/watch/call stack inspection
- [ ] Step over/in/out/continue with keyboard shortcuts (F5/F9/F10/F11)
- [ ] Debug console (stdin/stdout to adapter)
- [ ] Test explorer: cargo test --list discovery, per-item --exact run
- [ ] Test explorer: collapsible tree widget in desktop UI
- [ ] Test results → agent evidence attachment
- [ ] User-selectable mode toggle (live/fixture) in settings
- [ ] 3-OS debug integration tests (lldb-dap on macOS/Linux, codelldb/msvc on Windows)

### Wave 3 — AI Provider Product UX
- [ ] Provider selection UI in desktop settings panel
- [ ] Credential management UX: keyring integration for API keys per provider
- [ ] Provider health/status indicators in status bar
- [ ] Real inline prediction from selected provider (not deterministic-local)
- [ ] Chat/assist panel with streaming responses from real providers
- [ ] Context manifest inspector: show exactly what goes to the model
- [ ] Cost/token usage display per request and cumulative
- [ ] Provider-specific configuration (base URL, model selection, temperature)
- [ ] Ollama auto-detection (local loopback)
- [ ] llama-cpp server auto-detection
- [ ] OpenAI / OpenAI-compatible endpoint configuration
- [ ] Anthropic Messages API with SSE streaming in desktop
- [ ] MCP tool integration in assist panel
- [ ] Cancellation UX (abort in-flight requests)
- [ ] Integration tests against each provider with recorded/replay transport
- [ ] 3-OS provider smoke tests

### Wave 4 — Agent Workflow Command Center
- [ ] Wire `LegionWorkflowCoordinator` into desktop runtime as production path
- [ ] Plan editor: create/edit/approve workflow plans in desktop UI
- [ ] Fleet board: Kanban view of active workers with real status
- [ ] Worker panel: per-worker output, tool calls, evidence
- [ ] Sandbox panel: real sandbox activation with enforcement report
- [ ] Scope picker: interactive scope selection for delegated tasks
- [ ] Budget controls: token/cost limits per worker and per workflow
- [ ] Kill switch: immediate worker termination
- [ ] Risk strip: real-time risk assessment visualization
- [ ] Proposal review: multi-file diff from agent outputs
- [ ] Evidence bundle: export workflow evidence as TOML/JSON
- [ ] Replay: re-read workflow traces for post-mortem
- [ ] Merge readiness evaluation before apply
- [ ] GP-3 and GP-4 golden-path smokes pass with real agent loop
- [ ] Integration tests for coordinator → scheduler → worker → evidence pipeline

### Wave 5 — WASM Plugin Runtime
- [ ] Resolve wasmtime supply-chain ADR debt (W0.7 blocker)
- [ ] Implement `WasmPluginHost` with real wasmtime engine + WASI 0.3
- [ ] Define WIT interfaces for plugin host ABI (Phase 5 ABI v1)
- [ ] Host function implementations: fs_read (sandboxed), log, contribute_grammar, contribute_formatter, contribute_linter, contribute_command
- [ ] Plugin loading: validate manifest → compile WASM → instantiate with capability sandbox
- [ ] Plugin lifecycle: load/unload/reload, error isolation (plugin crash doesn't crash IDE)
- [ ] Grammar contribution: tree-sitter grammars via WASM plugin
- [ ] Formatter contribution: format-on-save via plugin
- [ ] Linter contribution: diagnostic overlay via plugin
- [ ] Command contribution: palette commands via plugin
- [ ] Quota enforcement: host call limits, output byte limits, execution timeouts
- [ ] Plugin management UI in desktop settings
- [ ] Hostile plugin tests (resource exhaustion, escape attempts, untrusted WASM)
- [ ] 3-OS WASM execution tests

### Wave 6 — Release Engineering & Distribution
- [ ] Signed installers: code signing for Windows (Authenticode), macOS (Developer ID), Linux (GPG)
- [ ] Native installers: MSI (Windows), DMG (macOS), deb/rpm (Linux)
- [ ] Auto-updater: HTTP manifest source, download, verify, stage, apply, rollback
- [ ] Update channel configuration (stable/preview)
- [ ] Crash reporting: native minidump capture, metadata-only upload (consent-gated)
- [ ] Telemetry: production spool → export pipeline (consent-gated, metadata-only)
- [ ] Retention vault: production activation of ChaCha20-Poly1305 encrypted storage
- [ ] First-run experience: workspace trust prompt, provider setup wizard, theme selection
- [ ] Fresh-VM install validation on all 3 OSes (Gatekeeper/SmartScreen/package manager)
- [ ] 3-OS CI: all gates merge-blocking, golden-path smokes merge-blocking
- [ ] Preview build pipeline: automated unsigned-beta → signed release promotion
- [ ] Version stamp, release notes, changelog generation

### Wave 7 — Product Polish & Cross-Platform Parity
- [ ] Accessibility: screen-reader projection, high contrast, focus management, keyboard-only navigation
- [ ] IME support: CJK input method composition
- [ ] Clipboard: full OS clipboard integration (copy/cut/paste with rich content)
- [ ] Multi-monitor: window position restore, DPI scaling
- [ ] Large workspace: 100K-file tree open <10s, 100MB file degraded mode, watcher burst debounce
- [ ] Search UX: search syntax (regex, glob, case), result navigation, preview
- [ ] Git UX: conflict resolution UI, blame gutter, diff navigation, commit history browser
- [ ] Editor polish: minimap, bracket matching, auto-indent, multiple cursors, code folding
- [ ] Theme: complete dark/light themes with all UI surfaces
- [ ] Keybindings: customizable keybindings, VS Code/JetBrains/Vim/Emacs presets
- [ ] Settings: unified settings UI covering all crate configurations
- [ ] Performance: startup <2s, input-to-paint p95 <16ms, memory <500MB for typical workspace
- [ ] Documentation: user guide, keyboard shortcut reference, provider setup guide
- [ ] Dogfood: weekly Legion-on-Legion development journal with evidence

### Integration Gate (after all waves converge)
- [ ] Full beta acceptance scenario passes end-to-end on all 3 OSes:
  Open large Rust repo → edit with LSP completion → run terminal cargo test → debug a failure → ask AI for multi-file change → inspect context manifest → review proposal diff → run tests on proposal → save safely → export audit evidence
- [ ] All standing gates pass on 3-OS CI (merge-blocking)
- [ ] All golden-path smokes (GP-1 through GP-4) pass on 3-OS CI
- [ ] Product readiness ledger: all non-deferred gates at "product workflow validated"
- [ ] Zero P0/P1 bugs in dogfood journal
- [ ] Signed installer installs and runs on fresh VM for each OS
- [ ] Performance budgets met on all 3 OSes

## Experience / Workflow

### Primary user flow (Manual → Assist → Delegate)
1. **Install:** Download signed installer for platform → run → first-run wizard (trust, provider, theme)
2. **Open workspace:** Select project folder → trust prompt → file tree loads, watcher starts, git status populates
3. **Edit:** Open Rust file → syntax highlighting (tree-sitter) → LSP completions/diagnostics → terminal cargo commands → debug with breakpoints
4. **Assist:** Toggle Assist mode → select provider → inline predictions appear → accept/dismiss → context manifest shows what model sees → cost counter updates
5. **Delegate:** Create task plan → approve scope → workers execute in sandboxed worktrees → fleet board shows progress → review proposals as diffs → apply or reject
6. **Ship:** Run tests → all green → commit → export evidence bundle

## Technical Direction

### Platform
- Rust native, cross-compiled for Windows (x86_64), macOS (x86_64 + aarch64), Linux (x86_64)
- egui/eframe 0.34 for desktop rendering
- Minimum Rust 1.92, edition 2024

### Architecture preserved
- Hexagonal port architecture (11 port traits in legion-protocol)
- `legion-protocol` — zero-behavior shared types
- `legion-app` — composition root, policy enforcement, authoritative state
- `legion-ui` — projection-only state and typed command intents
- `legion-desktop` — renderer/adapter edge, no product authority
- Security: `DenyByDefaultBroker`, proposal-mediated mutation, metadata-only egress

### Key dependencies for new work
- wasmtime 46 + WASI 0.3 for plugin execution (component model, WIT interfaces)
- Evaluate portable-pty / rust-pty vs. existing legion-platform PTY for cross-platform terminal
- ed25519-dalek (in tree) for release signing
- ChaCha20-Poly1305 (in tree) for retention vault activation
- reqwest + rustls (in tree) for HTTP update manifest + telemetry export

### Constraints
- No hosted/cloud egress without explicit user consent
- All AI provider interactions are metadata-only by default
- Every file mutation goes through the proposal pipeline
- Sandbox enforcement gaps must be honestly reported per-platform
- Terminal output must be redacted before projection (credentials, tokens)
- Plugin WASM modules execute in wasmtime sandbox with capability limits

## Open Questions

- **Wasmtime supply-chain debt (W0.7):** ADR-0019 accepted wasmtime but flagged an open supply-chain concern. Resolution path: complete the supply-chain audit and update ADR-0019 before Wave 5 begins execution. WASI 0.3 (Feb 2026) adds async I/O which may change the host ABI design.
- **Code signing credentials:** Signed installers require platform-specific certificates (Apple Developer ID, Microsoft Authenticode, GPG). Resolution: procure credentials as a prerequisite for Wave 6.
- **Live provider integration tests in CI:** Currently no CI job hits real AI provider endpoints. Resolution: decide whether to add BYOK secrets to CI or maintain recorded-transport testing only.
- **egui performance at scale:** No production IDE has shipped on egui at this scale. Resolution: Wave 7 performance work may require custom rendering for the code canvas if egui can't meet the 16ms paint budget on large files.
- **Multi-language LSP:** v1 targets Rust. Ship with LSP configuration UI that allows users to configure arbitrary language servers, but only test/validate rust-analyzer.
- **PTY library choice:** Evaluate whether legion-platform's existing PTY traits are sufficient or whether adopting portable-pty/rust-pty provides better cross-platform coverage with less maintenance.

## Start Input

Legion IDE Production Completion project. Fresh plan structure with 7 parallel waves + integration gate. Scope: all features except VS Code extension host, remote dev, and collaboration. Includes: full WASM plugin execution, all 6 AI providers with product UX, production terminal/debugger (user-selectable mode), signed installers, auto-updater, full 3-OS parity. Quality-driven timeline. Treats existing 30-crate/196k-LOC codebase, 45 ADRs, and M0-M12 evidence as the starting point.
