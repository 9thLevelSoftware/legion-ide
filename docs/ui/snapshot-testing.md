# Snapshot testing the rendered shell

Date: 2026-08-18
Suite: `crates/legion-desktop/tests/shell_snapshots.rs`
Baselines: `crates/legion-desktop/tests/snapshots/`

## What this catches, and what it does not

The interaction suites — `shell_affordances.rs`, `explorer_activation.rs`,
`session_restore.rs` — drive the real app through synthetic input and assert
that a control exists, is hit-testable, and dispatches. They cannot see what the
shell *looks like*.

Several defects found on 2026-08-17 were exactly that gap: five activity-rail
icons rendering as `□`, a tab's close button floating outside the tab it
belonged to, a modal laid out below the window's bottom edge, and the Windows
extended-length path prefix leaking into the breadcrumb. Every one was obvious
in a screenshot and invisible to an assertion about state.

It does **not** catch the other defect classes from that day, and it is worth
being explicit so nobody assumes broader cover than exists:

| Defect class | Caught by |
| --- | --- |
| Something renders wrongly | These snapshots |
| A control exists but does nothing | `shell_affordances.rs` and friends |
| A capability has no route from any gesture | `cargo run -p xtask -- intent-reachability` |
| A fixture teaches the code the wrong contract | Nothing automatic. See the DAP handshake in `plans/evidence/production/WS-A-D/phase-2-dap/B3-resolution-trust-dual-mode.md` |

That last row is the uncomfortable one. A snapshot records what the code renders
today; if the expectation itself is wrong, the snapshot preserves the mistake.
Snapshots are a regression net, not a correctness oracle — approving a diff is
the moment the judgement happens, and it cannot be delegated to the tool.

## Proof that the net actually catches something

A snapshot suite that has never been shown to fail is an assertion about
itself. This one was verified by regression: the close glyph painted in a tab
was shrunk from 12pt to 9pt, and

- `open-file` and `unsaved-changes-prompt` failed — the two states that render a
  tab strip;
- `empty-shell` and `explorer` passed — the two that do not.

The control mattered as much as the failure: had all four moved, the suite would
be reacting to something other than the change.

The first attempt at this proof passed on all four and looked like a hole in the
comparison. It was not. It perturbed `TAB_CLOSE_GLYPH_SIZE`, whose name promised
it sized the glyph while it actually sized the *hover* chip — and nothing is
hovered in a snapshot. The constant is now `TAB_CLOSE_HOVER_SIZE`, with
`TAB_CLOSE_GLYPH_FONT_SIZE` beside it.

Sensitivity is high: the failing diffs were 3 and 1 pixels. That is the intended
setting, and it is why baselines are per platform rather than compared loosely.

## Determinism

Anything machine-specific in the frame makes a baseline unreproducible, and the
suite found one in itself immediately: the breadcrumb and status bar render the
active buffer's canonical path, and the test workspace lives under a temp
directory carrying a pid, a timestamp, and — on Windows — the account name. The
first baselines could not match their own next run.

`stabilize_paths` in the suite rewrites that one field to `/workspace/{name}`
before rendering. One field is enough because everything else path-shaped is
derived from it, and explorer rows and tab titles render names rather than
paths. Two consecutive fresh runs must pass; that is the check to repeat if a
snapshot ever fails for reasons nobody changed.

## What it found on its first three-platform run

The bootstrap run paid for the suite before the suite was even committed.

The Legion mark in the top bar was `◆` (U+25C6 BLACK DIAMOND) in a label. The
three renders showed a small filled dot on Windows, an amber diamond on macOS,
and `□` — the missing-glyph box — on Linux. The product's own brand mark was a
broken character on one of its three targets.

This is the same defect as the five activity-rail icons, found the same way and
fixed the same way: `view/brand_mark.rs` draws it. A line is a line on every
platform; a codepoint is a negotiation with whatever fonts the host happens to
have.

Note what this implies for the bootstrap procedure below. A `.new.png` from a
platform you do not have is **a render, not a baseline**. Committing the Linux
one unexamined would have made the missing-glyph box the expected appearance of
the product on Linux, and the suite would then have defended it. "Look at the
diff" is not advice in step 4; it is the step.

## Why baselines are per platform

Font rasterisation, hinting and subpixel placement differ enough between
Windows, macOS and Linux that one baseline cannot serve all three without a
comparison threshold so loose it stops catching the defects above.

Baselines are therefore named `{state}-{os}.png`, from `std::env::consts::OS`:
a baseline has to match the machine that rasterised the glyphs, which is a
runtime fact rather than a build-time one.

The cost is real — every new snapshot is three files to regenerate — so the
suite deliberately stays small, and each state is one where a defect actually
lived rather than one added for coverage.

## Regenerating on the platform you have

```text
UPDATE_SNAPSHOTS=1 cargo test -p legion-desktop --test shell_snapshots
```

This writes the baseline for your platform only. Review the resulting PNG before
committing it: `UPDATE_SNAPSHOTS=1` accepts whatever the code currently draws,
including a regression.

## Regenerating for a platform you do not have

You do not need three machines.

1. Push the change. The `Standing gates` job runs on all three.
2. Any platform whose baseline is missing or stale fails `cargo test`.
3. That job uploads a `shell-snapshots-{os}` artifact containing every
   `.new.png` (what the run rendered) and `.diff.png` (what moved).
4. Download the artifact, **look at the diff**, and if the change is intended,
   commit the `.new.png` as `{state}-{os}.png`.

Step 3 is why the artifact upload runs `if: failure()` — a failure is the only
way to obtain a baseline for a platform nobody has locally, so it is a normal
part of the workflow rather than only an error path.

`.new.png` and `.diff.png` are gitignored. Only baselines are tracked.

## Bootstrapping a new snapshot

A new snapshot is red on the two platforms you are not on until their baselines
land, and that is expected. Add the test, generate your own platform's baseline,
push, and take the other two from the artifact.

## Requirements

`egui_kittest` renders through `wgpu` and prefers a CPU adapter, so no GPU is
needed. Linux additionally needs a software Vulkan implementation —
`mesa-vulkan-drivers` (lavapipe), installed by the Linux step in
`legion-gates.yml`. Windows uses WARP and macOS uses Metal, both present by
default on the hosted runners.
