# Phase 5 Future Surface Deferral Audit

Phase 5 activates only the isolated plugin runtime boundary. The following remain inactive and require separate ADRs, dependency-policy updates, protocol contracts, runtime tests, and evidence before activation:

- Collaboration runtime
- Remote development runtime
- Terminal execution expansion
- Cloud/provider plugin egress
- Hosted telemetry
- Hosted embeddings and vector retrieval
- Marketplace and VS Code extension compatibility
- Node-based extension execution
- Arbitrary host scripting

Plugin network, process, filesystem, terminal, AI, tracker, memory, collaboration, remote, UI, editor, and project authority remains denied by default. `devil-app -> devil-plugin` is active only as app-owned composition over protocol DTOs and does not grant plugin direct app internals.

Validated by:
- `plans/dependency-policy.md`
- `xtask` dependency checks
- `plugin_network_process_filesystem_and_untrusted_workspace_are_denied`
- `ui_plugin_contributions_are_projection_only_command_intents`
