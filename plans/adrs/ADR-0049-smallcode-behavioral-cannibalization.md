# ADR-0049: SmallCode Behavioral Cannibalization

## Status

Accepted

## Context

SmallCode (https://github.com/Doorman11991/smallcode) is an MIT-licensed
Node.js coding agent purpose-built for small local models (8B-35B parameters):
small-context tool routing, a tolerant multi-format tool-call parser, plan
anchoring against multi-turn drift, and patch-first (search/replace) editing
with an exact-unique-match rule. It is a coping layer for unreliable model
output, evolved against real local-model failure modes, with a test suite that
encodes those failure modes as vectors.

Legion has the opposite half: the authority substrate. Proposals, the
capability broker, sandboxing, terminal policy, and provider boundaries
already exist and are tested. Legion's AI loop lacks exactly the tolerant
decision logic SmallCode has proven.

Research (2026-08 course correction) recommends behavioral cannibalization,
not code merge: SmallCode's runtime is JavaScript with its own executor,
persistence, TUI, and plugin model, all of which duplicate or bypass Legion's
authority surfaces.

## Decision

### 1. Port rule

**Reuse semantics and tests; reimplement authority.** Legion ports SmallCode's
decision logic and test vectors as Rust re-implementations from documented
behavior. All effects terminate in Legion's existing ports: proposal system,
capability broker, sandbox, terminal, and provider boundaries. No JS runtime,
no SmallCode executor or plugin model, no second persistence stack.

### 2. Port map

| SmallCode module(s) | Legion destination | Action | Rationale |
| --- | --- | --- | --- |
| `src/tools/tool_call_extractor.js`, `src/tools/liquid_tool_parser.js`, `src/tools/tool_aliases.js` | `legion-ai` tool-call normalizer | Port | Tolerant recovery of tagged/fenced/bare/Liquid tool calls and alias/arg-name remapping is pure parsing; no authority involved. |
| `src/tools/two_stage_router.js`, `src/session/action_classifier.js` | `legion-ai` routing | Port | Deterministic category scoring and query/mutate classification cut prompt tokens and gate write tools; regex logic ports directly. |
| `src/model/profiles.js`, `src/model/adaptive_router.js`, `src/model/thinking_budget.js` (plus `src/model/router.js` complexity heuristic) | Model profiles extending `AssistedAiCapabilityMatrix` in `legion-protocol` | Adapt | Per-model capability/context/tool-format profiles map onto the existing capability matrix; adaptive selection stays advisory. |
| `src/governor/early_stop.js`, `src/governor/quality_monitor.js` | `legion-agent` loop governors | Port | Repetition/patch-spiral/empty-response detection is waste containment; emits corrections, executes nothing. |
| `src/tools/read_tracker.js`, `src/tools/dedup.js`, `src/tools/trust_decay.js` | `legion-agent` loop state | Port | Session-scoped read-before-write tracking, pure-tool dedup, and per-tool demotion counters; no side effects of their own. |
| `src/session/plan_tracker.js`, `src/session/dependency_graph.js`, `src/session/contract.js` | `legion-agent` plan anchoring | Adapt | Plan capture/re-injection, file-overlap dependency batching, and definition-of-done assertions; protocol plumbing exists in `legion-protocol/src/plan.rs`. |
| Patch-first edit semantics (`patch` tool in `bin/executor.js`: exact-unique-match `old_str`/`new_str`) | `legion-ai` patch resolution feeding `TextEditProposal` / `WorkspaceEditProposalPayload` | Adapt | The exact-one-match rule and its failure taxonomy port; writes terminate in Legion proposals, never in direct filesystem access. |
| `src/tools/hybrid_search.js` | `legion-index` (ranking heuristics only) | Adapt | BM25 + bag-of-words hybrid scoring informs ranking; Legion's index owns storage and traversal. |
| `bin/executor.js`, `src/tools/shell_session.js` (execution/shell authority) | — | Reject | Legion's sandbox, terminal, and capability broker own all effects. |
| JS plugin model (`src/plugins/`, `extensions/`) | — | Reject | Legion's signed WASM plugin runtime owns extensibility (ADR-0047). |
| TUI (`src/tui/`, `bin/tui.js`) | — | Reject | Legion has its own UI layer. |
| `bin/model_client.js` transports | — | Reject | Provider boundaries exist in `legion-ai-providers`. |
| MarrowScript embedding (`marrow/`, `src/compiled/`) | — | Reject | No second cognition/codegen layer; behaviors worth keeping are ported as plain Rust. |
| Snapshot/rollback store (`src/session/snapshot.js`, `src/session/undo.js`) | — | Reject | Legion proposals and worktrees own edit history and rollback. |
| Cloud auto-escalation (`bin/escalation.js`) | — | Reject | Silent escalation to cloud models is replaced by explicit `NeedsStrongerModel` consent events; no automatic egress. |

### 3. Classification: waste containment, not autonomy

The loop governors (dedup, read-before-write, early-stop, plan anchoring) are
**waste-containment and safety mechanisms**, not agent-autonomy expansion.
They make a bounded loop stop earlier, repeat less, and destroy less. This is
recorded so master plan section 5.3 anti-scope rule 5 ("do not optimize agent
autonomy before proposal review is excellent") is honored by construction:
nothing in this ADR increases what the agent may do, only how little it wastes
while doing it.

### 4. Bench holdout policy

The Legion bench task corpus keeps a held-out subset that is never run during
development. Held-out tasks are scored only at phase exits, so ported
heuristics cannot overfit the corpus they are graded on.

### 5. Attribution mechanics

- `THIRD_PARTY_NOTICES.md` at the repo root carries the SmallCode MIT notice
  and full license text.
- `docs/legal/smallcode-attribution.md` tracks per-module provenance (what was
  taken, where it landed, when).
- Ported test vectors carry provenance headers naming SmallCode and the MIT
  license.
- Ports are re-implementations from documented behavior. Wherever test vectors
  or algorithmic structure are taken substantially verbatim, the MIT notice is
  preserved.

## Consequences

- Legion's AI loop gains field-proven small-model coping behavior without
  adopting a JS runtime, a second executor, or a second persistence stack.
- Every ported behavior lands behind existing authority boundaries, so the
  security review surface does not grow.
- SmallCode's test vectors become fixture data in Legion crates, giving each
  port an acceptance suite before any Rust is written.
- Rejected modules are recorded with rationale, preventing future "port the
  rest" drift without an ADR amendment.
- Attribution obligations are discharged mechanically (notices file, legal
  doc, fixture headers) rather than ad hoc.

## References

- SmallCode repository: https://github.com/Doorman11991/smallcode (MIT)
- `THIRD_PARTY_NOTICES.md`, `docs/legal/smallcode-attribution.md`
- `crates/legion-ai/tests/fixtures/smallcode_vectors/` (extracted vectors)
- `crates/legion-protocol/src/plan.rs`, `crates/legion-protocol/src/capability.rs`
- Master Plan v0.2 section 5.3 (anti-scope rules)
- ADR-0046: Surface expansion freeze
