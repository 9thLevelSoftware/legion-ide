# Patch-first editing in the delegated loop — evidence (P5.F4.T3)

Date: 2026-08-15. Roadmap Phase 2.1
(`plans/legion-production-roadmap-v1.0.md`), governed by
`plans/adrs/ADR-0049-smallcode-behavioral-cannibalization.md`.

## Problem

`edit-as-proposal` accepted only `replacement` — the file's complete new
content. Small models overwhelmingly express edits as *fragments* ("replace
this with that") because reproducing a whole file costs tokens they do not
have and invites truncation. Legion had no way to accept that, so the model's
options were to rewrite the file wholesale or fail.

The tool-call recovery work (P5.F4.T1) made this concrete and dangerous: the
alias table briefly mapped `str_replace(old_string, new_string)` onto
`edit-as-proposal(replacement)`, which would have proposed replacing an entire
file with the new fragment. That mapping was withdrawn pending exact-match
semantics; this packet supplies them and restores it safely.

## What landed

**`crates/legion-ai/src/patch.rs`** — exact-match resolution of a fragment
against real file content:

- **Exact and unique, or refuse.** Zero matches is a no-match; two or more is
  ambiguous. Neither guesses, because picking one of two candidate sites edits
  the wrong line. Occurrences are counted **overlapping**, since
  `str::matches` reports `"aa"` in `"aaa"` once while it in fact starts at two
  positions — applying that under a uniqueness guarantee that does not hold.
- **An empty anchor never overwrites a file.** `old_str: ""` is meaningful only
  for a file that does not exist yet. Against existing content it would mean
  "replace everything", so a model attempting an insertion with a blank anchor
  is refused and pointed at `replacement` rather than silently destroying the
  file.
- **Whitespace and line-ending drift do not match.** A tab-vs-spaces or
  CRLF-vs-LF near-miss is refused, since "close enough" on indentation is how
  a patch lands in the wrong scope. Both are called out by name in the
  diagnostic, because they look identical in a transcript.
- **Refusals locate the problem.** Each carries the nearest candidate line and
  a similarity score plus an instruction to re-read and quote exactly. Without
  that a model can only escalate to rewriting the file — the outcome this
  layer exists to prevent.
- **Type errors are distinguished from lookup failures.** A missing or
  non-string `new_str` is a validation error, not a no-match; the model needs
  to know which it did.
- Block parsing for the formats models emit: conflict-style
  `<<<<<<< SEARCH` / `=======` / `>>>>>>> REPLACE`, and ```diff hunks. A block
  missing its divider or terminator yields nothing — a half-written edit is
  not a partially valid one, and applying its visible half would truncate the
  file.

**Block-format edits are recovered as edit calls.** A model that writes a
SEARCH/REPLACE block with no tool call at all — the format several model
families emit unprompted — would otherwise have its edit read as prose and
lost. Recovery runs last in the extraction order, so a real tool call always
wins over a block restatement of it, and only when the registry actually
offers an edit tool, so discussing a diff is not mistaken for requesting one.

**Successive edits to one file compose.** A run may edit the same file twice.
The loop keeps a per-run overlay of content already staged by earlier
proposals, and a fragment resolves against that rather than the untouched
worktree. Without it the second proposal silently omits the first edit, and
both carry preconditions for the same original file — so applying either makes
the other stale. The overlay is also the resolution *source*, not a cache: a
fragment can anchor on text an earlier edit introduced.

**Loop integration.** `execute_edit_as_proposal` accepts either form:
`replacement` for whole content, or `old_str`/`new_str` resolved against the
file in the worktree. Every fragment failure returns retryable
`InvalidArguments` feedback rather than terminating the run, so a model that
quoted text slightly wrong fixes it on the next turn.

**Schema.** `edit-as-proposal` now advertises `old_str`/`new_str` with
descriptions stating the exact-once requirement — a capability the model
cannot use if it is not told it exists. `required` drops to `["path"]`,
because a flat list cannot express "one of these two forms"; the executor
checks the pair and explains the valid shapes.

**Alias restoration.** `str_replace`, `Edit`, `patch` and friends map to
`edit-as-proposal` again, now preserving fragment semantics: `old_string` →
`old_str`, `new_string` → `new_str`. Whole-file forms (`content`, `text`)
still map to `replacement`. The fragment/whole-file decision is keyed on the
*arguments*, not the tool name, since the name is the least reliable thing a
small model produces.

## Authority unchanged

Edits remain proposals. The loop writes nothing: resolution reads the file to
compute the new content, and the result goes through
`DelegatedTaskProposalGenerator` for human review. `fragment_edit_replaces_only_the_matched_text`
asserts the on-disk file is byte-identical after the run.

## Verification

| Check | Result |
| --- | --- |
| `cargo test -p legion-ai --test patch_resolution` | 3/3 — **18/18 patch vectors** applied exactly or refused as specified |
| `cargo test -p legion-ai` | 86 passed / 0 failed |
| `cargo test -p legion-agent --test agent_loop_integration` | 19 passed / 0 failed (5 new fragment-edit cases) |
| `cargo test -p legion-agent --test openai_tool_loop_cross_check` | 4 passed / 0 failed (incl. block-format edit end to end) |
| `cargo test -p legion-agent --test tools_schema` | 3 passed / 0 failed |
| `cargo test --workspace --all-targets` | **2769 passed / 0 failed / 251 suites** |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |

End-to-end coverage in the delegated loop, not just the pure layer:

- `fragment_edit_replaces_only_the_matched_text` — the proposal carries the
  whole file with only the fragment changed, and the file on disk is untouched
- `ambiguous_fragment_is_refused_then_corrected` — an ambiguous fragment is
  refused, the diagnostic reaches the model, and its disambiguated retry
  succeeds within the same run
- `unmatched_fragment_is_refused_with_a_locating_diagnostic` — the refusal
  names the nearest line, and the run continues
- `block_format_edit_written_as_prose_reaches_the_edit_tool` — a raw
  SEARCH/REPLACE block, through the real provider and loop, produces a
  proposal whose content was resolved by exact match rather than taken whole
- `successive_fragment_edits_to_one_file_compose` — the second proposal carries
  both edits rather than omitting the first
- `a_fragment_can_anchor_on_text_introduced_by_an_earlier_edit` — the overlay
  is the resolution source, not merely a cache

Adversarial coverage: every prefix of a block-format seed (mid-marker
truncation) and a matrix of empty/CRLF/unicode/oversized fragments, asserting
no panic. Diagnostic construction is bounded — a 200K-character single-line
file against a 50K-character anchor builds its refusal in well under a second,
because the failure path compares bounded samples rather than whole lines.

## Not claimed

Two pieces of Phase 2.1 remain open and are **not** delivered here:

- The **Assist path** still inserts model text at `TextRange::byte(0, 0)`
  (`legion-app`). Fixing it needs the extract-before-modify step on
  `lib.rs` first, and is tracked separately.
- **AST-assisted re-anchoring** (resolve a fragment whose surrounding text
  moved) is not implemented; a moved fragment is refused with a diagnostic
  like any other no-match.

No live-model measurement exists, so this is not evidence of an end-to-end
task-success improvement — see
`plans/evidence/production/BENCH/baseline-raw-v1.md`.
