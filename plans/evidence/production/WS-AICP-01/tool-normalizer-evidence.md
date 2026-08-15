# Tool-call recovery for small local models — evidence (P5.F4.T1/T2)

Date: 2026-08-15. Roadmap Phase 2.2 (`plans/legion-production-roadmap-v1.0.md`),
governed by `plans/adrs/ADR-0049-smallcode-behavioral-cannibalization.md`.

## Problem

Small local models routinely emit tool calls as *prose* rather than as
structured provider fields — wrapped in `<tool_call>` tags, inside a fenced
code block, in Qwen-style `<|tool_call_start|>` Liquid syntax, or under
near-miss names (`Read` for `read_file`). Legion's providers parsed strictly,
so all of that was discarded and the turn ended having made no call.

Separately, a single unparseable `arguments` string failed the **entire**
completion with `ProviderError::RequestFailed`, ending the run with no way for
the model to correct itself. A test asserted that fail-hard behavior.

## What landed

**`crates/legion-ai/src/normalize.rs`** — extraction and alias
canonicalization as two independent stages (mirroring SmallCode's own split
between `tool_call_extractor.js` and `tool_aliases.js`):

- Priority-ordered scanning — tagged, Liquid, fenced, bare — where only the
  winning source is consumed, so a tagged call is never double-counted with a
  fenced restatement of it.
- Narrow repair by design. Trailing commas are stripped because they are
  unambiguous. Single-quoted keys, Python literals (`True`/`None`) and
  truncated JSON are **rejected**: guessing at those risks fabricating a call
  the model never made, which is a worse failure than recovering nothing.
- Bare JSON counts only as the whole message — `"Here is the call: {...}"` is
  the model describing an intent, not issuing one.
- Objects concatenated on one line are rejected while newline-separated
  objects are recovered, because only the latter has an unambiguous delimiter.
- Calls naming a tool the model was not offered are dropped.
- Consumed spans are stripped from the residual prose, so recovery never
  leaves a call restated as text.

**Alias resolution targets Legion's own registry.** Candidates are tried in
order and the first the registry offers wins: the name as written, then
SmallCode's canonical name, then Legion's native name (`read`, `grep`, `glob`,
`outline`, `edit-as-proposal`, `terminal-command`). A literal match always
wins first, so a registry exposing `shell` keeps receiving `shell` rather than
being rewritten to `bash`.

The third step is the one that matters in production and was missing in
review: the delegated loop offers Legion's names, while SmallCode's
vocabulary canonicalizes `Read` to `read_file` — a tool Legion does not have.
Without a Legion-native mapping the alias layer was inert exactly where it was
supposed to help, and the first test written for it passed only because it
constructed a registry offering `read_file`. Arguments are renamed onto the
target tool's keys (`file_path` → `path`, `cmd` → `command`, `new_string` →
`replacement`), and directory listings reshape rather than rename
(`ls(path)` → `glob(pattern)`).

**A recovered call is reported as a tool-use turn.** A model writing its call
as prose reports `finish_reason: "stop"`, because the provider only saw text.
Mapping that to `EndTurn` would make the loop finish the run without
dispatching what was just recovered — leaving the whole feature inert. The
provider therefore reports `ToolUse` whenever recovery produced a call or a
malformed block, and plain prose still ends the turn.

**Non-dispatchable malformed calls.** `ToolTurnBlock::MalformedToolCall`
carries `raw_arguments` and a diagnostic but deliberately has **no `input`
field** — an unparsed call is not dispatchable, and making that a type-level
fact means no future caller can execute one by accident. Adding the variant
made the compiler flag both wire serializers, which now skip it: replaying a
broken `tool_use` would oblige a `tool_result` for an id the model never
really issued.

This holds for **both** sources of a malformed call. `ExtractedToolCall`
distinguishes "arguments absent" from "arguments present but unparseable" via
`arguments_unparsed`, so a *recovered prose* call whose nested argument string
fails to parse becomes a malformed block too, rather than entering dispatch
with a null input. Collapsing both cases to `Value::Null` would have made a
legitimately argument-less call and a broken one indistinguishable.

**Loop feedback.** `run_delegated_task_loop` reports the diagnostic back as
text (not a `tool_result`, which would dangle without a matching `tool_use`),
counts it against `max_consecutive_retries`, and audits it with reason
`malformed_tool_arguments`. A model that can only emit broken JSON is stopped
by the retry budget instead of draining the turn budget.

## Authority unchanged

Recovery ends at the typed boundary. A recovered call is still only a
*request*: the capability broker authorizes it, scope containment applies, and
mutations remain proposal-mediated. Nothing here widens what an agent may do —
consistent with ADR-0049's classification of this work as reliability, not
autonomy expansion (master plan §5.3).

## Verification

| Check | Result |
| --- | --- |
| `cargo test -p legion-ai --test tool_call_corpus` | 3/3 — **58/58 corpus vectors** recovered exactly or safely rejected (roadmap bar: ≥99%) |
| Provider contract coverage (near-miss name, unparseable recovered arguments, prose recovery, malformed structured call, transport-shape failure, stop-reason) | 6 tests in `legion-ai-providers` |
| **End-to-end provider→loop contract** (`legion-agent --test openai_tool_loop_cross_check`) | 3 passed — prose `Read` dispatches as Legion's `read`; malformed prose call is audited then corrected |
| `cargo test -p legion-ai` | 61 passed / 0 failed |
| `cargo test -p legion-ai-providers` | 68 passed / 0 failed |
| `cargo test -p legion-agent --test agent_loop_integration` | 14 passed / 0 failed |
| `cargo test --workspace --all-targets` | **2729 passed / 0 failed / 250 suites** |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |

Corpus coverage by category: xml_tagged 7, alias 18, liquid 11, fenced_json 4,
bare_json 3, unknown_tool 3, concatenated_objects 2, trailing_comma 2,
python_literals 2, reasoning_fallback 2, single_quoted_keys 1, truncated_json
1, priority 1, passthrough 1.

Beyond the corpus: structural fuzzing over every prefix of 18 adversarial
seeds (unterminated tags, half-open fences, lone backslashes, brace soup)
asserting no panic and no empty-named call, plus unicode boundary cases.

**Corpus coverage alone did not prove the production path — and reviewers were
right to say so.** The 18 alias vectors exercise `normalize_alias` directly,
so a green corpus said nothing about whether a near-miss name survives a real
delegated run. Four gaps were found in review and closed here, none of which
the corpus could have caught:

1. alias canonicalization never reached the provider filter;
2. it then targeted SmallCode's names rather than Legion's registry, so it
   remained inert even once wired;
3. recovered calls with unparseable arguments entered dispatch with a null
   input;
4. a recovered call reported `EndTurn`, so the loop finished before
   dispatching it.

The lesson is recorded rather than papered over: a pure-layer corpus proves
the parser, not the feature. The binding evidence is now the end-to-end
contract in `crates/legion-agent/tests/openai_tool_loop_cross_check.rs`, where
a real `OpenAiCompatibleProvider` drives the real loop over a scripted
transport: a model writing `<tool_call>{"name":"Read",...}</tool_call>` with
`finish_reason: "stop"` results in an audited dispatch of Legion's `read`
tool, and one with unparseable arguments produces a
`malformed_tool_arguments` audit followed by a successful corrected call.

## Replaced test, preserved invariant

`openai_malformed_arguments_returns_provider_error` asserted that malformed
arguments fail the whole completion. Its *safety* intent — unvalidated
arguments must never reach dispatch — is now carried by the type system and by
three replacement tests: no dispatchable use may come from unparseable
arguments; a response whose *shape* is wrong is still a hard provider error;
and a prose-embedded call is recovered exactly once with surrounding prose
intact.

## Incidental fix

The bench live-runner tests named their fixture directories from a clock
alone. Windows' clock is coarse enough that two tests starting together shared
a directory and deleted each other's fixture on cleanup — a real flake that
surfaced only under full-workspace parallel load. Names now carry a counter.

## Not claimed

This work is measured by unit and corpus tests, **not** by live-model runs. No
local model runtime is installed, so the raw-versus-governed bench comparison
(`plans/evidence/production/BENCH/baseline-raw-v1.md`, backlog `P9.F1.T4`)
remains unmeasured. Do not read this evidence as a demonstrated end-to-end
task-success improvement.
