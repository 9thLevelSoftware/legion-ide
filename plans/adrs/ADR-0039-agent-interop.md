# ADR-0039: Agent Interop

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6
recommendation verbatim (option (a), **all three, sequenced:
MCP client parity audit vs `rmcp` (migrate if the ~June 2026
MCP spec rev breaks the hand-rolled transport), ACP host at
M4, Legion-as-MCP-server post-GA**) and records the resulting
crate boundary and the "convert competitors' harnesses into
Legion's supply" thesis from §4 (do not build what ACP lets
you rent) and §6 row 239. The three pieces compose into a
single trust story: Legion already ships a hand-rolled MCP
client (stdio + Streamable HTTP) over its existing capability
broker today; the parity audit is the M0 workstream that
decides whether the hand-rolled transport remains the
production surface or migrates to `rmcp`; the ACP host lands
at M4 via WS-13.T4 so Claude Code, Codex-class, and
Gemini-class external agents run inside Legion's
proposal/evidence envelope; Legion-as-MCP-server is a
post-GA expansion that exposes Legion's tools (read/grep/
glob/outline/edit-as-proposal/terminal) as MCP primitives
without becoming a GA blocker. Legion's native agent remains
the default and the air-gap option; ACP-hosted external
agents are an opt-in addition that the proposal/evidence
envelope gates, never a replacement for the native agent.

## Context

Legion's strategic position is the control plane around
agents, not the agent harnesses themselves: proposals,
evidence, policy, review, sandboxing, and fleet
orchestration are the differentiators (Production Master
Plan §4 calls this out as "do not build what ACP lets you
rent"). The 2026 market converged on multi-vendor agent
hosting as a first-class surface — Zed and Devin Desktop
host Claude Code, Codex-class, and Gemini-class external
agents natively through the Agent Client Protocol — and
Legion needs the same composition path so external agents
run inside Legion's proposal/evidence envelope, with their
edits landing as proposals, their shell running inside
Legion sandboxes, and their activity feeding the Legion
communication stream/evidence model. MCP is the substrate
the trust story builds on: it won the 2026 protocol race
(~97M monthly SDK downloads reported; AAIF under the Linux
Foundation governs MCP + AGENTS.md + goose), and Legion
already speaks it through a hand-rolled client that ships
in `legion-ai-providers`.

The mid-plan honesty call-out in §2.2.2 makes this ADR
uncomfortable-but-necessary: the spec risk on both MCP and
ACP is real, and the plan calls out a pre-decided
mitigation — keep the hand-rolled MCP transport behind a
trait so a future `rmcp` migration is mechanical, and
confirm at M0 that the production hand-rolled transport
still matches the current MCP spec rev (2025-11-25) so
the migration is a deliberate choice, not a forced rewrite.
The June 2026 MCP spec rev is the planned trigger; the M0
ratification is the audit, not the migration.

The §6 row compares three options: (a) **all three
sequenced** (MCP client parity audit, ACP host at M4,
Legion-as-MCP-server post-GA), (b) MCP client only (status
quo — the hand-rolled transport, no ACP, no Legion-as-
server), and (c) replace the hand-rolled MCP client with
`rmcp` and ignore ACP. Option (a) is the master plan's
recommendation because it converts external agent
harnesses into supply for Legion's trust/fleet layer while
preserving the option to migrate to `rmcp` if spec churn
makes the hand-rolled transport a liability. Option (b) is
the current state minus the post-GA pieces, and §4 rejects
it because it leaves Legion competing with the harness
instead of composing with it. Option (c) is the spec-
churn defensive choice and is rejected because the
hand-rolled transport is small (one transport trait plus
two implementations plus one client plus a permission
gate), it's already exercised by 19 contract tests, and
migrating to `rmcp` is a one-line dependency swap if the
~June 2026 spec rev breaks the transport — option (c) is
what the WS-09.T6 acceptance shape explicitly enables.

The current Legion state is real and exercised by tests:

- `legion-ai-providers` (`crates/legion-ai-providers/src/lib.rs`,
  2446 lines) is the production provider layer. It
  ships the hand-rolled MCP client surface. The
  `McpTransport` trait at line 967 is the transport
  boundary; the `StdioMcpTransport` (line 990, with the
  `spawn_session` at line 1026 and the `send` impl at
  line 1095) is the stdio carrier; the
  `StreamableHttpMcpTransport` (line 1118, with `new` at
  line 1124 and `send` at line 1129) is the Streamable
  HTTP carrier; the `McpClient<T>` (line 1156, with
  `new` at line 1166, `list_tools_request` /
  `list_resources_request` / `list_prompts_request`
  envelope builders at 1198/1203/1208, and
  `call_tool_with_permission` at line 1319) is the
  permission-gated client; the `McpClientError` enum
  (line 921, with `InvalidRegistry`,
  `InvalidEnvelope`, `InvalidListResponse`,
  `UnknownTool`, `UnknownResource`, `UnknownPrompt`,
  `PermissionRequired`, `Transport` variants) is the
  failure surface. The five in-source MCP contract
  tests at lines 2230 / 2297 / 2328 / 2359 / 2403 of
  `crates/legion-ai-providers/src/lib.rs` — the
  `mcp_client_builds_json_rpc_requests_and_requires_tool_permission`,
  `mcp_client_rejects_permission_for_different_mcp_tool_target`,
  `mcp_client_rejects_permission_for_different_mcp_tool_capability`,
  `stdio_mcp_transport_reuses_one_process_across_requests`,
  and
  `mcp_client_reloads_registry_after_list_changed_notification`
  tests — are the M0 contract surface. The
  `mcp_registry` fixture at line 2175 is the test
  registry. The `crates/legion-ai-providers/tests/prompt_stability.rs`
  file (3 contract tests) is the cache-stable prompt
  assembly substrate the WS-09.T2 workstream extends
  for the MCP passthrough. The M0 ratification does
  not change the hand-rolled transport; the
  WS-09.T6 workstream runs the parity audit against
  `rmcp` and the MCP spec rev 2025-11-25 and migrates
  to `rmcp` if the spec rev breaks the transport.

- `legion-protocol` (`crates/legion-protocol/src/lib.rs`)
  is the protocol DTO layer. It owns the MCP DTO
  surface: `McpServerId` (line 22544), `McpToolName`
  (line 22549), `McpResourceUri` (line 22554),
  `McpPromptName` (line 22559), `McpTransportKind`
  (line 22578, with `Stdio` / `StreamableHttp`
  variants), `McpServerDescriptor` (around line
  22661 with `server_id` + `transport_kind`),
  `McpToolDescriptor` (line 22682 with
  `McpServerId` + `McpToolName` + the
  `McpToolDescriptor` shape including
  `input_schema_hash`, `risk_label`,
  `required_permission_profile`,
  `action_class`, `capability`, `redaction_hints`),
  `McpResourceDescriptor` (line 22707),
  `McpPromptDescriptor` (line 22726),
  `McpRegistrySnapshot` (line 22743 with `tools`,
  `resources`, `prompts` vectors and
  `last_notification_kind` /
  `list_version` fields), and the
  `validate_mcp_registry_snapshot` validator at
  line 24001. The `McpRegistryReloaded` /
  `StaleMcpRegistry` notification kinds (around
  lines 22809 / 22849) are the list-changed
  protocol signals. The protocol DTO test
  `dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only`
  in `crates/legion-protocol/tests/dto_contracts.rs`
  at line 6472 is the metadata-only DTO contract
  test that exercises the registry snapshot
  validation + JSON-RPC envelope validation +
  the risk-row metadata-only invariant. The M0
  ratification does not change the protocol DTO
  surface; the WS-09.T6 parity audit may extend
  it with new MCP spec fields (notifications,
  OAuth, etc.) but the M0 boundary is the
  existing DTOs.

- `legion-security` (`crates/legion-security/src/lib.rs`)
  is the capability broker. The
  `mcp_tool_permission_allows_runtime` function
  (line 228) is the MCP-specific broker gate:
  every MCP tool call must present a
  `DelegatedTaskToolPermissionRequest` with the
  matching `target_id` and `capability` (or
  `mcp_tool_permission_allows_runtime` returns
  `false` and the client returns
  `McpClientError::PermissionRequired` from
  `call_tool_with_permission`). The two contract
  tests at lines 2089 / 2098 of
  `crates/legion-security/src/lib.rs` assert the
  allow / deny path. The M0 ratification does
  not change the broker; the WS-09.T6 workstream
  may extend the permission profile with the
  MCP spec rev 2025-11-25 tool-permission model
  (org-style allowlists, tool-permission UI
  consistent with the capability broker) under
  the same broker-mediated contract.

- `legion-ai` (`crates/legion-ai/`) is the
  provider trait layer. The
  `plans/dependency-policy.md` §1 entry at line
  111 authorizes `legion-ai` to depend on
  `legion-protocol` and `legion-security` (and
  the MUST rules at lines 115-117 require both).
  The M0 ratification does not extend
  `legion-ai`'s allowed edges; the WS-09.T6
  workstream composes through the existing
  `legion-ai` ↔ `legion-ai-providers` ↔
  `legion-protocol` ↔ `legion-security` edge
  chain. The M0 ratification explicitly forbids
  `legion-ai` from declaring any MCP runtime
  dependency (`rmcp`, `modelcontextprotocol`,
  etc.) at M0; the WS-09.T6 parity audit is the
  path that authorizes a `rmcp` runtime
  dependency if the spec rev breaks the
  hand-rolled transport.

- `legion-agent` (`crates/legion-agent/`) is
  the Phase 4 metadata-only agent state machine.
  The `plans/dependency-policy.md` §1 entry at
  line 144 authorizes `legion-ai`,
  `legion-protocol`, and `legion-tracker`. The
  M0 ratification does not extend
  `legion-agent`'s allowed edges: the agent
  state machine drives Delegate/Workflow tool
  calls through the existing
  `legion-agent` ↔ `legion-ai` ↔
  `legion-ai-providers` ↔ `legion-protocol`
  edge chain, and the MCP passthrough is the
  WS-12.T1 tool-registry entry point that
  composes the hand-rolled `McpClient` under
  the existing capability broker. The M0
  ratification explicitly forbids
  `legion-agent` from declaring any MCP
  runtime dependency (`rmcp`,
  `modelcontextprotocol`, `agent-client-protocol`,
  etc.) at M0; the agent state machine stays
  metadata-only and never owns MCP state the
  same way it never owns process or filesystem
  authority. The WS-13.T4 ACP host is the
  later workstream that introduces a
  separate `legion-acp` (or equivalent) host
  substrate composed under the
  capability broker, not a `legion-agent`
  re-architecture.

- `legion-app` (`crates/legion-app/`) is the GUI
  composition crate. The
  `plans/dependency-policy.md` §1 entry at line
  86 authorizes the full app composition set
  (including `legion-agent`, `legion-ai`,
  `legion-ai-providers`, `legion-protocol`,
  `legion-security`, `legion-platform`,
  `legion-observability`, etc.). The M0
  ratification does not extend `legion-app`'s
  allowed edges; the WS-12.T1 tool-registry
  composes the MCP passthrough through the
  existing `legion-app` ↔ `legion-ai` ↔
  `legion-ai-providers` edge chain, and the
  WS-13.T4 ACP host composes through the
  same `legion-app` composition entry point
  (the same `delegated.allocate_sandbox`
  pattern that the OS-sandbox tier in
  ADR-0038 already wraps) so the proposal
  service, the capability broker, and the
  evidence ledger all see the external agent's
  edits and shell activity identically to the
  native agent's edits and shell activity.

- `legion-cli` (`crates/legion-cli/`,
  declared at `plans/dependency-policy.md` §1
  line 175) is the CLI composition crate
  authorized to depend on `legion-index`,
  `legion-protocol`, and `legion-storage`.
  The M0 ratification does not extend
  `legion-cli`'s allowed edges; the
  WS-12.T1 / WS-13.T4 workstreams compose
  through the existing `legion-cli` ↔
  `legion-app` edge plus the `legion-app` ↔
  `legion-ai` ↔ `legion-ai-providers` edge
  chain. The CLI is one of the two entry
  points that may launch the MCP passthrough
  / ACP host (alongside `legion-desktop`); the
  capability-broker contract is the same from
  both entry points.

- `legion-desktop`
  (`crates/legion-desktop/src/view.rs`) is the
  GUI desktop adapter and the second entry
  point. The `legion-desktop` policy entry at
  `plans/dependency-policy.md` §1 line 77
  authorizes `legion-app`, `legion-protocol`,
  and `legion-ui`. The M0 ratification does
  not extend `legion-desktop`'s allowed
  edges; the desktop adapter launches the
  MCP passthrough / ACP host through the
  existing `legion-desktop` ↔ `legion-app`
  edge, and the proposal/evidence envelope
  applies identically from both entry points.

- `legion-ui` (`crates/legion-ui/`) is the
  projection-only UI shell. The `legion-ui`
  policy entry at
  `plans/dependency-policy.md` §1 lines
  54-75 forbids every renderer / editor /
  project / storage / app / agent / terminal
  / security / observability / platform edge
  and only allows `legion-protocol`. The M0
  ratification does not extend `legion-ui`'s
  allowed edges: the ACP-host status surface
  (the WS-13.T4 acceptance shape) is a new
  `AgentHostStatusProjection` family on top
  of the existing `legion-ui` ↔
  `legion-protocol` edge, emitted by
  `legion-app` and rendered by
  `legion-desktop`. `legion-ui` never owns
  MCP state, never owns the ACP host
  transport, and never owns mutation
  authority. The boundary sketch in this
  ADR reinforces this rule with a future
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS`
  audit (no `legion-ui` may declare any
  `rmcp` / `modelcontextprotocol` /
  `agent-client-protocol` runtime
  dependency), shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` audit
  in `xtask/src/main.rs` and the
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_BOUNDARY_POLICY_MARKERS`
  sketches in ADR-0034 / 0035 / 0036 /
  0037 / 0038.

- `legion-editor` is the editor substrate.
  The M0 ratification does not extend
  `legion-editor`'s allowed edges
  (forbids any `rmcp` / MCP / ACP runtime
  dependency) and the `legion-editor`
  policy entry at
  `plans/dependency-policy.md` §1 lines
  43-52 (the `MUST NOT depend on
  legion-project` rule plus the editor
  permission set) remains intact.

The §2.2 invariants constrain the agent-interop
layer:

- **App-composed and capability-gated** —
  the MCP passthrough is composed through
  the existing `legion-ai-providers` ↔
  `legion-security` ↔ `legion-protocol`
  edge chain, and the
  `mcp_tool_permission_allows_runtime`
  broker gate at `legion-security` line
  228 is the policy surface. The ACP host
  (the WS-13.T4 workstream) is composed
  through the existing `legion-app`
  composition path (the same
  `delegated.allocate_sandbox` entry
  point that the OS-sandbox tier in
  ADR-0038 wraps), and the same
  `terminal.launch` / `terminal.close` /
  `terminal.kill` /
  `delegated.runtime.allocate` /
  `sandbox.os.activate` /
  `sandbox.os.network.activate` /
  `sandbox.os.fs.activate` capabilities
  (the Phase 8 production reservation
  set at
  `plans/dependency-policy.md` §1 line
  247 plus the OS-sandbox extensions
  from ADR-0038) are the policy surface.
  External agents cannot bypass the
  broker; their tool calls land as
  proposals, their shell runs in
  Legion sandboxes, and their activity
  feeds the Legion communication
  stream/evidence model. The WS-13.T4
  workstream extends the Phase 8
  production capability reservation set
  with the ACP-host names (for example
  `acp.host.connect`,
  `acp.host.spawn`,
  `acp.host.terminate`) and the broker
  is the ACP-host audit point.

- **Proposal-mediated mutation** —
  external agent edits (whether through
  the MCP passthrough or the ACP host)
  apply edits to buffers or disk only
  through the accepted Phase 2 proposal
  routes (`ADR-0016`) and the AI-plane
  proposal flow. The MCP `call_tool_with_permission`
  returns `McpClientError::PermissionRequired`
  when the broker denies the call (the
  contract test at line 2230 asserts
  this); the WS-13.T4 ACP host wraps
  external agent edits in the same
  proposal envelope so the proposal
  service, the capability broker, and
  the evidence ledger all see the
  external agent's edits identically to
  the native agent's edits. The
  containment pre-flight (the
  `validate_containment` path at
  `offline_ai.rs` line 410 plus the new
  `validate_sandbox_profile` path the
  ADR-0038 workstream adds) runs before
  the proposal service emits a proposal
  bundle for external agent edits, so
  the sandbox profile is part of the
  proposal metadata and visible in the
  diff surface.

- **Projection-only UI boundary** —
  `legion-ui` consumes
  `AgentHostStatusProjection` /
  `McpRegistryProjection` /
  `McpToolCallAuditProjection` (the
  future projection family that
  WS-09.T6 / WS-13.T4 add on top of the
  existing `legion-ui` ↔
  `legion-protocol` edge) and emits
  `CommandDispatchIntent` only. UI never
  owns MCP state, never owns the ACP
  host transport, and never owns
  mutation authority. The `legion-ui`
  policy entry at
  `plans/dependency-policy.md` §1 lines
  54-75 already forbids every renderer
  / editor / project / storage / app /
  agent / terminal / security /
  observability / platform edge, and the
  structural audit enforces it. The
  boundary sketch in this ADR
  reinforces this rule with a future
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS`
  audit (no `legion-ui` may declare any
  `rmcp` / `modelcontextprotocol` /
  `agent-client-protocol` runtime
  dependency), shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS`
  audit in `xtask/src/main.rs` and the
  `SEARCH_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `LSP_BOUNDARY_POLICY_MARKERS` /
  `TERMINAL_BOUNDARY_POLICY_MARKERS` /
  `SANDBOX_BOUNDARY_POLICY_MARKERS`
  sketches in ADR-0034 / 0035 / 0036
  / 0037 / 0038.

- **Metadata-first observability** —
  every MCP passthrough call, every ACP
  host spawn, every ACP host tool call,
  every external agent edit proposal,
  every external agent shell execution
  emits a metadata-only observability
  record: server id, tool name, input
  schema hash, capability used,
  principal, correlation id, causality
  id, event sequence, risk label,
  permission decision, sandbox
  process id, exit reason. Raw argv
  and raw network payloads are never
  emitted; only the
  `mcp.tool.metadata` / `acp.tool.metadata`
  projection (server id, tool name,
  input schema hash, capability,
  principal) is emitted, and the raw
  payload is visible only in the local
  UI session. The observability sinks
  that reject zero IDs apply to MCP
  and ACP records the same way they
  apply to terminal / AI / tracker /
  retrieval / sandbox output. The
  WS-09.T6 / WS-13.T4 acceptance shape
  is a metadata-only `McpToolCallAuditProjection`
  / `AcpToolCallAuditProjection`
  (server id / agent id, tool name,
  capability used, principal, sandbox
  id, exit reason) so the existing
  privacy inspector can render the
  external agent's tool surface for
  the user.

- **Fail-closed policy** — the broker
  denies an unknown MCP tool call; the
  broker denies an unknown ACP host
  tool call; the broker denies an
  external agent that bypasses the
  proposal service; the broker denies
  a tool call that the
  `mcp_tool_permission_allows_runtime`
  gate rejects; the broker denies an
  external agent that cannot reach the
  sandbox; a transport compile or
  apply failure is a hard error that
  propagates as `McpClientError::Transport`
  or the equivalent ACP host error
  and the MCP passthrough / ACP host
  call is cancelled. The
  `terminal.kill` capability is the
  fail-closed escape hatch for the
  ACP host shell: a kill signal to
  the ACP host's process group tears
  down the entire process tree
  (composed through the ADR-0038
  OS-sandbox tier), and the
  cancellation reap path (the
  WS-12.T6 acceptance shape) reaps
  any orphaned ACP host processes on
  shutdown.

The plan compared three options: (a) **all
three sequenced** (MCP client parity audit
vs `rmcp` at M0, ACP host at M4,
Legion-as-MCP-server post-GA), (b) MCP
client only (status quo — keep the
hand-rolled transport, skip ACP, skip
Legion-as-server), and (c) replace the
hand-rolled MCP client with `rmcp` now
and ignore ACP. Option (a) is the master
plan's recommendation: it converts
external agent harnesses into supply for
Legion's trust/fleet layer while
preserving the option to migrate to
`rmcp` if spec churn makes the
hand-rolled transport a liability. The
June 2026 MCP spec rev is the planned
trigger; option (a) is what the WS-09.T6
acceptance shape explicitly enables. The
`rmcp` migration is a one-line dependency
swap if the spec rev breaks the
hand-rolled transport, and the migration
is the only thing the parity audit may
uncover — the audit is the deliberate
choice, not the migration. Option (b) is
the current state and §4 rejects it
because it leaves Legion competing with
the harness instead of composing with it.
Option (c) is the spec-churn defensive
choice and is rejected because the
hand-rolled transport is small (one
trait plus two implementations plus one
client plus a permission gate) and is
already exercised by 19 contract tests
in `legion-ai-providers` (16 in
`src/lib.rs` plus 3 in
`tests/prompt_stability.rs`); the
WS-09.T6 acceptance shape explicitly
forbids a forced migration and commits
to the parity audit instead.

## Decision

Legion will execute **all three, sequenced**:
**(1)** an MCP client parity audit vs
`rmcp` and the current MCP spec
(2025-11-25) at M0, migrating to `rmcp`
only if the spec rev breaks the
hand-rolled transport; **(2)** an ACP
host at M4 (WS-13.T4) so Claude Code,
Codex-class, and Gemini-class external
agents run as Legion workers; **(3)**
Legion-as-MCP-server as a post-GA
expansion, never a GA blocker.

- **MCP client parity audit (WS-09.T6,
  M0).** Legion already ships a
  hand-rolled MCP client in
  `legion-ai-providers` (the
  `McpTransport` trait at line 967, the
  `StdioMcpTransport` at line 990, the
  `StreamableHttpMcpTransport` at line
  1118, and the `McpClient<T>` at line
  1156 with the
  `mcp_tool_permission_allows_runtime`
  broker gate at `legion-security` line
  228). The WS-09.T6 workstream runs a
  parity audit of the hand-rolled
  transport against `rmcp` and the MCP
  spec rev 2025-11-25. The audit covers:
  the JSON-RPC envelope validation
  (the `validate_mcp_json_rpc_envelope`
  validator at `legion-protocol` line
  24001), the registry snapshot
  validation (the
  `validate_mcp_registry_snapshot`
  validator at `legion-protocol` line
  24001), the list-changed notification
  semantics (the `McpRegistryReloaded`
  / `StaleMcpRegistry` protocol kinds
  at `legion-protocol` lines 22809 /
  22849), the Streamable HTTP
  transport (the `StreamableHttpMcpTransport`
  at `legion-ai-providers` line 1118),
  the stdio transport (the
  `StdioMcpTransport` at line 990 with
  the `spawn_session` at line 1026 and
  the `send` impl at line 1095), and
  the permission-gated tool call
  (the `call_tool_with_permission` at
  line 1319 with the
  `mcp_tool_permission_allows_runtime`
  gate at `legion-security` line 228).
  The audit records the
  2025-11-25-spec parity for each
  piece and produces a written
  decision: keep the hand-rolled
  transport, or migrate to `rmcp`
  (and only if the spec rev breaks
  the transport). The audit's three
  reference servers (filesystem-class,
  web-class, custom) are the WS-09.T6
  acceptance shape; the audit
  re-runs the five in-source MCP
  contract tests in
  `legion-ai-providers/src/lib.rs`
  plus the protocol DTO test in
  `legion-protocol/tests/dto_contracts.rs`
  line 6472 against the reference
  servers and records the results.
  The audit's permission prompt UX
  (the tool-permission UI consistent
  with the capability broker) is the
  WS-09.T6 acceptance shape for
  §6 row 239 option (a).

- **MCP migration to `rmcp` (M0,
  conditional).** If the parity
  audit determines the spec rev
  breaks the hand-rolled transport,
  the WS-09.T6 workstream migrates
  to `rmcp`. The migration is a
  one-line dependency swap (the
  `McpTransport` trait is the
  boundary; `StdioMcpTransport` /
  `StreamableHttpMcpTransport` /
  `McpClient<T>` re-implement
  against `rmcp`'s transport), the
  protocol DTO surface (the
  `McpServerId` / `McpToolName` /
  `McpRegistrySnapshot` / etc.
  DTOs at `legion-protocol` line
  22544 onwards) is unchanged, the
  broker gate (the
  `mcp_tool_permission_allows_runtime`
  function at `legion-security`
  line 228) is unchanged, and the
  five in-source MCP contract
  tests in `legion-ai-providers`
  continue to pass against the
  new transport. The M0
  ratification does **not**
  authorize a `rmcp` runtime
  dependency today; the
  `legion-ai-providers` ↔
  `legion-ai` ↔ `legion-protocol`
  ↔ `legion-security` edge chain
  is unchanged, and the
  `Cargo.lock` contains zero
  `rmcp` / `modelcontextprotocol`
  dependencies today. The M0
  parity audit is a
  documentation-and-tests change
  that records the decision; the
  migration is a separate gate
  with its own ADR amendment if
  the audit determines the spec
  rev breaks the transport.

- **ACP host (WS-13.T4, M4).**
  Legion will implement the
  Agent Client Protocol host at
  M4 (WS-13.T4) so external
  agents (Claude Code,
  Codex-class, Gemini-class) run
  as Legion workers: their edits
  land as proposals, their shell
  runs in Legion sandboxes, and
  their activity feeds the Legion
  communication stream/evidence
  model. The ACP host is a new
  capability-broker-mediated
  entry point composed through
  the existing `legion-app`
  composition path (the same
  `delegated.allocate_sandbox`
  entry point that the OS-sandbox
  tier in ADR-0038 wraps) so the
  proposal service, the
  capability broker, and the
  evidence ledger all see the
  external agent's edits and
  shell activity identically to
  the native agent's edits and
  shell activity. The WS-13.T4
  workstream extends the Phase 8
  production capability
  reservation set at
  `plans/dependency-policy.md` §1
  line 247 with the ACP-host
  names (for example
  `acp.host.connect`,
  `acp.host.spawn`,
  `acp.host.terminate`) and the
  broker is the ACP-host audit
  point. The WS-13.T4 acceptance
  shape is one external agent
  completes GP-3 inside the
  Legion envelope; the WS-13.T4
  workstream adds the
  `AgentHostStatusProjection` /
  `AcpToolCallAuditProjection`
  family on top of the existing
  `legion-ui` ↔ `legion-protocol`
  edge so the Legion fleet
  console (WS-13.T2) renders
  the external agent's tool
  surface for the user. The M0
  ratification does not extend
  any crate's allowed edges; the
  WS-13.T4 workstream composes
  through the existing
  `legion-app` ↔ `legion-agent` ↔
  `legion-ai` ↔
  `legion-ai-providers` ↔
  `legion-protocol` ↔
  `legion-security` ↔
  `legion-platform` ↔
  `legion-terminal` edge chain.
  The M0 ratification explicitly
  forbids `legion-agent` from
  declaring any ACP runtime
  dependency
  (`agent-client-protocol`,
  `acp-rs`, etc.); the ACP
  host is a `legion-app`
  composition entry point,
  not a `legion-agent`
  re-architecture.

- **Legion-as-MCP-server
  (post-GA).** Legion will
  expose its own tools
  (read/grep/glob/outline from
  WS-10.T1; edit-as-proposal
  from WS-12.T1; terminal from
  WS-05.T5) as MCP primitives
  in a post-GA expansion. The
  post-GA expansion lets
  third-party agents (Claude
  Code, Codex-class,
  Gemini-class) consume
  Legion's tools as MCP servers
  the same way they consume any
  other MCP server. The M0
  ratification does not
  authorize the post-GA
  expansion; the WS-15.T4
  "agent-capability marketplace
  position" workstream is the
  later planning slot for the
  registry schema + local
  install flow, and the
  Legion-as-MCP-server
  implementation is a
  separate gate with its own
  ADR amendment when the
  post-GA expansion activates.
  Legion-as-MCP-server is
  **not** a GA blocker; the
  master plan §4 row 214
  explicitly defers it.

- **Crate boundary.** The
  agent-interop layer is split
  across `legion-ai-providers`,
  `legion-protocol`,
  `legion-security`,
  `legion-ai`, `legion-agent`,
  `legion-app`, `legion-cli`,
  and `legion-desktop` along
  the accepted policy entries
  in `plans/dependency-policy.md`
  §1. `legion-ai-providers`
  owns the MCP client: the
  `McpTransport` trait (line
  967), the `StdioMcpTransport`
  (line 990), the
  `StreamableHttpMcpTransport`
  (line 1118), the `McpClient<T>`
  (line 1156), and the
  `McpClientError` enum (line
  921). The M0 boundary is
  the existing hand-rolled
  transport; the WS-09.T6
  parity audit may migrate to
  `rmcp` (a one-line
  dependency swap) if the spec
  rev breaks the transport.
  The `legion-ai-providers`
  policy entry at
  `plans/dependency-policy.md`
  §1 lines 119-125 authorizes
  `legion-ai`, `legion-protocol`,
  and `legion-security`; the
  M0 ratification does not
  extend `legion-ai-providers`'s
  allowed edges. `legion-protocol`
  owns the MCP DTO surface
  (`McpServerId` / `McpToolName` /
  `McpResourceUri` /
  `McpPromptName` /
  `McpTransportKind` /
  `McpServerDescriptor` /
  `McpToolDescriptor` /
  `McpResourceDescriptor` /
  `McpPromptDescriptor` /
  `McpRegistrySnapshot`) and
  the validators
  (`validate_mcp_json_rpc_envelope`
  / `validate_mcp_registry_snapshot`
  at line 24001). The M0
  ratification does not
  extend the MCP DTO surface;
  the WS-09.T6 parity audit
  may add new DTOs for spec
  rev 2025-11-25 fields
  (notifications, OAuth,
  etc.) but the M0 boundary
  is the existing DTOs.
  `legion-security` owns the
  MCP broker gate
  (`mcp_tool_permission_allows_runtime`
  at line 228); the M0
  ratification does not
  extend the broker surface.
  `legion-ai` owns the
  provider trait layer; the
  M0 ratification does not
  extend `legion-ai`'s allowed
  edges (the §1 lines 111-117
  entry authorizes
  `legion-protocol` and
  `legion-security`). The
  M0 ratification explicitly
  forbids `legion-ai` from
  declaring any MCP runtime
  dependency (`rmcp`,
  `modelcontextprotocol`,
  etc.); the WS-09.T6
  migration is the path that
  authorizes a `rmcp` runtime
  dependency if the spec rev
  breaks the transport.
  `legion-agent` owns the
  agent state machine; the
  M0 ratification does not
  extend `legion-agent`'s
  allowed edges (the §1 line
  144 entry authorizes
  `legion-ai`, `legion-protocol`,
  and `legion-tracker`). The
  M0 ratification explicitly
  forbids `legion-agent`
  from declaring any ACP
  runtime dependency
  (`agent-client-protocol`,
  `acp-rs`, etc.); the
  WS-13.T4 ACP host is a
  `legion-app` composition
  entry point, not a
  `legion-agent`
  re-architecture.
  `legion-app` owns the
  GUI composition: the
  MCP passthrough is
  composed through the
  existing `legion-app` ↔
  `legion-ai` ↔
  `legion-ai-providers` ↔
  `legion-protocol` ↔
  `legion-security`
  edge chain (the same
  chain the WS-12.T1
  tool-registry uses),
  and the WS-13.T4 ACP
  host is composed
  through the same
  `delegated.allocate_sandbox`
  entry point that the
  OS-sandbox tier in
  ADR-0038 wraps. The
  `legion-app` policy
  entry at §1 line 86
  authorizes the full
  app composition set;
  the M0 ratification
  does not extend
  `legion-app`'s
  allowed edges.
  `legion-cli` owns the
  CLI composition:
  the `legion-cli agent
  run --mcp-passthrough=…`
  / `legion-cli workflow
  run --acp-agent=…`
  flags compose through
  the existing
  `legion-cli` ↔
  `legion-app` edge.
  The `legion-cli`
  policy entry at
  §1 line 175 authorizes
  `legion-index`,
  `legion-protocol`,
  and `legion-storage`;
  the M0 ratification
  explicitly forbids
  `legion-cli` from
  declaring any MCP or
  ACP runtime
  dependency, and
  the `xtask` policy
  audit enforces the
  boundary.
  `legion-desktop`
  owns the desktop
  adapter composition:
  the desktop adapter
  launches the MCP
  passthrough / ACP
  host through the
  existing
  `legion-desktop` ↔
  `legion-app` edge,
  and the
  proposal/evidence
  envelope applies
  identically from
  both entry points.
  The `legion-desktop`
  policy entry at
  §1 line 77 authorizes
  `legion-app`,
  `legion-protocol`,
  and `legion-ui`; the
  M0 ratification
  does not extend
  `legion-desktop`'s
  allowed edges.
  `legion-ui` and
  `legion-editor`
  may **not** declare
  any MCP or ACP
  runtime dependency.
  The `legion-ui`
  policy entry at
  §1 lines 54-75
  already forbids
  every renderer /
  editor / project /
  storage / app /
  agent / terminal /
  security /
  observability /
  platform edge, and
  the structural
  audit enforces it.
  The boundary sketch
  in this ADR
  reinforces this rule
  with a future
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS`
  audit (no `legion-ui`
  may declare any
  `rmcp` /
  `modelcontextprotocol`
  / `agent-client-protocol`
  runtime dependency),
  shaped like the
  existing
  `PARSER_BOUNDARY_POLICY_MARKERS`
  audit in
  `xtask/src/main.rs`
  and the
  `SEARCH_BOUNDARY_POLICY_MARKERS`
  /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS`
  /
  `LSP_BOUNDARY_POLICY_MARKERS`
  /
  `TERMINAL_BOUNDARY_POLICY_MARKERS`
  /
  `SANDBOX_BOUNDARY_POLICY_MARKERS`
  sketches in
  ADR-0034 / 0035 /
  0036 / 0037 / 0038.
  The M0 ratification
  does not require the
  agent-interop-boundary
  audit to land today;
  the audit is a
  phase-gate
  improvement that
  becomes useful the
  moment a workspace
  package actually
  declares one of the
  forbidden MCP/ACP
  runtime crates.
  Today, no package
  declares any of
  them, so the audit
  is a
  forward-compatibility
  gate, not a
  regression guard.

- **Compatibility with
  `ADR-0004`
  (actor model +
  async runtime).**
  The MCP passthrough
  is composed through
  the existing actor
  model and the
  existing `tokio`
  async runtime the
  `legion-platform`
  Phase 8 production
  rebaseline already
  uses. The
  `McpClient<T>` is
  an injected
  transport over the
  actor-owned
  scheduling; the
  `StdioMcpTransport`
  uses the existing
  `tokio::process`
  child / stdin /
  stdout handles and
  the existing
  `tokio` async
  runtime. The
  `StreamableHttpMcpTransport`
  uses the existing
  `reqwest` async
  client (the
  `legion-ai-providers`
  `Cargo.toml`
  declares
  `reqwest = { workspace = true }`)
  which the
  `legion-platform`
  Phase 8 production
  rebaseline already
  uses for hosted
  telemetry. The M0
  ratification does
  not add a new
  async runtime; the
  MCP passthrough
  reuses the existing
  one.

- **Compatibility
  with `ADR-0006`
  (AI provider
  abstraction).**
  The MCP passthrough
  is composed
  through the
  existing
  `legion-ai`
  provider trait
  layer, the
  existing
  `legion-ai-providers`
  provider registry,
  and the existing
  `legion-protocol`
  DTO surface. The
  network allowlist
  for the
  `StreamableHttpMcpTransport`
  is resolved
  through the same
  capability broker
  the
  `legion-ai`
  provider
  capability flags
  use (per
  `ADR-0006`), so
  the same provider
  capabilities that
  gate chat and
  completion gate
  the MCP server
  egress. The M0
  ratification does
  not extend the
  provider
  capability flags;
  the WS-09.T6
  parity audit is
  the path that
  extends them if
  the spec rev
  2025-11-25 needs
  new fields.

- **Compatibility
  with the air-gap
  story.** The MCP
  passthrough
  inherits the
  air-gap hard-deny
  from the broker:
  in air-gap mode
  the
  `mcp_tool_permission_allows_runtime`
  broker gate
  denies hosted MCP
  servers, the
  `StreamableHttpMcpTransport`
  is denied, and
  the only allowed
  MCP carrier is
  stdio against
  loopback MCP
  servers (the
  same hard-deny
  the
  `legion-ai`
  provider
  capability flags
  already enforce).
  The offline
  `legion-app` path
  remains the
  air-gap default.
  The hosted
  provider
  activation flow
  from WS-09.T4
  remains the
  consent-gated
  path for MCP
  server egress,
  and the MCP
  passthrough
  inherits the
  air-gap hard-deny
  from the broker.

- **Honest
  spec-churn
  caveats.** The
  June 2026 MCP
  spec rev is the
  planned trigger
  for the `rmcp`
  migration; if
  the spec rev
  breaks the
  hand-rolled
  transport the
  WS-09.T6
  workstream
  migrates. The
  migration is a
  one-line
  dependency swap
  (the
  `McpTransport`
  trait is the
  boundary) and
  the protocol DTO
  surface
  (`McpServerId` /
  `McpToolName` /
  `McpRegistrySnapshot`
  / etc.) is
  unchanged, so
  the migration is
  a transport
  swap, not a
  re-architecture.
  The ACP spec
  itself is younger
  than MCP and
  the spec risk is
  higher; the
  WS-13.T4 workstream
  is gated on a
  stable ACP rev
  and the
  acceptance shape
  is one external
  agent completes
  GP-3 inside the
  Legion envelope.
  Legion-as-MCP-server
  is a post-GA
  expansion and
  the spec risk
  on the server
  side is a
  post-GA concern,
  not an M0
  concern.

## Consequences

- **Positive:**
  the M0
  ratification
  ratifies the
  §6 row 239
  "all three
  sequenced"
  recommendation
  verbatim. The
  three pieces
  compose into a
  single trust
  story: the
  hand-rolled MCP
  client is the
  M0 surface and
  the WS-09.T6
  parity audit
  decides whether
  to keep it or
  migrate to
  `rmcp`; the ACP
  host is the M4
  piece (WS-13.T4)
  that converts
  external agent
  harnesses into
  Legion's supply;
  Legion-as-MCP-server
  is the post-GA
  expansion that
  exposes Legion's
  tools as MCP
  primitives. The
  M0 ratification
  commits to the
  three-piece
  story without
  forcing a `rmcp`
  migration
  today.

- **Positive:**
  the WS-09
  workstream
  (T1 native
  Anthropic
  Messages
  client, T2
  prompt caching
  discipline, T3
  OpenAI Responses
  + compatible
  consolidation,
  T4 hosted-
  provider
  activation
  gates, T5 cost
  & usage
  analytics, T6
  MCP client GA +
  `rmcp` decision)
  has a real
  starting point
  in
  `crates/legion-ai-providers/src/lib.rs`
  (the
  `McpTransport`
  trait at line
  967, the
  `StdioMcpTransport`
  at line 990, the
  `StreamableHttpMcpTransport`
  at line 1118,
  the `McpClient<T>`
  at line 1156,
  the
  `McpClientError`
  enum at line
  921), in
  `crates/legion-protocol/src/lib.rs`
  (the MCP DTO
  surface from
  line 22544
  onwards, the
  validators at
  line 24001), and
  in
  `crates/legion-security/src/lib.rs`
  (the
  `mcp_tool_permission_allows_runtime`
  function at
  line 228). The
  existing
  `legion-ai-providers`
  ↔ `legion-ai` ↔
  `legion-protocol`
  ↔ `legion-security`
  edge chain is
  already
  policy-allowed;
  the WS-09.T6
  parity audit is
  a documentation-
  and-tests
  change, not a
  new crate edge.

- **Positive:**
  the WS-13
  workstream
  (T1 workflow
  runtime
  activation, T2
  fleet console
  UI, T3 approval
  queue & risk
  gates, T4 ACP
  host, T5
  workflow
  review/replay)
  has a real
  starting point
  in
  `crates/legion-app/src/lib.rs`
  (the
  `delegated.allocate_sandbox`
  composition
  entry point at
  line 14714 that
  the OS-sandbox
  tier in
  ADR-0038 wraps,
  the GUI Phase 4
  composition set
  the
  `legion-app`
  policy entry
  at §1 line 86
  authorizes),
  in
  `crates/legion-ai-providers/src/lib.rs`
  (the
  hand-rolled
  MCP client the
  WS-12.T1
  tool-registry
  uses for MCP
  passthrough),
  and in
  `crates/legion-agent/src/lib.rs`
  (the Phase 4
  metadata-only
  agent state
  machine that
  drives the
  external agent
  through the
  `legion-app`
  composition
  entry point).

- **Positive:**
  air-gap mode is
  preserved. The
  MCP passthrough
  inherits the
  air-gap
  hard-deny from
  the broker; the
  hosted MCP
  server egress
  is denied in
  air-gap mode;
  the offline
  `legion-app`
  path remains
  the air-gap
  default. The
  hosted provider
  activation
  flow from
  WS-09.T4
  remains the
  consent-gated
  path for MCP
  server egress,
  and the MCP
  passthrough
  inherits the
  air-gap
  hard-deny from
  the broker.

- **Negative:**
  the spec risk
  on both MCP
  and ACP is
  real, and the
  M0 ratification
  commits to the
  three-piece
  story without
  forcing a
  migration. The
  WS-09.T6
  parity audit is
  the deliberate
  choice; the
  `rmcp`
  migration is a
  one-line
  dependency
  swap if the
  spec rev
  breaks the
  hand-rolled
  transport. The
  §6 row 513
  records this
  as risk R6
  ("MCP/ACP spec
  churn
  (transport rev
  ~June 2026)")
  with mitigation
  "transport
  behind own
  trait; rmcp
  migration
  pre-decided
  (ADR-0039);
  stdio-first",
  severity
  Medium, and
  likelihood
  Low.

- **Negative:**
  the ACP spec
  itself is
  younger than
  MCP and the
  spec risk is
  higher. The
  WS-13.T4
  workstream is
  gated on a
  stable ACP rev
  and the
  acceptance
  shape is one
  external agent
  completes GP-3
  inside the
  Legion envelope.
  The M0
  ratification
  commits to the
  WS-13.T4
  acceptance
  shape, not to
  a specific ACP
  rev.

- **Mitigation:**
  the MCP
  passthrough is
  isolated behind
  the
  `McpTransport`
  trait (line
  967) so the
  `rmcp`
  migration is a
  one-line
  dependency
  swap. The
  protocol DTO
  surface (the
  `McpServerId` /
  `McpToolName` /
  `McpRegistrySnapshot`
  / etc. DTOs at
  `legion-protocol`
  line 22544
  onwards) is
  unchanged
  across the
  migration, so
  the protocol
  layer is
  migration-safe.
  The five
  in-source MCP
  contract tests
  in
  `legion-ai-providers/src/lib.rs`
  (lines 2230 /
  2297 / 2328 /
  2359 / 2403)
  continue to
  pass against
  the new
  transport. The
  protocol DTO
  test in
  `legion-protocol/tests/dto_contracts.rs`
  line 6472
  continues to
  pass. The MCP
  conformance
  suite (the
  WS-09.T6
  acceptance
  shape, 3
  reference
  servers)
  re-runs against
  the new
  transport and
  records the
  results.

- **Positive:**
  the M0
  ratification
  explicitly
  preserves the
  §2.2 invariants
  (app-composed
  and
  capability-gated,
  proposal-mediated
  mutation,
  projection-only
  UI boundary,
  metadata-first
  observability,
  fail-closed
  policy). The
  MCP passthrough
  is broker-gated
  (the
  `mcp_tool_permission_allows_runtime`
  function at
  `legion-security`
  line 228); the
  external agent
  edits are
  proposal-mediated
  (the same
  Phase 2
  proposal routes
  `ADR-0016`
  already gate
  this); the
  external agent
  shell runs in
  Legion
  sandboxes (the
  ADR-0038
  OS-sandbox
  tier); the
  external agent
  activity is
  metadata-only
  observability;
  and the
  fail-closed
  policy applies
  identically to
  the MCP
  passthrough /
  ACP host and
  to the native
  agent. The
  boundary sketch
  in this ADR
  reinforces the
  projection-only
  UI invariant
  with a future
  `AGENT_INTEROP_BOUNDARY_POLICY_MARKERS`
  audit (no
  `legion-ui` may
  declare any
  `rmcp` /
  `modelcontextprotocol`
  /
  `agent-client-protocol`
  runtime
  dependency),
  shaped like the
  existing
  `PARSER_BOUNDARY_POLICY_MARKERS`
  audit in
  `xtask/src/main.rs`
  and the
  `SEARCH_BOUNDARY_POLICY_MARKERS`
  /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS`
  /
  `LSP_BOUNDARY_POLICY_MARKERS`
  /
  `TERMINAL_BOUNDARY_POLICY_MARKERS`
  /
  `SANDBOX_BOUNDARY_POLICY_MARKERS`
  sketches in
  ADR-0034 /
  0035 / 0036 /
  0037 / 0038.

## Verification

- `cargo run -p xtask -- check-deps`
  passes against
  the uncommitted
  working tree at
  baseline
  `b56dcb2`: the
  `legion-ai-providers`
  policy entry at
  §1 lines 119-125,
  the
  `legion-ai`
  policy entry at
  §1 lines
  111-117, the
  `legion-protocol`
  policy entry at
  §1 line 86
  (the
  `legion-app`
  composition
  set), the
  `legion-agent`
  policy entry at
  §1 line 144,
  the `legion-cli`
  policy entry at
  §1 line 175,
  the
  `legion-desktop`
  policy entry at
  §1 line 77, the
  `legion-ui`
  policy entry at
  §1 lines 54-75,
  and the
  `legion-editor`
  policy entry at
  §1 lines 43-52
  are all intact
  and match the
  current
  `Cargo.toml`
  files; zero
  `rmcp` /
  `modelcontextprotocol`
  /
  `agent-client-protocol`
  workspace
  dependencies
  exist in
  `Cargo.lock`
  today; the
  MCP DTO
  surface in
  `legion-protocol/src/lib.rs`
  (the
  `McpServerId` /
  `McpToolName` /
  `McpResourceUri`
  /
  `McpPromptName`
  /
  `McpTransportKind`
  /
  `McpServerDescriptor`
  /
  `McpToolDescriptor`
  /
  `McpResourceDescriptor`
  /
  `McpPromptDescriptor`
  /
  `McpRegistrySnapshot`
  DTOs from
  line 22544
  onwards) and
  the validators
  (the
  `validate_mcp_json_rpc_envelope`
  /
  `validate_mcp_registry_snapshot`
  validators at
  line 24001) are
  intact; the
  broker gate
  (the
  `mcp_tool_permission_allows_runtime`
  function at
  `legion-security`
  line 228) is
  intact.
- `cargo run -p xtask -- docs-hygiene`
  passes against
  the uncommitted
  working tree at
  baseline
  `b56dcb2`.
- `cargo run -p xtask -- no-egui-textedit`
  passes against
  the uncommitted
  working tree at
  baseline
  `b56dcb2`.
- `cargo fmt --all --check`
  passes against
  the uncommitted
  working tree at
  baseline
  `b56dcb2`.
- `cargo test -p legion-ai-providers --tests`
  passes (16/0 in
  `src/lib.rs`
  plus 3/0 in
  `tests/prompt_stability.rs`
  = 19/0 contract
  tests; the
  five MCP
  contract tests
  at lines 2230 /
  2297 / 2328 /
  2359 / 2403 of
  `crates/legion-ai-providers/src/lib.rs`
  are the M0
  contract
  surface).
- `cargo test -p legion-protocol --tests`
  passes (109/0
  contract tests;
  the
  `dto_contracts_automate_mcp_registry_decision_feed_and_risk_rows_are_metadata_only`
  test at line
  6472 of
  `crates/legion-protocol/tests/dto_contracts.rs`
  is the MCP DTO
  contract test).
- `cargo test -p legion-security --tests`
  passes (50/0
  contract tests
  plus 1
  cross-platform
  test = 51/0
  contract tests;
  the two
  `mcp_tool_permission_allows_runtime`
  contract tests
  at lines 2089 /
  2098 of
  `crates/legion-security/src/lib.rs`
  are the broker
  gate contract).
- WS-09.T6
  acceptance
  shape: 3
  reference
  servers
  (filesystem-class,
  web-class,
  custom) pass
  MCP conformance
  against
  spec rev
  2025-11-25;
  permission
  prompts
  audited;
  parity decision
  (keep
  hand-rolled or
  migrate to
  `rmcp`)
  recorded.
- WS-13.T4
  acceptance
  shape: one
  external agent
  completes GP-3
  inside the
  Legion
  envelope; the
  external agent's
  edits land as
  proposals; the
  external agent's
  shell runs in
  Legion
  sandboxes; the
  external agent's
  activity feeds
  the Legion
  communication
  stream/evidence
  model.

## Open Questions / Future Work

- **WS-09.T6 parity audit
  trigger date.** The
  June 2026 MCP spec rev
  is the planned trigger
  for the `rmcp`
  migration; if the spec
  rev lands before WS-09.T6
  runs, the WS-09.T6
  workstream runs the
  parity audit against
  the new spec rev and
  migrates if the spec
  rev breaks the
  hand-rolled transport.
  If the spec rev is
  delayed, the WS-09.T6
  workstream runs the
  parity audit against
  the current spec rev
  (2025-11-25) and
  records the decision.
- **WS-13.T4 ACP rev.**
  The ACP spec itself is
  younger than MCP and
  the spec risk is
  higher; the WS-13.T4
  workstream is gated on
  a stable ACP rev and
  the acceptance shape
  is one external agent
  completes GP-3 inside
  the Legion envelope.
  The M0 ratification
  commits to the
  WS-13.T4 acceptance
  shape, not to a
  specific ACP rev.
- **Legion-as-MCP-server
  post-GA.** The
  post-GA expansion is
  a separate gate with
  its own ADR amendment
  when it activates; the
  WS-15.T4
  "agent-capability
  marketplace position"
  workstream is the
  later planning slot
  for the registry
  schema + local install
  flow.
- **`AGENT_INTEROP_BOUNDARY_POLICY_MARKERS`
  audit.** A future
  phase-gate improvement
  shaped like the
  existing
  `PARSER_BOUNDARY_POLICY_MARKERS`
  audit in
  `xtask/src/main.rs`
  and the
  `SEARCH_BOUNDARY_POLICY_MARKERS`
  /
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS`
  /
  `LSP_BOUNDARY_POLICY_MARKERS`
  /
  `TERMINAL_BOUNDARY_POLICY_MARKERS`
  /
  `SANDBOX_BOUNDARY_POLICY_MARKERS`
  sketches in ADR-0034
  / 0035 / 0036 / 0037
  / 0038. The M0
  ratification does not
  require the
  agent-interop-boundary
  audit to land today;
  the audit is a
  phase-gate
  improvement that
  becomes useful the
  moment a workspace
  package actually
  declares one of the
  forbidden MCP/ACP
  runtime crates.
  Today, no package
  declares any of
  them, so the audit
  is a
  forward-compatibility
  gate, not a
  regression guard.
