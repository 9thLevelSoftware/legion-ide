# Tool calling for the local providers — evidence (roadmap 3.1)

Date: 2026-08-17. Roadmap Phase 3 (`plans/legion-production-roadmap-v1.0.md`),
item: *"Implement `ToolCallingProvider` for `OllamaProvider` (native `/api/chat`
tools) and `LlamaCppProvider` (delegate to OpenAI-compatible impl)."*

## What already existed

- `ToolCallingProvider` (`crates/legion-ai/src/tool_calls.rs`) with
  `ToolTurnBlock::MalformedToolCall`, the non-dispatchable variant that carries
  no `input` by construction.
- An implementation for `OpenAiCompatibleProvider` and one for
  `AnthropicMessagesClient` (`crates/legion-ai-providers/src/lib.rs`).
- The tolerant tool-call normalizer (`crates/legion-ai/src/normalize.rs`) and
  the `LEGION_AI_GOVERNORS` measurement seam.
- `OllamaProvider` and `LlamaCppProvider` as `ModelProvider`s only —
  completion and embeddings, no tool calling.
- `legion-bench`'s live runner already reaches a local model, but through
  `OpenAiCompatibleProvider` pointed at Ollama's `/v1` compatibility shim
  (`xtask/src/legion_bench_live.rs`, default endpoint
  `http://127.0.0.1:11434/v1`). So "a local model can drive the loop" was
  already true *via the shim*; what did not exist was either local provider
  implementing the trait.

Two things were inaccurate before this change and are worth naming:

1. `LlamaCppProvider::capabilities()` delegates to its inner
   `OpenAiCompatibleProvider`, so it already advertised `tool_use: true` while
   not implementing `ToolCallingProvider`. The advertisement was not backed by
   anything.
2. `OllamaProvider::capabilities()` advertised `tool_use: false`, which was
   accurate, and is now `true`.

## What was built

**`crates/legion-ai-providers/src/ollama_tools.rs`** (new module) — native
`/api/chat` tool calling. It is a module rather than more lines in `lib.rs`
because that file is over 6,000 lines; the shared schema-grammar builder was
extracted from the OpenAI implementation at the same time, so `lib.rs` gains
the llama.cpp delegation and loses the duplicated grammar block.

Ollama's native endpoint diverges from OpenAI's in four places, each of which
breaks a strict OpenAI parser:

| | OpenAI chat-completions | Ollama `/api/chat` |
| --- | --- | --- |
| assistant message | `choices[0].message` | `message` |
| tool-call arguments | JSON **string** | JSON **object** (parsed server-side) |
| tool-call id | `tool_calls[].id` | none — results correlate by `tool_name` |
| generation limit / stop | `max_tokens`, `finish_reason` (incl. `"tool_calls"`) | `options.num_predict`, `done_reason` (never `"tool_calls"`) |

Consequences that took actual decisions rather than transcription:

- **Ids are synthesized** (`ollama-{index}`) because the agent loop pairs every
  result with the id of the call it answers and Ollama issues none. Positional
  rather than random, so replaying the same response yields the same ids. An id
  a proxy *did* supply is preferred over the synthesized one.
- **Results cite `tool_name`.** Serialization therefore walks the whole
  conversation, recording the name each `ToolUse` used, so a later `ToolResult`
  can name the tool it answers. When no prior call matches the id the field is
  omitted rather than guessed — naming the wrong tool would tell the model a
  call it never made had returned.
- **The stop reason is inferred.** Structured calls present → `ToolUse`;
  otherwise `done_reason: "length"` → `MaxTokens`; otherwise a *recovered*
  call → `ToolUse`; otherwise `EndTurn`. The recovered-call arm exists for the
  same reason it does on the OpenAI path: a model writing its call as prose
  reports `stop`, and reporting `EndTurn` would end the run without dispatching
  what was just recovered.
- **Arguments that never parsed become `MalformedToolCall`,** bounded by the
  existing `bounded_raw_arguments`, exactly as on the OpenAI path. Absent
  arguments are *not* treated as a parse failure — a tool taking no arguments
  is legitimate, and conflating the two would make it undispatchable. A string
  (an OpenAI-shaped proxy in front of Ollama) is parsed; valid JSON that is not
  an object, and any scalar or array, is refused rather than coerced.
- **A missing `function.name` is a hard `RequestFailed`,** not a malformed
  block. Ollama constructs that field itself, so its absence means the response
  shape is wrong, not that the model produced something it could be asked to
  correct — and a `MalformedToolCall` with no legible name gives the model
  nothing actionable.
- **An HTTP 200 carrying an `error` string is a `RequestFailed`** that
  preserves the server's own message, instead of surfacing as "missing
  message".
- The tolerant normalizer runs on the same terms as the OpenAI path, reading
  Ollama's reasoning channel (`thinking`) only when visible content is blank,
  and skipped entirely when structured calls are present so no call is counted
  twice (ADR-0049).
- `LEGION_AI_TOOL_TRANSPORT=schema` is honored, spelled Ollama's way: the
  grammar goes in `format` (inline) rather than `response_format.json_schema`,
  and `tools` is withheld under it. Omitting this would have made the same
  environment variable silently mean different things on two local runtimes.

**`LlamaCppProvider` delegates.** `llama-server` serves the OpenAI dialect,
so `complete_with_tools` forwards to the inner `OpenAiCompatibleProvider`
verbatim — four lines. Delegation is total on purpose: prose recovery,
malformed-block handling, the governors seam and schema transport are exactly
what a small model behind `llama-server` needs, and a parallel implementation
would drift from the OpenAI one the first time either side is fixed. The
response keeps the llama.cpp provider id because the inner provider is
constructed with it.

## Tests

No test here requires a running server; both providers are driven through the
existing `with_transport` seam.

`crates/legion-ai-providers/tests/local_provider_tool_calling.rs` — 11 tests,
the four real cases for **both** providers plus shared behavior:

| Case | Ollama | llama.cpp |
| --- | --- | --- |
| well-formed tool call | `ollama_parses_a_well_formed_native_tool_call` | `llama_cpp_parses_a_well_formed_tool_call_through_the_openai_dialect` |
| no tool call | `ollama_reply_with_no_tool_call_ends_the_turn` | `llama_cpp_reply_with_no_tool_call_ends_the_turn` |
| malformed arguments | `ollama_unparseable_arguments_never_reach_dispatch` | `llama_cpp_unparseable_arguments_never_reach_dispatch` |
| transport failure | `ollama_transport_failure_is_a_provider_error` | `llama_cpp_transport_failure_is_a_provider_error` |

Plus `ollama_advertises_tool_use`,
`ollama_response_of_the_wrong_shape_still_fails_hard`, and
`both_local_providers_recover_a_call_written_as_prose`.

`crates/legion-ai-providers/src/ollama_tools.rs` — 13 unit tests over the wire
shapes, which are the part a compatibility shim would hide:
`payload_uses_ollamas_native_chat_shape`,
`schema_mode_sends_a_format_grammar_and_withholds_tools`,
`tool_results_are_correlated_by_name_because_ollama_has_no_call_ids`,
`an_error_result_carries_the_fact_in_its_text`,
`a_malformed_call_is_never_replayed_to_the_model`,
`object_arguments_parse_into_a_dispatchable_call`,
`a_server_supplied_id_is_preferred_over_a_synthesized_one`,
`absent_arguments_stay_dispatchable`,
`string_arguments_from_an_openai_shaped_proxy_are_parsed`,
`a_missing_function_name_is_a_shape_failure_not_a_model_error`,
`plain_prose_ends_the_turn`, `a_truncated_reply_reports_max_tokens`,
`an_http_200_carrying_an_error_string_is_still_a_failure`.

**Both measurement arms.** The malformed-argument case behaves differently
under `LEGION_AI_GOVERNORS=off` (hard provider error, the pre-port behavior)
than with the governors on (typed non-dispatchable block). The tests read the
seam and assert the contract for the arm they are running in, rather than
assuming a default — the invariant common to both, that unvalidated arguments
never reach dispatch, is asserted either way. Verified by running the suite
under both settings; see the table below.

## Verification

Run on 2026-08-17, Windows 11, exit codes read directly (not through a pipe).

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all` | 0 | clean |
| `cargo test --workspace --all-targets --no-fail-fast` | 0 | **3019 passed / 0 failed / 19 ignored across 263 suites** |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0 warnings, 0 errors |
| `cargo run -p xtask -- extract-before-modify` | 0 | `no chokepoint file grew past its slack` |
| `cargo run -p xtask -- check-deps` | 0 | `dependency policy checks passed` |
| `cargo run -p xtask -- docs-hygiene` | 0 | `documentation hygiene checks passed` |
| `cargo run -p xtask -- claim-audit` | 0 | `claim audit passed` |
| `cargo run -p xtask -- verify-kanban-backlog` | 0 | `10 epic(s), 41 feature(s), 161 task(s)` |
| `cargo run -p xtask -- verify-readiness-consistency` | 0 | `161 backlog task(s) cross-checked` |
| `cargo test -p legion-ai-providers --all-targets` | 0 | 59 unit + 11 `local_provider_tool_calling` + 25 other integration = 95 passed / 0 failed / 1 ignored |
| `LEGION_AI_GOVERNORS=off cargo test -p legion-ai-providers --test local_provider_tool_calling` | 0 | 11 passed / 0 failed |

The workspace count moved from 2745 (the 2026-08-15 tool-normalizer evidence)
to 3019 across the intervening work; this change contributes 24 of them
(13 unit + 11 integration).

## Pre-existing problem found, not introduced here

Five `legion-ai-providers` unit tests fail when `LEGION_AI_GOVERNORS=off` is
set in the environment:

```
tests::openai_malformed_arguments_yield_non_dispatchable_block
tests::openai_recovered_call_with_unparseable_arguments_is_not_dispatchable
tests::openai_recovers_prose_call_under_a_near_miss_name
tests::openai_recovers_tagged_call_written_as_prose
tests::recovered_calls_report_tool_use_even_when_the_model_said_stop
```

They assert the governed behavior unconditionally while the code they exercise
branches on that seam, so they encode an assumption about the environment
rather than about the code. They predate this change and are untouched by it —
the standing gate set does not set the variable, so they are green in CI — but
they are the same class of defect as the three machine-dependent failures
recorded on 2026-08-14, and the bench runner is exactly the thing that sets
this variable. Recorded rather than fixed, because fixing them is a change to
tests outside this task's file list.

## Backlog

**There is no backlog task covering this work.** The kanban backlog
(`plans/kanban/legion-ga-backlog.toml`) has no Phase 3 / local-provider
tool-calling entry: `P4.F1` covers provider activation and policy UX, `P5.F4`
covers the SmallCode reliability port, and neither mentions Ollama or
llama.cpp. Nothing was invented to have something to tick, and no task status
was changed.

## Not claimed

- **Nothing here was run against a real Ollama or a real `llama-server`.** No
  local model runtime is installed on this machine. Every test drives an
  injected transport with a hand-written response body. The request and
  response shapes were taken from Ollama's published API documentation
  (`docs/api.md` in `ollama/ollama`), not observed on the wire. If the shapes
  are wrong, these tests will not say so — they will agree with the mistake.
  A live smoke test against a running Ollama, in the style of
  `openai_tool_calling_live_smoke`, is the missing evidence.
- **The `deterministic-local` fixture default is not retired.** That is a
  separate Phase 3 item; this change makes a local provider *capable* of
  driving the agent loop and nothing selects it yet. `legion-app`'s
  `complete_product_chat` still uses `OllamaProvider` for chat completion only,
  and the Delegate path is still handed whatever `ToolCallingProvider` the
  caller supplies. PR-AI-001's fixture caveat stands.
- **No bench numbers.** The raw-versus-governed comparison
  (`plans/evidence/production/BENCH/baseline-raw-v1.md`, backlog `P9.F1.T4`)
  remains unmeasured, and nothing here changes that.
- **The `LEGION_AI_TOOL_TRANSPORT=schema` path for Ollama is unit-tested only
  at the payload level.** `format` is asserted to carry the grammar and `tools`
  to be withheld; that Ollama then honors the grammar is not demonstrated here.
- **No readiness-ledger row was moved.** Advertising `tool_use: true` on
  `OllamaProvider` is a statement about the adapter, not about a validated
  product workflow.
