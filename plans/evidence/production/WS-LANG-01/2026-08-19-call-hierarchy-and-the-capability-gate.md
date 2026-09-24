# Call hierarchy, and the capability gate nobody could open

**Date:** 2026-08-19
**Task:** P2.F1.T6 — wire the call-hierarchy app stub so incoming/outgoing calls reach a panel
**Related:** P2.F1.T4 (read-side navigation, marked done), `xtask/intent-reachability.toml`

## What was asked for

Three `CommandDispatchIntent` variants — `PrepareCallHierarchy`,
`ShowIncomingCalls`, `ShowOutgoingCalls` — routed to `AppCommandRequest::Noop`.
The `legion-lsp` protocol layer behind them was complete and contract-tested.
The `intent-reachability` gate is what made that visible rather than invisible,
and P2.F1.T6 is the task it forced into existence.

## What was built

**Two round trips, one question.** LSP does not answer "who calls the thing
under my caret" in one request: `prepareCallHierarchy` resolves a position to a
symbol, and only then can `incomingCalls` or `outgoingCalls` be asked. The
direction is chosen at step one and needed at step two, so it waits in
`pending_call_hierarchy` between them. Making the caller issue both would have
put an LSP sequencing detail into the intent vocabulary, where a user gesture
has no business knowing about it.

**Rows are locations.** A call is a place in a file exactly as a reference is,
so the rows are `LanguageLocationProjection` and land in the panel that already
lists locations. A dedicated call-hierarchy panel would have meant building a
new surface for a row type the product already renders — and an unbuilt surface
is how a feature ends up complete and unreachable, which is the defect this task
exists to retire.

**Gesture.** Ctrl/Cmd+Alt+H for callers, Ctrl/Cmd+Alt+Shift+H for callees,
behind the same editor-focus guard the editor text paths use. A test presses
plain Ctrl+H and asserts it still means find-and-replace, which is the collision
proof the binding choice needed.

## The larger defect this uncovered

Wiring the feature meant gating it on `callHierarchyProvider`. Checking whether
that gate could ever open produced this:

```
never recorded: referencesProvider, documentSymbolProvider,
                inlayHintProvider, codeLensProvider, callHierarchyProvider
recorded:       hoverProvider, definitionProvider,
                completionProvider, diagnosticProvider
```

`issue_lsp_read` refuses to send a request unless the server advertised the
matching capability, and the health record it consults was built by a parser
naming four keys while the read side gated on nine. **References, document
symbols, inlay hints and code lenses have never reached rust-analyzer.**
P2.F1.T4, which claims to have wired them, is marked done.

Two things hid it, and both are worth more than the bug.

The tests for those requests inject a health record directly —
`set_lsp_health_for_test(health_with_caps(&[("referencesProvider", true)]))` —
so they assert that a request fires when the capability is present, which is
true, and never that it was present, which was false. That is the
fixture-teaches-the-wrong-contract row that `docs/ui/snapshot-testing.md` lists
as caught by nothing automatic. It still is not caught automatically; it was
caught by someone asking whether a gate could open.

And it was invisible in use. A refused gate falls back to the lexical index, so
the panel fills with rows. They were the index's answers rather than the
server's, which looks like a working feature until someone asks why references
in another crate never appear.

The fix is one shared list, `GATED_READ_CAPABILITIES`. Capability values are
also read as `boolean | XOptions` per LSP rather than `as_bool()`, since
rust-analyzer sends objects for several of these and `as_bool()` on an object is
`None` — which read as unsupported and closed the gate on a server that had just
said yes.

`crates/legion-app/tests/lsp_capability_gating.rs` drives the real handshake
against the mock and fails when the two lists drift. It failed before the fix
with exactly the five names above. The mock now advertises every gated
capability **except** `codeLensProvider`, which is the deliberate negative
control: a mock that advertises everything cannot tell "records the key" from
"records the right answer", and an implementation marking everything supported
would pass.

## Two bugs in the new code, found by tests rather than by reading

**The pending slot was taken before the buffer was checked.** Ask for callers in
file A, switch to B, ask for callees: A's late answer consumed B's slot, failed
the buffer check and returned, after which B's own answer found nothing pending
and was discarded. The user's most recent question produced nothing, forever.
Matched before it is taken now.

**Asking the opposite direction left the previous answer on screen under the
previous heading.** Callers still listed while "outgoing" was requested, and
permanently so if the server never replied — while the operations list recorded
an `OutgoingCalls` operation with status `Ready`.

Fixing the second needed a third state rather than a cleared one. Empty rows
with a direction means "the server answered: nobody calls this", which is a
result and must be shown as one. Empty rows while waiting is a different thing,
and rendering it as the first states a conclusion the product does not have —
permanently, when the answer never arrives because the server lacks the
capability or the caret was on whitespace. `call_hierarchy_awaiting` separates
them; the panel says "asking…" until an answer lands.

## Verification

- 25 new tests across the module, the app seam, the gesture and the renderer
- The renderer tests read text out of the **paint shapes**, not the projection,
  so a row that is projected but clipped or dropped by the row cap does not
  count as rendered
- Claims were mutation-checked rather than asserted: repointing the incoming
  bridge arm at `FindReferences` fails 3 tests, suppressing the direction
  heading fails 3, reverting the intent arm to `Noop` fails exactly 1
- `cargo test -p legion-app -p legion-desktop -p legion-protocol -p legion-lsp`
  — 156 suites, 0 failures
- `intent-reachability`, `extract-before-modify`, `docs-hygiene`, `claim-audit`,
  `check-deps` all pass
- `view.rs` grew 8 lines against its 120-line budget

## Allowlist

`ShowIncomingCalls` and `ShowOutgoingCalls` are retired from
`xtask/intent-reachability.toml` — the gate's staleness rule demanded it the
moment a gesture reached them, which is the rule working as designed.

`PrepareCallHierarchy` stays exempt with an accurate reason: it resolves a
symbol and stops, so a key for it would be exactly the do-nothing control this
gate exists to prevent. Its exit condition is written down — give it a visible
result of its own first.

## Still open

`P2.F1.T4` is marked done and four of its features never reached the server. The
capability fix means they now will, but nothing in this work verified that
references, document symbols, inlay hints or code lenses actually behave
correctly against a real rust-analyzer now that their requests are being sent
for the first time. That deserves its own pass.
