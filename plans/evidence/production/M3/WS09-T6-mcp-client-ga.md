# WS09.T6 MCP Client GA Evidence

Date: 2026-06-12
Kanban card: `t_c6bd115d`
Scope: MCP client GA decision, conformance coverage, permission prompt audit

## Decision
Keep the current hand-rolled MCP client/transport layer for now; do not migrate to `rmcp`.

Reason: the current implementation already passes the required local conformance checks for the three reference server classes exercised in this workspace, and the permission prompt path is already enforced through the security-owned MCP tool permission helpers. The only production adjustment needed for the HTTP transport was installing the rustls crypto provider before issuing blocking HTTP requests.

## Changes
- `crates/legion-ai-providers/src/lib.rs`
  - Added rustls crypto-provider initialization before Streamable HTTP requests.
- `crates/legion-ai-providers/Cargo.toml`
  - Added direct `rustls` dependency for the transport initialization helper.
- `crates/legion-ai-providers/tests/mcp_ga_conformance.rs`
  - Added three conformance tests:
    - filesystem-class stdio reference server
    - web-class streamable HTTP reference server
    - custom stdio reference server with list-changed reloads
  - Added permission prompt audit assertions using the security-owned MCP permission helpers.

## Verification
- `cargo test -p legion-ai-providers --all-targets -- --nocapture` ✅
- `cargo fmt --all --check` ✅

## Notes
- The HTTP transport needed explicit rustls provider installation because this workspace uses `reqwest` with `rustls-no-provider`.
- The conformance fixture covers tools, resources, prompts, tool calls, and list-changed reload behavior.
