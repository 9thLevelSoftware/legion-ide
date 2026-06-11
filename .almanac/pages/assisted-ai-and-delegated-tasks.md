---
title: Assisted AI And Delegated Tasks
summary: "Assisted AI, provider routing, and delegated-task execution are app-gated surfaces that depend on product mode, permission decisions, and metadata-only context rather than ambient runtime authority."
topics: [ai-runtime, flows, decisions, architecture]
sources:
  - id: app-lib
    type: file
    path: crates/legion-app/src/lib.rs
    note: Defines `AppProductMode` and composes AI, agent, provider, memory, and tracker surfaces.
  - id: ai-lib
    type: file
    path: crates/legion-ai/src/lib.rs
    note: Defines provider routing and assisted-AI request abstractions.
  - id: providers-lib
    type: file
    path: crates/legion-ai-providers/src/lib.rs
    note: Defines local and compatible provider slots plus MCP client support.
  - id: control-trust-tests
    type: file
    path: crates/legion-app/tests/control_trust_surfaces.rs
    note: Verifies product-mode and proposal-lifecycle controls for assisted AI.
  - id: delegated-task-tests
    type: file
    path: crates/legion-app/tests/delegated_task_integration.rs
    note: Verifies delegated-task permission gating, proposal output, and metadata-only chat citations.
status: active
verified: 2026-06-08
---
`AppProductMode` is the first AI gate. `Manual` rejects AI dispatch. `Assist` enables inline assist and proposal/explain flows. `Delegate` adds delegated-task execution, and `Automate` widens automation surfaces further [@app-lib]. This means "AI is compiled in" and "AI is allowed right now" are different facts.

## Provider and routing shape

`[[crates/legion-ai/src/lib.rs]]` defines the abstractions for chat, embeddings, inline prediction, route decisions, and capability-aware provider requests [@ai-lib]. `[[crates/legion-ai-providers/src/lib.rs]]` supplies concrete slots such as deterministic local, Ollama, llama.cpp, OpenAI-compatible, and Copilot NES, plus MCP client support [@providers-lib]. The provider layer still sits behind app-owned policy, security, and product-mode routing [@app-lib].

## Delegated tasks are approval-gated

`[[crates/legion-app/tests/delegated_task_integration.rs]]` shows the intended delegated-task flow:

- a missing plan is a structured outcome, not a panic [@delegated-task-tests]
- delegate execution waits for tool permission before allocating a sandbox [@delegated-task-tests]
- denied permission keeps deny precedence and fails closed [@delegated-task-tests]
- explicit allow can produce a proposal-ready output, but the result is still a proposal payload rather than an autonomous write [@delegated-task-tests]

The same test file also shows delegate chat projecting citations with file path, byte range, and chunk hash while omitting raw-source payloads [@delegated-task-tests]. That ties this surface back to [[metadata-only-observability-and-storage]] and [[text-snapshots-and-degraded-mode]].

## Assisted AI is controlled like proposals

`[[crates/legion-app/tests/control_trust_surfaces.rs]]` verifies that manual mode rejects assisted AI dispatch and that app-owned projections, proposal rows, and redaction hints stay in place when AI features are available [@control-trust-tests]. The repository treats assisted AI as another controlled workflow layered on top of existing proposal, audit, and workspace boundaries, not as a side channel around them.
