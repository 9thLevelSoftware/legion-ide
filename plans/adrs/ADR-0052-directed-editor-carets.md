# ADR-0052: Directed caret state in editor authority

## Status

Accepted — 2026-09-05 by the root coordinator after independent design review. Part of full-product package S1-04. Acceptance of this architecture decision is not evidence that navigation is implemented or product-qualified.

## Context

`EditorBufferState` currently stores cursors and ordered selections independently. `set_selections` does not move the active cursor. A backward selection loses its anchor/head direction, repeated Shift movement cannot extend reliably, and edits/undo restore text without restoring a coherent caret state. Native exploration also found Home unmapped in the desktop; source inspection confirms Home and End are absent. Fixing only their key translation would retain those selection defects.

The approved full-product plan assigns ordinary selection, Unicode, undo/redo and multi-cursor behavior to S1-04. The editor remains the authority and the desktop remains a projection/input adapter. This decision changes the existing editor state contract; it introduces no new runtime surface or crate dependency.

## Decision

### One state, derived views

Each editor buffer owns one ordered vector of directed carets: a valid UTF-8 `TextPosition` head and optional anchor. A collapsed caret has no anchor. An anchor equal to the head is permitted while extending through the anchor; ordered ranges are derived for painting and clipboard operations. Neither the UI nor the app stores a second persistent selection model.

Existing public `Cursor` and `Selection` structs remain compatibility values. Cursor views are derived from heads and selection views from normalized anchor/head endpoints. An owned cursor vector replaces the current borrowed slice API. Viewport generation derives both projections directly from the authoritative vector, preserving primary-caret order.

Every open buffer has at least one caret, including an empty text buffer. All setters validate every endpoint before changing any state. Empty cursor or directed-caret replacement is rejected atomically. `set_cursors` replaces the caret set and clears anchors. Empty `set_selections` clears anchors while preserving heads; nonempty `set_selections` replaces the set with forward carets (`anchor=start`, `head=end`). Its count need not match the previous cursor count. A directed setter accepts explicit heads/anchors without sorting away direction. Invalid endpoints leave the whole prior state intact.

Selections remain attached to their buffer across tab switches and temporary focus loss. A renderer may retain a pointer gesture's initial coordinate until drag completion; it sends both gesture endpoints, and keyboard extension uses only editor-owned direction after that gesture. This is transient input capture, not editor state ownership.

### Edits and history

Before committing an edit batch, resolve every head/anchor to an original absolute byte offset. Apply the existing validated nonoverlapping edits to staged text, and map every offset through those same edits in descending application order. For each replacement `[start,end)`:

- Points before `start` stay in place.
- Points at or beyond a nonempty replacement's `end` shift by inserted bytes minus removed bytes.
- Points within the replacement map to its new end for a head and new start for an anchor.
- At a zero-width insertion, a head at the insertion moves after inserted text and an anchor at the insertion stays before it.

Descending application is the boundary convention: at a shared boundary, process the higher-offset replacement first, then map that result through the lower-offset replacement. For `abcd` with `[1,2) -> XX` and `[2,3) -> Y`, a head originally at byte 2 ends at byte 4 in `aXXYd`, and an anchor there ends at byte 3. Multiple insertions at one offset use the exact stable order used by the text transaction; right-affinity heads end after all inserted text and left-affinity anchors stay before it. Convert mapped offsets through staged text before committing. The mapped caret state, text, version and history must commit together; a failed batch changes none of them. Use rope/index operations, never whole-text materialization for this transformation.

The app's existing explicit cursor collapse after ordinary input remains the command's final caret action. Multi-cursor input must install its complete final caret set, not overwrite only the primary head. Generic programmatic edits preserve mapped selections; an explicit user selection replacement clears the replaced selection through the existing app edit path. Do not infer user intent merely from `TransactionSource::User`, which also labels programmatic operations.

Undo entries store the corresponding caret vector with each text snapshot. Undo and redo restore both, and move the current text/carets into the opposite history stack. A maximal uninterrupted sequence of successful edits with the same nonempty group ID is one history unit, retaining the earliest pre-group text/carets; redo captures the final post-group text/carets. `None` edits are separate units, another group ID breaks a group, and successful undo/redo breaks the active edit group. Noncontiguous reuse of an ID never merges units. Coalesce the active group before enforcing retention. Retention may evict a whole group under the existing budget, but must never expose a partial tail of that group as undoable. Remember an active group whose anchor was evicted: its continued edits still apply successfully, but the entire group remains unavailable for undo; they do not start a fresh history unit or reject the edit. New groups may be recorded normally. Eviction truncates the affected buffer history stack through that unit (the unit and all entries below it), preserving a contiguous reachable suffix and preventing undo/redo from jumping across a missing unit. Leased snapshots remain valid through their independent leases even when removed from history. Evicting the active undo unit therefore clears older undo history for that buffer; the next new group can be undone only as far as its own starting state. A failed edit/history operation changes neither grouping metadata nor text/carets. Test eviction during an active group, continued edits after eviction, and a subsequent independent group. Proposal-mediated file saving remains in force.

### Navigation through existing authority

Home, End and document-boundary movement are semantic requests through the existing desktop action, UI intent and app/editor route. The editor resolves all active heads atomically using line-index byte lengths. Plain movement clears anchors; extending movement initializes each absent anchor from that caret's prior head, then preserves it across repeats and reversal. Empty buffers, trailing newlines, CRLF and lines beyond the viewport follow actual text metrics.

Protocol coordinates retain distinct UTF-8 byte columns, absolute byte offsets and UTF-16 offsets. The renderer must not compute byte destinations by incrementing a scalar count or copying that count into UTF-16 fields. Mouse direction must reach the app without ordered-range normalization.

This migration supports the full navigation work; S1-04 still requires grapheme movement, vertical preferred columns, clipboard/IME/Vim behavior, settings and native qualification. A state migration or Home/End test alone cannot close that package.

## Implementation sequence and verification

1. Migrate editor caret authority, derived projections and compatibility setters; update actual callers, not unrelated types named Selection. Add directed-setter and invalid-input atomicity tests.
2. Map carets through edit transactions and snapshot them in history. Test points before/inside/at boundaries/after edits, adjacent and zero-width edits, multiple carets, failed batches, and grouped undo/redo in both directions.
3. Route directed pointer endpoints and semantic boundary navigation through the existing app authority. Test backward drag followed by repeated Shift+Home/End reversal, plain collapse, per-buffer tab continuity, all carets on unequal lines, and editor-input focus guards.
4. Verify Unicode/CRLF byte and UTF-16 projections and a streamed buffer over the full-cache limit. Run affected editor/app/desktop tests and dependency ownership gates. Then obtain ordinary native input evidence on each required platform through S1-02; headless tests remain lower-layer evidence.

The implementation must add the corresponding dependency-policy contract note and ownership/contract tests before activating new command variants. For the grapheme-boundary primitive, `legion-text` is authorized to depend directly on the pinned workspace `unicode-segmentation = 1.13.2` crate. Text owns rope-backed Unicode segmentation and byte-boundary validation; editor authority owns directional deletion and all editing decisions. No other new runtime dependency is authorized. The root coordinator owns architecture and final acceptance; Luna workers perform bounded implementation/review under the user's execution policy.

## S1-04f horizontal movement addendum

Horizontal movement is an editor-authority operation over the existing ordered
`DirectedCaret` vector. `HorizontalDirection::Left` and `Right` resolve each
head against `TextBuffer`'s strict extended-grapheme boundary APIs, including
CRLF, streamed buffers, and valid scalar offsets inside a grapheme cluster.
Plain movement collapses a nonempty selection to its lower or upper byte
endpoint respectively, without an additional step, and clears anchors.
Extended movement initializes an absent anchor from the old head and retains
anchors through reversal and crossing. Every caret is resolved against the
original text before any state is committed; a failed endpoint conversion
leaves all carets unchanged. Movement changes neither text, buffer version,
transaction history, nor change events.

## S1-04g viewport line-origin addendum

Viewport line metrics may carry the absolute byte and UTF-16 origins of each
logical line in the current snapshot. The editor derives both values from its
snapshot line index without materializing full text, including for streamed
buffers. A missing origin is legacy or unavailable metadata and must remain
`None`; consumers must never infer zero. Byte and UTF-16 origins are separate
coordinates because line-local character metrics cannot recover either absolute
snapshot origin.

## S1-04g visual caret affinity addendum

`CaretAffinity` is protocol-owned and defaults to `Upstream`, preserving the
legacy side of a shared soft-wrap boundary. `DirectedCaret` stores that value
alongside its head and optional anchor; `with_affinity` provides explicit
visual placement while the legacy constructor remains upstream-compatible.
`ViewportProjection.cursor_affinities` is an optional, ordered vector aligned
with `cursors`; omitted legacy metadata means upstream affinity for every
cursor. The editor emits the complete vector from its authoritative caret
state.

`EditorEngine::set_visual_directed_carets` requires the expected snapshot ID
and buffer version, validates all endpoints before mutation, and updates only
the caret vector. Stale identity or invalid endpoint failures preserve text,
version, history, and the prior ordered caret vector. Text edits and semantic
coordinate movement construct fresh upstream carets, while undo and redo
restore the complete captured caret vector including affinity. Visual
placement itself never creates a text transaction or undo entry.

## S1-04h vertical authority addendum

The vertical contract is editor-owned. Each directed caret may carry a typed,
finite, non-negative row-local `PreferredX` in rendering-point space plus its
existing grapheme-boundary `CaretAffinity`; the editor owns the preferred value
and its validity. It is not a UI state, absolute screen coordinate, scalar
column, or UTF-16 offset. The renderer supplies the shaped geometry through the
forthcoming protocol/app route; it does not become an authority or a new
desktop-to-editor dependency.

The transient layout identity is separate from `snapshot_id` and
`buffer_version`. It identifies the shaping configuration used for the supplied
facts: font/settings revision, wrapping policy and width, and tab policy. It
does not authenticate the supplied row geometry and does not include text or
per-row facts. A nonzero layout identity is required, and a preferred X is
reusable only when that configuration identity is unchanged. Snapshot/version,
buffer identity, and the exact ordered source-caret vector are the content
guards; the caller and renderer/app route must supply a current coherent
configuration and matching bounded facts. The editor core cannot independently
distinguish a stale opaque layout token from another valid nonzero configuration
identity.

`move_vertically` validates the complete ordered source-caret vector before
processing targets: buffer identity, snapshot ID, buffer version, caret count,
head/optional-anchor UTF-8 validity, source line/span containment, grapheme
boundary validity, affinity validity, and finite preferred X. For every caret it
then validates the target row's bounded shaped stops, exact span adjacency,
source-to-target logical-line adjacency, target-stop bounds, and affinity at a
shared wrap boundary. Optional row ordinals are descriptive only; they are
never used to infer adjacency when absent. The renderer may provide only the
bounded source/target stops needed for the request, including for streamed or
huge lines; full-file materialization and unbounded row enumeration are
forbidden. The entire ordered target vector commits atomically only after all
guards pass.

Vertical movement retains each caret's preferred X for the same layout and
re-seeds it from renderer-shaped source geometry when a valid layout
configuration identity changes. Any nonvertical placement or movement, edit, reset, or explicit caret
replacement clears preferred X while preserving the established directional
anchor/affinity rules. Undo and redo restore the complete caret vector,
preferred X, affinity, and layout identity with the text snapshot. Rejected
requests change none of these values, text, version, history, or events.

The bounded editor core and its contract tests implement and test these guards,
resets, and undo/redo semantics. The live desktop key route, protocol/app
transport of the shaped facts, and native GUI qualification remain pending;
the current core evidence does not claim completion of the full vertical
workflow.
