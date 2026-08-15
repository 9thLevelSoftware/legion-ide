# SmallCode Attribution and Provenance

Legion ports behaviors from SmallCode
(https://github.com/Doorman11991/smallcode, MIT License, Copyright (c) 2026
Doorman11991) under the rule in ADR-0049: **reuse semantics and tests;
reimplement authority**. No SmallCode source code is compiled into Legion
binaries. This file tracks exactly what has been taken, from where, and when.
See `THIRD_PARTY_NOTICES.md` at the repo root for the full MIT license text.

Legend for "what was taken":

- **semantics** — decision logic re-implemented in Rust from documented behavior
- **test vectors** — input/expected-output cases derived from SmallCode tests or source
- **nothing yet** — port planned, no material taken so far

## Provenance table

| SmallCode file(s) | Legion destination | What was taken | Date / status |
| --- | --- | --- | --- |
| `src/tools/tool_call_extractor.js`, `src/tools/liquid_tool_parser.js`, `test/liquid_tool_parser.test.js` | `crates/legion-ai/tests/fixtures/smallcode_vectors/tool_call_vectors.jsonl` | test vectors (tagged/fenced/bare/Liquid tool-call recovery cases) | extracted 2026-08-15 |
| `src/tools/tool_aliases.js`, `test/tool_aliases.test.js` | `crates/legion-ai/tests/fixtures/smallcode_vectors/tool_call_vectors.jsonl` | test vectors (alias and argument-name remapping cases) | extracted 2026-08-15 |
| `bin/executor.js` (`patch` tool: exact-unique-match `old_str`/`new_str` semantics) | `crates/legion-ai/tests/fixtures/smallcode_vectors/patch_vectors.jsonl` | test vectors (patch apply/failure taxonomy) | extracted 2026-08-15 |
| `src/tools/tool_call_extractor.js`, `src/tools/liquid_tool_parser.js` | `crates/legion-ai/src/normalize.rs` (`extract_tool_calls`) | semantics — priority-ordered tagged/Liquid/fenced/bare scanning, call-shape normalization, narrow trailing-comma repair, span stripping. Re-implemented in Rust from documented behavior; no source copied. | ported 2026-08-15 |
| `src/tools/tool_aliases.js` | `crates/legion-ai/src/normalize.rs` (`normalize_alias`, `resolve_against_known`) | semantics — tool-name and argument-name canonicalization, directory-listing alias reshaping. Legion applies it only when the written name does not match an offered tool. | ported 2026-08-15 |
| `src/tools/two_stage_router.js`, `src/session/action_classifier.js` | `legion-ai` routing | nothing yet (semantics port planned) | planned |
| `src/model/profiles.js`, `src/model/adaptive_router.js`, `src/model/thinking_budget.js`, `src/model/router.js` | model profiles extending `AssistedAiCapabilityMatrix` in `legion-protocol` | nothing yet (semantics adaptation planned) | planned |
| `src/governor/early_stop.js`, `src/governor/quality_monitor.js` | `legion-agent` loop governors | nothing yet (semantics port planned) | planned |
| `src/tools/read_tracker.js`, `src/tools/dedup.js`, `src/tools/trust_decay.js` | `legion-agent` loop state | nothing yet (semantics port planned) | planned |
| `src/session/plan_tracker.js`, `src/session/dependency_graph.js`, `src/session/contract.js` | `legion-agent` plan anchoring (`legion-protocol/src/plan.rs`) | nothing yet (semantics adaptation planned) | planned |
| `bin/executor.js` (`patch` tool semantics) | `legion-ai` patch resolution feeding `TextEditProposal` / `WorkspaceEditProposalPayload` | nothing yet (semantics adaptation planned) | planned |
| `src/tools/hybrid_search.js` | `legion-index` (ranking heuristics only) | nothing yet (semantics adaptation planned) | planned |

Modules rejected in ADR-0049 (executor/shell authority, JS plugins, TUI,
`bin/model_client.js` transports, MarrowScript embedding, snapshot/rollback
store, cloud auto-escalation) take nothing and are not listed above; the
rejection rationale lives in the ADR.

## Fixture provenance headers

Every fixture file under
`crates/legion-ai/tests/fixtures/smallcode_vectors/` begins with the header:

> Derived from SmallCode (https://github.com/Doorman11991/smallcode), MIT
> License — see THIRD_PARTY_NOTICES.md

JSONL files carry it as a first record `{"_license": "..."}`. Records
synthesized by Legion (categories SmallCode's tests do not cover) are marked
`"source": "synthetic"`; all other records name the SmallCode file they were
derived from.

## Maintenance rule

When a planned port lands, change its row from "planned" to "ported
YYYY-MM-DD" and note the Legion file(s). When new vectors are extracted, add
or update the extraction rows with the date.
