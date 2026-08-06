# Task 2 report: Prototype design tokens and responsive shell geometry

## Status

Completed on top of `df5f089`.

## Implementation

- Retokenized the dark shell to the accepted blue-slate/warm-amber palette:
  shell `#16202b`, editor `#121a23`, panel `#1d2a38`, border `#2c3a4a`,
  primary `#f4f1eb`, muted `#7e8a9b`, amber `#cf8136`, Assist blue
  `#2e7fb8`, success `#4fae6d`, and danger `#d23b2e`.
- Kept light/system themes and strengthened their focus/accent roles; raised
  the standard eyebrow token to 11px and never derive muted editor text below
  11px.
- Added pure `ShellGeometry::for_available_size(width, height)`. It is the
  only responsive policy seam used by the renderer: desktop uses
  42px top / 46px rail + 248px explorer / 325px right / 192px bottom / 24px
  status; compact uses a deterministic 250px left region and 325px right
  region, leaving 385px for a 960px-wide editor canvas.
- Replaced mode-dependent outer panel sizes with this geometry. Panels remain
  user-resizable at desktop widths and use deterministic exact compact sizes.
- Added an activity rail inside the left region. Its Files, Search, and
  Symbols controls only dispatch existing command-palette actions; it owns no
  navigation or product state.

## TDD evidence

### RED

After adding the accepted token assertions and geometry contract test, before
the implementation:

```powershell
$env:CARGO_BUILD_JOBS='1'; cargo test -p legion-desktop theme::tests -- --nocapture; cargo test -p legion-desktop --test projection_rendering projection_rendering_uses_stable_responsive_shell_geometry -- --nocapture
```

Result: failed as expected with `E0432`, `no ShellGeometry in view`, from the
new geometry test. Cargo compiles integration targets before running the theme
filter, so this expected missing-feature failure prevented the token assertion
from executing in that invocation.

### GREEN

```powershell
$env:CARGO_BUILD_JOBS='1'; cargo test -p legion-desktop --lib theme::tests -- --nocapture
$env:CARGO_BUILD_JOBS='1'; cargo test -p legion-desktop --lib view::tests -- --nocapture
$env:CARGO_BUILD_JOBS='1'; cargo test -p legion-desktop --test projection_rendering -- --nocapture
```

All passed: 2 theme tests, 12 view tests, and all 21 projection-rendering
tests (including desktop and 960x720 geometry assertions).

## Verification

```powershell
cargo fmt --all --check
git diff --check
```

Both passed. An initial broad `cargo test -p legion-desktop theme::tests`
GREEN attempt exceeded 300 seconds with no compiler/linker/test child on this
Windows host; it was terminated and replaced with the narrow lib-only command
above, as required. All desktop test commands used `CARGO_BUILD_JOBS=1`.

## Files changed

- `crates/legion-desktop/src/theme.rs`
- `crates/legion-desktop/src/view.rs`
- `crates/legion-desktop/tests/projection_rendering.rs`

## Self-review

- The geometry function takes only viewport dimensions, not product mode, so a
  mode transition cannot change the outer shell allocation.
- At 960x720 the asserted center width is 385px, above the 360px requirement.
- Compact sizes are fixed only below the single 1100px threshold; desktop
  panels preserve the existing resizable behavior.
- The rail uses only `OpenPalette` actions and does not add renderer-owned
  product/navigation state.
- `git diff --check` found no whitespace errors.

## Concerns

- No live native screenshot was captured in this task; visual chrome and
  mode-specific rail recomposition are intentionally owned by later tasks.
- Two unrelated `xtask` files were concurrently modified in the shared
  worktree and are deliberately excluded from this task's commit.
