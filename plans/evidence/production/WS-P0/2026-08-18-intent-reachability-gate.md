# A standing gate for capabilities nobody can reach

Date: 2026-08-18
Gate: `cargo run -p xtask -- intent-reachability`
Config: `xtask/intent-reachability.toml`

## Why

On 2026-08-17 four separate capabilities were found complete, tested, and with
no way for a person to reach them. Each was found by the owner running the app,
one at a time, and every test suite stayed green throughout — because in every
case the app layer was correct and the route to it did not exist:

| Capability | What was missing |
| --- | --- |
| Open a file from the explorer | The row dispatched select-and-reveal; no buffer ever opened |
| Persist the session | `session_state` had no default path, so `save_session_state` returned immediately |
| Restore panel sizes | Splitter fractions were persisted and reloaded, and read by no renderer |
| Multi-cursor | Intents, app handling and eight passing tests; no `DesktopAction`, no bridge translation, no keybinding |

Four instances of one defect class in a day is a pattern, and finding the fifth
by chance is not a plan.

## What the gate does

`CommandDispatchIntent` is the whole vocabulary of things the product can be
asked to do. The gate enumerates its variants and requires each to be named by
some file that can turn a gesture into it — the bridge, the keyboard handler,
the `:` command line, Vim mappings, and the palette's string-keyed table in
`legion-app`.

An intent with no route fails the build unless `xtask/intent-reachability.toml`
allowlists it **with a written reason**. Three further rules keep the allowlist
from becoming the problem:

- a reason is required and may not be blank;
- an entry that names no variant is an error;
- an entry whose intent *became* reachable is an error, so an exemption cannot
  quietly outlive its justification.

The check is textual on purpose. Tracing real reachability would mean following
control flow through the renderer, a string-keyed palette table and the Vim
parser, and a gate whose verdict nobody can predict is a gate people route
around. "Some route-carrying file names this variant" is coarse, but it is
exactly the property that was absent in all four cases, and it cannot be
satisfied by accident.

## What it found immediately

Six of 178 intents had no route. They divided cleanly, and the division is the
useful part:

**Two were implemented and unreachable — now wired.**

- `SetLineWrappingPolicy`. The setting is projected, the app handles it, and
  `code_line_wrap_width` genuinely reads it — so Off / Viewport / FixedColumn
  all work, and the only value reachable in the product was the default. Now a
  three-pill row in Settings → Editor.
- `ActivateLanguageCodeLens`. The app resolves the lens, checks it is runnable,
  and launches its command in the terminal. rust-analyzer reports these for
  every `#[test]` and binary target. They rendered only inside a diagnostic
  string, so the "run this test" affordance existed everywhere except on
  screen. Now a `Runnables` button row above the test controls.

**One is inert — allowlisted, not wired.**

- `SetEditorFontFamily` is persisted, projected and displayed in Settings, but
  every code-canvas call site hard-codes `egui::FontId::monospace(...)`, so the
  family never reaches text layout. Wiring a picker would have offered a control
  that appears to work and does not — the exact defect this gate exists to
  prevent. The allowlist entry says so and says to give it an effect first.

**Three are stubs — allowlisted with the roadmap item that owns them.**

- `PrepareCallHierarchy`, `ShowIncomingCalls`, `ShowOutgoingCalls` all route to
  `AppCommandRequest::Noop`. These are not unreachable features; they are
  unbuilt ones. The `legion-lsp` protocol layer is complete and contract-tested
  with no app plumbing behind it. Tracked by backlog task **P2.F1.T6**.

  That task was created on 2026-08-18, when the citation was checked and found
  to point at nothing: the reasons named a "roadmap 1.6" that exists in no
  document, the roadmap itself deferring work items to the kanban backlog. An
  allowlist entry whose reason cites a phantom tracker is the rot the reason
  requirement exists to prevent, and it survived a day because the gate checks
  that a reason is *present*, not that it is *true*.

A seventh candidate, `Noop`, was allowlisted on the first draft and the
staleness rule immediately rejected it: it *is* produced from the desktop. The
rule earned its keep before the gate was committed.

## Verification

```
cargo run -p xtask -- intent-reachability
intent-reachability: 178 intent(s) reachable or allowlisted
```

Three unit tests cover the enum reader, which is the part that could silently
weaken the gate: a variant carrying a struct body must not truncate the list, a
lowercase field must not be mistaken for a variant, and a missing enum must be
reported rather than read as an empty set — the last because returning empty
would make the gate pass loudly while checking nothing.

Added to `legion-gates.yml` beside `extract-before-modify`. All standing gates
green; `legion-desktop`, `legion-ui`, `legion-app` and `xtask` suites pass;
clippy clean.

## Scope note

The gate checks `CommandDispatchIntent` only. `DesktopAction` has the same
failure mode in the other direction — an action no control produces — and is
not covered here. Worth adding once this one has proven itself in CI rather
than widening an unproven gate on its first day.

## The gate caught its author twice

Worth recording, because both were the very defect the gate exists to retire.

The runnables row shipped first with `.take(6)`, silently dropping every
runnable past the sixth. Told about it, the second attempt added a `+N more`
label — which counts the hidden ones without giving anyone a way to run them.
As review put it: the label can count the hostages but cannot release them. The
list is now unbounded inside a bounded `ScrollArea`, since the activity sidebar
has no scroll of its own and that is what made a cap look necessary.

The renderer was also listed in `route_sources` and contributes nothing:
`view.rs` and its submodules name `DesktopAction`, never
`CommandDispatchIntent`, because the bridge is where a gesture becomes an
intent. Removed, with the reason recorded in the config.
