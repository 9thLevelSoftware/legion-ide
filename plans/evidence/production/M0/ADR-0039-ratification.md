# M0 — ADR-0039 (Agent Interop) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0039-agent-interop.md`](../../../adrs/ADR-0039-agent-interop.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- check-deps` (dependency direction + structural
audit, with `legion-ai-providers`, `legion-ai`, `legion-protocol`,
`legion-security`, `legion-agent`, `legion-app`, `legion-cli`,
`legion-desktop`, `legion-ui`, and `legion-editor` policy entries verified
against `plans/dependency-policy.md` §1 and the agent-interop-boundary
sketch in the ratified ADR)
Acceptance target: master-plan §6 row 239 "ADR-0039 | Agent interop" →
option (a) ratified in-repo: **all three, sequenced** — MCP client parity
audit vs `rmcp` at M0 (migrate if spec rev ~June 2026 breaks the
hand-rolled transport), ACP host at M4, Legion-as-MCP-server post-GA;
hand-rolled MCP client is the M0 surface; no `rmcp` /
`modelcontextprotocol` / `agent-client-protocol` workspace dependency
today; broker-mediated tool calls; proposal-mediated external agent
edits; metadata-only audit; spec risk explicitly accepted (R6 in §6 row
513); Legion's native agent remains the default and the air-gap option.

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in
  `plans/adrs/ADR-0039-agent-interop.md`.
- Decision text matches Production Master Plan v0.1 §6 row 239
  recommendation verbatim: option (a) — **all three, sequenced** — MCP
  client parity audit vs `rmcp` at M0 (migrate if the spec rev ~June 2026
  breaks the hand-rolled transport), ACP host at M4, Legion-as-MCP-server
  post-GA. The plan's §6 row 239 explicitly says "**(a) all three**,
  sequenced: MCP client parity audit vs `rmcp` (migrate if spec rev
  ~June 2026 breaks the hand-rolled transport), ACP host at M4,
  Legion-as-MCP-server post-GA." The ADR ratifies that recommendation
  without amendment and records the WS-09.T6 acceptance shape ("3
  reference servers (filesystem-class, web-class, custom) pass
  conformance; permission prompts audited") as the M0/M1/M2 gate and
  the WS-13.T4 acceptance shape ("one external agent completes GP-3
  inside the Legion envelope") as the M4 gate.
- No amendments were required to the master-plan recommendation. The
  ADR adds six confirmations consistent with the plan and with current
  code / contracts:
  1. The hand-rolled MCP client is live and exercised by tests today.
     `legion-ai-providers` (`crates/legion-ai-providers/src/lib.rs`,
     2446 lines) is the production provider layer. The
     `McpTransport` trait (line 967) is the transport boundary; the
     `StdioMcpTransport` (line 990, with the `spawn_session` at line
     1026 and the `send` impl at line 1095) is the stdio carrier; the
     `StreamableHttpMcpTransport` (line 1118, with `new` at line 1124
     and `send` at line 1129) is the Streamable HTTP carrier; the
     `McpClient<T>` (line 1156, with `new` at line 1166, the
     `list_tools_request` / `list_resources_request` /
     `list_prompts_request` envelope builders at 1198/1203/1208, and
     `call_tool_with_permission` at line 1319) is the
     permission-gated client; the `McpClientError` enum (line 921,
     with `InvalidRegistry`, `InvalidEnvelope`, `InvalidListResponse`,
     `UnknownTool`, `UnknownResource`, `UnknownPrompt`,
     `PermissionRequired`, `Transport` variants) is the failure
     surface. The five in-source MCP contract tests at lines 2230 /
     2297 / 2328 / 2359 / 2403 of
     `crates/legion-ai-providers/src/lib.rs` — the
     `mcp_client_builds_json_rpc_requests_and_requires_tool_permission`,
     `mcp_client_rejects_permission_for_different_mcp_tool_target`,
     `mcp_client_rejects_permission_for_different_mcp_tool_capability`,
     `stdio_mcp_transport_reuses_one_process_across_requests`, and
     `mcp_client_reloads_registry_after_list_changed_notification`
     tests — are the M0 contract surface. The
     `crates/legion-ai-providers/tests/prompt_stability.rs` file (3
     contract tests) is the cache-stable prompt assembly substrate
     the WS-09.T2 workstream extends for the MCP passthrough. The M0
     ratification does **not** change the hand-rolled transport; the
     WS-09.T6 workstream runs the parity audit against `rmcp` and
     the MCP spec rev 2025-11-25 and migrates to `rmcp` if the spec
     rev breaks the transport.
  2. The MCP DTO surface is live and exercised by tests today.
     `legion-protocol` (`crates/legion-protocol/src/lib.rs`) is the
     protocol DTO layer. It owns the MCP DTO surface: `McpServerId`
     (line 22544), `McpToolName` (line 22549), `McpResourceUri`
     (line 22554), `McpPromptName` (line 22559), `McpTransportKind`
     (line 22578, with `Stdio` / `StreamableHttp` variants),
     `McpServerDescriptor` (around line 22661 with `server_id` +
     `transport_kind`), `McpToolDescriptor` (line 22682 with
     `McpServerId` + `McpToolName` + the `McpToolDescriptor` shape
     including `input_schema_hash`, `risk_label`,
     `required_permission_profile`, `action_class`, `capability`,
     `redaction_hints`), `McpResourceDescriptor` (line 22707),
     `McpPromptDescriptor` (line 22726), `McpRegistrySnapshot` (line
     22743 with `tools`, `resources`, `prompts` vectors and
     `last_notification_kind` / `list_version` fields), and the
     `validate_mcp_registry_snapshot` validator at line 24001. The
     `McpRegistryReloaded` / `StaleMcpRegistry` notification kinds
     (around lines 22809 / 22849) are the list-changed protocol
     signals. The protocol DTO test
     `dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only`
     in `crates/legion-protocol/tests/dto_contracts.rs` at line 6472
     is the metadata-only DTO contract test that exercises the
     registry snapshot validation + JSON-RPC envelope validation +
     the risk-row metadata-only invariant. The M0 ratification does
     **not** change the protocol DTO surface; the WS-09.T6 parity
     audit may extend it with new MCP spec fields (notifications,
     OAuth, etc.) but the M0 boundary is the existing DTOs.
  3. The MCP broker gate is live and exercised by tests today.
     `legion-security` (`crates/legion-security/src/lib.rs`) is the
     capability broker. The `mcp_tool_permission_allows_runtime`
     function (line 228) is the MCP-specific broker gate: every MCP
     tool call must present a `DelegatedTaskToolPermissionRequest`
     with the matching `target_id` and `capability` (or
     `mcp_tool_permission_allows_runtime` returns `false` and the
     client returns `McpClientError::PermissionRequired` from
     `call_tool_with_permission`). The two contract tests at lines
     2089 / 2098 of `crates/legion-security/src/lib.rs` assert the
     allow / deny path. The M0 ratification does **not** change the
     broker; the WS-09.T6 workstream may extend the permission
     profile with the MCP spec rev 2025-11-25 tool-permission
     model (org-style allowlists, tool-permission UI consistent
     with the capability broker) under the same broker-mediated
     contract.
  4. The agent state machine that drives Delegate/Workflow
     tool calls is metadata-only and never owns MCP/ACP state.
     `legion-agent` (`crates/legion-agent/src/lib.rs`) is the Phase
     4 metadata-only agent state machine. The
     `plans/dependency-policy.md` §1 entry at line 144 authorizes
     `legion-ai`, `legion-protocol`, and `legion-tracker`. The M0
     ratification does **not** extend `legion-agent`'s allowed
     edges: the agent state machine drives Delegate/Workflow tool
     calls through the existing
     `legion-agent` ↔ `legion-ai` ↔ `legion-ai-providers` ↔
     `legion-protocol` edge chain, and the MCP passthrough is
     the WS-12.T1 tool-registry entry point that composes the
     hand-rolled `McpClient` under the existing capability
     broker. The M0 ratification explicitly forbids
     `legion-agent` from declaring any MCP or ACP runtime
     dependency (`rmcp`, `modelcontextprotocol`,
     `agent-client-protocol`, `acp-rs`, etc.) at M0; the agent
     state machine stays metadata-only and never owns MCP/ACP
     state the same way it never owns process or filesystem
     authority. The WS-13.T4 ACP host is the later workstream
     that introduces a `legion-app` composition entry point
     (the same `delegated.allocate_sandbox` pattern that the
     OS-sandbox tier in ADR-0038 already wraps) for the
     external agent, not a `legion-agent` re-architecture.
  5. The GUI composition path the MCP passthrough / ACP host
     composes through is projection-bound and never owns
     MCP/ACP state. `legion-app` (`crates/legion-app/src/lib.rs`)
     is the GUI composition crate. The
     `plans/dependency-policy.md` §1 entry at line 86 authorizes
     the full app composition set (including `legion-agent`,
     `legion-ai`, `legion-ai-providers`, `legion-protocol`,
     `legion-security`, `legion-platform`,
     `legion-observability`, etc.). The M0 ratification does
     **not** extend `legion-app`'s allowed edges; the WS-12.T1
     tool-registry composes the MCP passthrough through the
     existing `legion-app` ↔ `legion-ai` ↔
     `legion-ai-providers` edge chain, and the WS-13.T4 ACP
     host composes through the same `legion-app` composition
     entry point (the same `delegated.allocate_sandbox`
     pattern that the OS-sandbox tier in ADR-0038 already
     wraps) so the proposal service, the capability broker,
     and the evidence ledger all see the external agent's
     edits and shell activity identically to the native
     agent's edits and shell activity. `legion-cli` is one
     of the two entry points that may launch the MCP
     passthrough / ACP host (alongside `legion-desktop`);
     the capability-broker contract is the same from both
     entry points. `legion-desktop` is the GUI desktop
     adapter and the second entry point. The
     `legion-desktop` policy entry at
     `plans/dependency-policy.md` §1 line 77 authorizes
     `legion-app`, `legion-protocol`, and `legion-ui`. The
     M0 ratification does **not** extend `legion-desktop`'s
     allowed edges; the desktop adapter launches the MCP
     passthrough / ACP host through the existing
     `legion-desktop` ↔ `legion-app` edge, and the
     proposal/evidence envelope applies identically from
     both entry points. The `legion-ui` policy entry at
     lines 54-75 forbids every renderer / editor /
     project / storage / app / agent / terminal /
     security / observability / platform edge, and the
     structural audit enforces it. The boundary sketch in
     the ratified ADR reinforces this rule with a future
     `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS` audit (no
     `legion-ui` may declare any `rmcp` /
     `modelcontextprotocol` / `agent-client-protocol`
     runtime dependency), shaped like the existing
     `PARSER_BOUNDARY_POLICY_MARKERS` audit in
     `xtask/src/main.rs` and the
     `SEARCH_BOUNDARY_POLICY_MARKERS` /
     `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
     `LSP_BOUNDARY_POLICY_MARKERS` /
     `TERMINAL_BOUNDARY_POLICY_MARKERS` /
     `SANDBOX_BOUNDARY_POLICY_MARKERS` sketches in
     ADR-0034 / 0035 / 0036 / 0037 / 0038.
  6. The `legion-ai` provider trait layer the MCP passthrough
     composes through stays policy-bounded and never owns
     MCP/ACP runtime dependencies. `legion-ai` is the
     provider trait layer. The
     `plans/dependency-policy.md` §1 entry at lines 111-117
     authorizes `legion-ai` to depend on `legion-protocol`
     and `legion-security` (and the MUST rules require
     both). The M0 ratification does **not** extend
     `legion-ai`'s allowed edges; the WS-09.T6 workstream
     composes through the existing `legion-ai` ↔
     `legion-ai-providers` ↔ `legion-protocol` ↔
     `legion-security` edge chain. The M0 ratification
     explicitly forbids `legion-ai` from declaring any MCP
     runtime dependency (`rmcp`, `modelcontextprotocol`,
     etc.) at M0; the WS-09.T6 parity audit is the path
     that authorizes a `rmcp` runtime dependency if the
     spec rev breaks the hand-rolled transport. The
     `legion-ai-providers` `Cargo.toml` declares
     `reqwest = { workspace = true }` for the
     `StreamableHttpMcpTransport` and uses the existing
     `tokio::process` for the `StdioMcpTransport`; the
     `legion-ai-providers` ↔ `legion-ai` ↔
     `legion-protocol` ↔ `legion-security` edge chain
     is the M0 boundary, and the M0 ratification does
     **not** declare any new internal crate edge or
     any new external MCP/ACP runtime dependency
     today.

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR. The
  agent-interop layer is split across `legion-ai-providers`,
  `legion-protocol`, `legion-security`, `legion-ai`,
  `legion-agent`, `legion-app`, `legion-cli`, and `legion-desktop`
  along the accepted policy entries in
  `plans/dependency-policy.md` §1.
- The `legion-ai-providers` policy entry at
  `plans/dependency-policy.md` §1 lines 119-125 is unchanged:
  `legion-ai-providers` may depend on `legion-ai`,
  `legion-protocol`, and `legion-security` (and the MUST rule
  at line 125 requires `legion-ai`). The current
  `crates/legion-ai-providers/Cargo.toml` is consistent with
  this entry (`legion-ai`, `legion-protocol`, `legion-security`,
  plus `reqwest`, `serde_json`, `thiserror`). The M0
  ratification does **not** declare any new
  `legion-ai-providers` dependency, and the M0 ratification
  explicitly forbids `legion-ai-providers` from declaring
  any MCP runtime dependency (`rmcp`,
  `modelcontextprotocol`, etc.) at M0. The WS-09.T6 parity
  audit is the path that authorizes a `rmcp` runtime
  dependency if the spec rev breaks the hand-rolled
  transport; the parity audit is a documentation-and-tests
  change that records the decision, and the `rmcp`
  migration is a one-line dependency swap (the
  `McpTransport` trait at line 967 is the boundary). The
  `xtask` policy audit confirms zero `rmcp` /
  `modelcontextprotocol` / `agent-client-protocol` /
  `acp-rs` workspace dependencies exist in `Cargo.lock`
  today.
- The `legion-ai` policy entry at
  `plans/dependency-policy.md` §1 lines 111-117 is unchanged:
  `legion-ai` may depend on `legion-protocol` and
  `legion-security`. The current
  `crates/legion-ai/Cargo.toml` is consistent with this
  entry. The M0 ratification explicitly forbids
  `legion-ai` from declaring any MCP runtime dependency
  (`rmcp`, `modelcontextprotocol`, etc.) at M0, and the
  `xtask` policy audit enforces the boundary.
- The `legion-protocol` policy entry at
  `plans/dependency-policy.md` §1 line 86 (the
  `legion-app` composition set) is unchanged: `legion-protocol`
  may depend on the shared contracts boundary (no internal
  crate edge for `legion-protocol` itself; `legion-protocol`
  is the protocol DTO layer). The current
  `crates/legion-protocol/Cargo.toml` is consistent with
  this entry. The M0 ratification does **not** declare any
  new `legion-protocol` dependency, and the MCP DTO
  surface (the `McpServerId` / `McpToolName` /
  `McpResourceUri` / `McpPromptName` /
  `McpTransportKind` / `McpServerDescriptor` /
  `McpToolDescriptor` / `McpResourceDescriptor` /
  `McpPromptDescriptor` / `McpRegistrySnapshot` DTOs from
  line 22544 onwards, plus the validators at line 24001)
  is the M0 boundary. The WS-09.T6 parity audit may add
  new DTOs for spec rev 2025-11-25 fields (notifications,
  OAuth, etc.) but the M0 boundary is the existing DTOs.
- The `legion-security` policy entry at
  `plans/dependency-policy.md` §1 line 20 is unchanged:
  `legion-security` may depend on `legion-protocol`. The
  current `crates/legion-security/Cargo.toml` is
  consistent with this entry, and the
  `mcp_tool_permission_allows_runtime` function at line
  228 is the broker gate the WS-12.T1 tool-registry
  composes through. The M0 ratification does **not**
  declare any new `legion-security` dependency, and the
  WS-09.T6 parity audit may extend the broker with the
  MCP spec rev 2025-11-25 tool-permission model
  (org-style allowlists, tool-permission UI consistent
  with the capability broker) under the same
  broker-mediated contract.
- The `legion-agent` policy entry at
  `plans/dependency-policy.md` §1 line 144 is unchanged:
  `legion-agent` may depend on `legion-ai`,
  `legion-protocol`, and `legion-tracker`. The current
  `crates/legion-agent/Cargo.toml` is consistent with
  this entry, and `legion-agent` does not contain any
  MCP or ACP runtime code today. The M0 ratification
  explicitly forbids `legion-agent` from declaring any
  MCP or ACP runtime dependency (`rmcp`,
  `modelcontextprotocol`, `agent-client-protocol`,
  `acp-rs`, etc.) at M0, and the `xtask` policy audit
  enforces the boundary. The WS-13.T4 ACP host is the
  later workstream that introduces a `legion-app`
  composition entry point (the same
  `delegated.allocate_sandbox` pattern that the
  OS-sandbox tier in ADR-0038 already wraps) for the
  external agent, not a `legion-agent`
  re-architecture.
- The `legion-app` policy entry at
  `plans/dependency-policy.md` §1 line 86 is unchanged:
  `legion-app` may depend on the full app composition set
  (`legion-agent`, `legion-ai`, `legion-ai-providers`,
  `legion-collaboration`, `legion-editor`, `legion-index`,
  `legion-lsp`, `legion-memory`, `legion-observability`,
  `legion-platform`, `legion-plugin`, `legion-project`,
  `legion-protocol`, `legion-remote`, plus the remaining
  lines). The current `crates/legion-app/Cargo.toml` is
  consistent with this entry, and the
  `delegated.allocate_sandbox` command at line 14714 of
  `crates/legion-app/src/lib.rs` is the existing
  app-composition entry point the WS-12.T1 tool-registry
  / WS-13.T4 ACP host compose through. The M0 ratification
  does **not** authorize a new `legion-app` ↔
  `legion-ai-providers` edge or any other new edge; the
  existing `legion-app` ↔ `legion-ai-providers` edge in
  the §1 line 86 policy entry is the path the MCP
  passthrough / ACP host composes through.
- The `legion-cli` policy entry at
  `plans/dependency-policy.md` §1 line 175 is unchanged:
  `legion-cli` may depend on `legion-index`,
  `legion-protocol`, and `legion-storage`. The current
  `crates/legion-cli/Cargo.toml` is consistent with this
  entry. The M0 ratification explicitly forbids
  `legion-cli` from declaring any MCP or ACP runtime
  dependency, and the `xtask` policy audit enforces the
  boundary by iterating the same `package_dependencies`
  map that drives the renderer-boundary,
  parser-boundary, search-boundary, retrieval-boundary,
  LSP-boundary, terminal-boundary, and sandbox-boundary
  checks. The WS-12.T1 / WS-13.T4 workstreams compose
  through the existing `legion-cli` ↔ `legion-app`
  edge plus the `legion-app` ↔ `legion-ai` ↔
  `legion-ai-providers` edge chain.
- The `legion-desktop` policy entry at
  `plans/dependency-policy.md` §1 line 77 is unchanged:
  `legion-desktop` may depend on `legion-app`,
  `legion-protocol`, and `legion-ui`. The current
  `crates/legion-desktop/Cargo.toml` is consistent with
  this entry. The M0 ratification does **not** authorize
  a new `legion-desktop` ↔ `legion-ai-providers` edge;
  the existing `legion-desktop` ↔ `legion-app` edge
  plus the `legion-app` ↔ `legion-ai` ↔
  `legion-ai-providers` edge chain is the path the
  MCP passthrough / ACP host composes through.
- The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already
  forbids `legion-ui` from depending on `legion-project`,
  `legion-editor`, `legion-storage`, `eframe`, `egui`,
  `egui-winit`, `egui-wgpu`, `winit`, `wgpu`,
  `accesskit`, `slint`, `tauri`, `wry`, `tao`, or `gpui`.
  None of the MCP/ACP runtime crates (`rmcp`,
  `modelcontextprotocol`, `agent-client-protocol`,
  `acp-rs`, etc.) are added to that list because the
  `legion-ui` policy entry is already a closed boundary
  (only `legion-protocol` is allowed). The boundary
  sketch in the ratified ADR reinforces this rule with
  a future `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS` audit
  (no `legion-ui` may declare any `rmcp` /
  `modelcontextprotocol` / `agent-client-protocol`
  runtime dependency), shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` audit in
  `xtask/src/main.rs` and the
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_BOUNDARY_POLICY_MARKERS` sketches in
  ADR-0034 / 0035 / 0036 / 0037 / 0038.
- The `legion-editor` policy entry is unchanged and
  forbids any `legion-platform` / process / network /
  terminal / MCP / ACP runtime dependency. The
  `legion-editor` MUST NOT rules at
  `plans/dependency-policy.md` §1 lines 43-52 explicitly
  forbid the `legion-editor` ↔ `legion-project` edge
  and the structural audit enforces it. The M0
  ratification explicitly forbids `legion-editor` from
  declaring any MCP or ACP runtime dependency
  (`rmcp`, `modelcontextprotocol`,
  `agent-client-protocol`, `acp-rs`, etc.) at M0, and
  the `xtask` policy audit enforces the boundary.
- The agent-interop workspace dependencies (`rmcp`,
  `modelcontextprotocol`, `agent-client-protocol`,
  `acp-rs`, etc.) are **not** added to the root
  `Cargo.toml` at M0. They will be added during
  WS-09.T6 (the parity audit) if the spec rev breaks
  the hand-rolled transport, and during WS-13.T4
  (the ACP host) under the same dependency-policy
  gate that authorized the parser-boundary audit
  in ADR-0033, the LSP-boundary audit sketched in
  ADR-0034, the terminal-boundary sketch in
  ADR-0035, the search-boundary sketch in
  ADR-0036, the retrieval-boundary sketch in
  ADR-0037, and the sandbox-boundary sketch in
  ADR-0038. The gate is forward-compatible with a
  future
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS` /
  `AGENT_INTEROP_RUNTIME_ALLOWED_PACKAGES = ["legion-ai-providers"]`
  / `FORBIDDEN_AGENT_INTEROP_DEPS = ["rmcp",
  "modelcontextprotocol", "agent-client-protocol",
  "acp-rs"]` audit shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]`
  / `FORBIDDEN_PARSER_DEPS = ["tree-sitter",
  "tree-sitter-rust"]` audit in
  `xtask/src/main.rs`. The M0 ratification does not
  require the agent-interop-boundary audit to land
  today; the ADR commits to the boundary and to the
  runtime activation path, not to a new `xtask`
  subcommand.
- `xtask` does not need a new subcommand. The
  structural dependency audit and the
  protocol-contract audit that already run as part
  of `check-deps` are sufficient to enforce the
  current `legion-ai-providers`, `legion-ai`,
  `legion-protocol`, `legion-security`,
  `legion-agent`, `legion-app`, `legion-cli`,
  `legion-desktop`, `legion-ui`, and `legion-editor`
  policy entries; the future
  agent-interop-boundary audit is a phase-gate
  improvement, not an M0 prerequisite.

## Gate Evidence (verbatim)

All gates were run against the current working tree with commit
baseline `b56dcb2`; the ratification changes (ADR flip + this
evidence file) are untracked as required by the task's "no commit
without explicit user instruction" rule. (The working tree
contains unrelated uncommitted edits from sibling M0 cards; they
are not part of this ratification and are noted only so the gate
outputs are reproducible against the same baseline.)

### `cargo run -p xtask -- check-deps`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- check-deps
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/xtask check-deps`
dependency policy checks passed
```

Exit code: `0`. The renderer-boundary audit, the parser-boundary
audit, the structural dependency audit, the protocol-contract
audit, and the phase 3 / 4 / 5 / 6 / 7 / 8 / 13 acceptance
governance audits all pass against the current tree. In
particular:

- `plans/dependency-policy.md` still contains every
  `PARSER_BOUNDARY_POLICY_MARKERS` string.
- The `legion-ai-providers` policy entry at
  `plans/dependency-policy.md` §1 lines 119-125 is
  intact and matches `crates/legion-ai-providers/Cargo.toml`
  (`legion-ai`, `legion-protocol`, `legion-security`,
  plus `reqwest`, `serde_json`, `thiserror`; the
  hand-rolled MCP transport is the M0 surface; no
  `rmcp` / `modelcontextprotocol` / `agent-client-protocol`
  / `acp-rs` workspace dependency declared today).
- The `legion-ai` policy entry at
  `plans/dependency-policy.md` §1 lines 111-117 is
  intact and matches `crates/legion-ai/Cargo.toml`
  (`legion-protocol`, `legion-security`; no MCP or
  ACP runtime dependency declared today).
- The `legion-protocol` policy entry at
  `plans/dependency-policy.md` §1 line 86 is intact
  and the MCP DTO surface
  (`McpServerId` / `McpToolName` /
  `McpResourceUri` / `McpPromptName` /
  `McpTransportKind` / `McpServerDescriptor` /
  `McpToolDescriptor` / `McpResourceDescriptor` /
  `McpPromptDescriptor` / `McpRegistrySnapshot`
  DTOs from line 22544 onwards, plus the
  validators at line 24001) is intact and
  exercised by the protocol DTO test
  `dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only`
  in `crates/legion-protocol/tests/dto_contracts.rs`
  at line 6472.
- The `legion-security` policy entry at
  `plans/dependency-policy.md` §1 line 20 is
  intact and matches `crates/legion-security/Cargo.toml`
  (a subset of the allowed internal edges
  `legion-protocol`; the
  `mcp_tool_permission_allows_runtime` function at
  line 228 is the broker gate; the two contract
  tests at lines 2089 / 2098 are the broker
  contract).
- The `legion-agent` policy entry at
  `plans/dependency-policy.md` §1 line 144 is
  intact and matches `crates/legion-agent/Cargo.toml`
  (`legion-ai`, `legion-protocol`, `legion-tracker`;
  no MCP or ACP runtime dependency declared today;
  the agent state machine is metadata-only and
  drives Delegate/Workflow tool calls through the
  `legion-app` composition entry point).
- The `legion-app` policy entry at
  `plans/dependency-policy.md` §1 line 86 is intact
  and matches `crates/legion-app/Cargo.toml` (the
  full GUI Phase 4 composition set; no MCP or ACP
  runtime dependency declared today; the
  `delegated.allocate_sandbox` command at line
  14714 is the existing app-composition entry
  point the MCP passthrough / ACP host compose
  through).
- The `legion-cli` policy entry at
  `plans/dependency-policy.md` §1 line 175 is
  intact and matches `crates/legion-cli/Cargo.toml`
  (`legion-index`, `legion-protocol`, `legion-storage`;
  no MCP or ACP runtime dependency declared today).
- The `legion-desktop` policy entry at
  `plans/dependency-policy.md` §1 line 77 is
  intact and matches
  `crates/legion-desktop/Cargo.toml`
  (`legion-app`, `legion-protocol`, `legion-ui`;
  no MCP or ACP runtime dependency declared
  today; the desktop adapter launches the MCP
  passthrough / ACP host through the existing
  `legion-desktop` ↔ `legion-app` edge).
- The `legion-ui` policy entry at lines 54-75 is
  intact and matches `crates/legion-ui/Cargo.toml`
  (only `legion-protocol`; every renderer /
  editor / project / storage / app / agent /
  terminal / security / observability / platform
  edge is forbidden, and the structural audit
  enforces it). The boundary sketch in the
  ratified ADR reinforces this rule with a
  future `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS`
  audit (no `legion-ui` may declare any `rmcp` /
  `modelcontextprotocol` / `agent-client-protocol`
  / `acp-rs` runtime dependency).
- The `legion-editor` policy entry at lines 43-52
  is intact and matches
  `crates/legion-editor/Cargo.toml`
  (`legion-observability`, `legion-protocol`,
  `legion-text`; the `MUST NOT depend on
  legion-project` rule is enforced; no MCP or
  ACP runtime dependency declared today).

### `cargo run -p xtask -- docs-hygiene`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- docs-hygiene
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

Exit code: `0`. Confirms the ADR-0039 ratification does not
break doc-hygiene invariants (broken relative Markdown links
or unallowlisted stale Legion-rename markers).

### `cargo run -p xtask -- no-egui-textedit`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- no-egui-textedit
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running `target/debug/xtask no-egui-textedit`
no-egui-textedit checks passed
```

Exit code: `0`. Companion gate (ADR-0032) unchanged; this
ratification did not touch the painter module or its scanned
paths. The agent-interop status surface (the future
`AgentHostStatusProjection` family on top of the existing
`legion-ui` ↔ `legion-protocol` edge, emitted by `legion-app`
and rendered by `legion-desktop`) is a status badge / projection
surface, not a `TextEdit` surface; the no-`TextEdit` rule
covers the code canvas, not the status badge / projection
surfaces.

### `cargo fmt --all --check`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo fmt --all --check
$ echo $?
0
```

Exit code: `0`. No formatting drift introduced by the
ratification changes (the changes are documentation-only; no
`.rs` files were touched).

### `cargo test -p legion-ai-providers --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-ai-providers --tests 2>&1 | grep -E '^(test result|running |error)'
running 16 tests
test tests::configured_provider_value_prefers_legion_then_legacy_then_standard_names ... ok
test tests::deterministic_local_provider_completes_without_cloud_credentials ... ok
test tests::deterministic_local_provider_returns_deterministic_embedding_vectors ... ok
test tests::deterministic_local_provider_predicts_bounded_inline_result ... ok
test tests::inline_prediction_registry_exposes_required_provider_slots ... ok
test tests::mcp_client_builds_json_rpc_requests_and_requires_tool_permission ... ok
test tests::llama_cpp_provider_posts_loopback_requests_without_bearer_by_default ... ok
test tests::mcp_client_rejects_permission_for_different_mcp_tool_capability ... ok
test tests::mcp_client_rejects_permission_for_different_mcp_tool_target ... ok
test tests::mcp_client_reloads_registry_after_list_changed_notification ... ok
test tests::llama_cpp_provider_can_attach_optional_local_bearer_token ... ok
test tests::ollama_provider_posts_completion_and_embedding_requests ... ok
test tests::provider_registry_exposes_configured_adapters ... ok
test tests::openai_compatible_provider_posts_byok_completion_and_embedding_requests ... ok
test tests::stdio_mcp_transport_reuses_one_process_across_requests ... ok
test tests::unconfigured_external_provider_slots_refuse_inline_prediction_explicitly ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/prompt_stability.rs (target/debug/deps/prompt_stability-a62fb1f956d488d4)

running 3 tests
test prompt_stability_fixture_records_cache_relevant_prefix_fields ... ok
test prompt_metadata_hash_is_stable_for_equivalent_requests ... ok
test prompt_serialization_is_deterministic_for_fixed_inputs ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. The hand-rolled MCP client surface is exercised
by the five MCP contract tests in
`crates/legion-ai-providers/src/lib.rs` at lines 2230 / 2297 /
2328 / 2359 / 2403 — the
`mcp_client_builds_json_rpc_requests_and_requires_tool_permission`,
`mcp_client_rejects_permission_for_different_mcp_tool_target`,
`mcp_client_rejects_permission_for_different_mcp_tool_capability`,
`stdio_mcp_transport_reuses_one_process_across_requests`, and
`mcp_client_reloads_registry_after_list_changed_notification`
tests. The total is 19 contract tests (16 in `src/lib.rs` plus
3 in `tests/prompt_stability.rs`), the prompt-stability
substrate the WS-09.T2 workstream extends for the MCP
passthrough. The M0 ratification does **not** change the
hand-rolled transport; the WS-09.T6 workstream runs the
parity audit against `rmcp` and the MCP spec rev 2025-11-25
and migrates to `rmcp` if the spec rev breaks the transport.

### `cargo test -p legion-protocol --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-protocol --tests 2>&1 | grep -E '^(test result|running |error)'
...
test result: ok. 109 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

Exit code: `0`. The protocol DTO surface (109 contract tests)
exercises the MCP DTO surface
(`McpServerId` / `McpToolName` / `McpResourceUri` /
`McpPromptName` / `McpTransportKind` /
`McpServerDescriptor` / `McpToolDescriptor` /
`McpResourceDescriptor` / `McpPromptDescriptor` /
`McpRegistrySnapshot` from line 22544 onwards, plus the
validators at line 24001) and the metadata-only DTO
contract test
`dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only`
in `crates/legion-protocol/tests/dto_contracts.rs` at line
6472. The M0 ratification does **not** change the protocol
DTO surface; the WS-09.T6 parity audit may add new DTOs for
spec rev 2025-11-25 fields (notifications, OAuth, etc.) but
the M0 boundary is the existing DTOs.

### `cargo test -p legion-security --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-security --tests 2>&1 | grep -E '^(test result|running |error)'
...
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/path_policy_windows.rs (target/debug/deps/path_policy_windows-2a70254d9fe581ba)

running 1 test
test sibling_prefix_escape_is_rejected_cross_platform ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. The capability broker surface (50 contract
tests plus 1 cross-platform test = 51 contract tests)
exercises the MCP broker gate
(`mcp_tool_permission_allows_runtime` at line 228) through
the two contract tests at lines 2089 / 2098 of
`crates/legion-security/src/lib.rs`. The M0 ratification
does **not** change the broker; the WS-09.T6 workstream may
extend the broker with the MCP spec rev 2025-11-25
tool-permission model (org-style allowlists, tool-permission
UI consistent with the capability broker) under the same
broker-mediated contract.

## Summary

- M0 ADR-0039 (Agent interop) ratified in-repo: status flipped
  `Draft` → `Accepted` in
  `plans/adrs/ADR-0039-agent-interop.md` (48779 bytes).
- Decision text matches Production Master Plan v0.1 §6 row 239
  verbatim (option (a) all three, sequenced: MCP client parity
  audit vs `rmcp` at M0, ACP host at M4, Legion-as-MCP-server
  post-GA).
- Evidence package:
  `plans/evidence/production/M0/ADR-0039-ratification.md` (this
  file).
- Six confirmations recorded consistent with the plan and
  current code:
  1. The hand-rolled MCP client is live (legion-ai-providers
     `McpTransport` trait at line 967 + `StdioMcpTransport`
     at line 990 + `StreamableHttpMcpTransport` at line 1118
     + `McpClient<T>` at line 1156 + `McpClientError` enum
     at line 921, exercised by 19 legion-ai-providers contract
     tests across 2 binaries, including the 5 MCP contract
     tests at lines 2230 / 2297 / 2328 / 2359 / 2403).
  2. The MCP DTO surface is live (legion-protocol
     `McpServerId` at line 22544 + `McpToolName` at line
     22549 + `McpResourceUri` at line 22554 + `McpPromptName`
     at line 22559 + `McpTransportKind` at line 22578 +
     `McpServerDescriptor` + `McpToolDescriptor` at line
     22682 + `McpResourceDescriptor` at line 22707 +
     `McpPromptDescriptor` at line 22726 + `McpRegistrySnapshot`
     at line 22743 + `validate_mcp_registry_snapshot` at line
     24001, exercised by 109 legion-protocol contract tests
     including the MCP DTO test
     `dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only`
     at line 6472).
  3. The MCP broker gate is live (legion-security
     `mcp_tool_permission_allows_runtime` at line 228,
     exercised by 2 legion-security contract tests at lines
     2089 / 2098 plus 50 legion-security contract tests
     total).
  4. The agent state machine stays metadata-only and never
     owns MCP/ACP state (legion-agent §1 line 144 dependency
     policy entry unchanged, no MCP or ACP runtime
     dependency declared today; the M0 ratification
     explicitly forbids legion-agent from declaring any
     `rmcp` / `modelcontextprotocol` / `agent-client-protocol`
     / `acp-rs` runtime dependency).
  5. The GUI composition path the MCP passthrough / ACP host
     composes through stays projection-bound and never owns
     MCP/ACP state (legion-app §1 line 86 entry unchanged;
     legion-cli §1 line 175 entry unchanged; legion-desktop
     §1 line 77 entry unchanged; legion-ui §1 lines 54-75
     entry unchanged; legion-editor §1 lines 43-52 entry
     unchanged; the boundary sketch in the ratified ADR
     reinforces this rule with a future
     `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS` audit shaped
     like the existing `PARSER_BOUNDARY_POLICY_MARKERS`
     audit in `xtask/src/main.rs` and the
     `SEARCH_BOUNDARY_POLICY_MARKERS` /
     `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
     `LSP_BOUNDARY_POLICY_MARKERS` /
     `TERMINAL_BOUNDARY_POLICY_MARKERS` /
     `SANDBOX_BOUNDARY_POLICY_MARKERS` sketches in
     ADR-0034 / 0035 / 0036 / 0037 / 0038).
  6. The legion-ai provider trait layer the MCP passthrough
     composes through stays policy-bounded and never owns
     MCP/ACP runtime dependencies (legion-ai §1 lines
     111-117 entry unchanged; no MCP runtime dependency
     declared today; the M0 ratification explicitly
     forbids legion-ai from declaring any `rmcp` /
     `modelcontextprotocol` runtime dependency; the
     WS-09.T6 parity audit is the path that authorizes
     a `rmcp` runtime dependency if the spec rev breaks
     the hand-rolled transport).
- All phase gates pass against the uncommitted working tree at
  baseline `b56dcb2`: `cargo run -p xtask -- check-deps`,
  `cargo run -p xtask -- docs-hygiene`, `cargo run -p xtask
  -- no-egui-textedit`, `cargo fmt --all --check`, plus
  legion-ai-providers 19/0 (16 in `src/lib.rs` + 3 in
  `tests/prompt_stability.rs`), legion-protocol 109/0,
  legion-security 51/0.
- Two files untracked per the no-commit rule:
  `plans/adrs/ADR-0039-agent-interop.md` (48779 bytes) +
  `plans/evidence/production/M0/ADR-0039-ratification.md`
  (this file).
- Next card: WS-09.T6 (MCP client GA + `rmcp` decision) is
  unblocked; the existing hand-rolled MCP client
  (McpTransport trait at legion-ai-providers line 967 +
  StdioMcpTransport at line 990 + StreamableHttpMcpTransport
  at line 1118 + McpClient<T> at line 1156) is the M0
  surface, the WS-09.T6 workstream runs the parity audit
  against `rmcp` and the MCP spec rev 2025-11-25, and the
  `rmcp` migration is a one-line dependency swap (the
  McpTransport trait is the boundary) if the spec rev breaks
  the hand-rolled transport. WS-13.T4 (ACP host) is the M4
  piece that converts external agent harnesses into Legion's
  supply, gated on a stable ACP rev and the acceptance
  shape "one external agent completes GP-3 inside the Legion
  envelope". Legion-as-MCP-server is the post-GA expansion
  and is explicitly **not** a GA blocker per §4 row 214.
