# P1.F2.T4 — layout persistence actually runs in the product

Date: 2026-08-17
Readiness row: PR-UI-001
Task: P1.F2.T4 — "Persist/restore layout metadata only (no code, no AI context)"
Acceptance: "Restart restores the layout; nothing AI-relevant is persisted."

## What was wrong

The persistence machinery was complete and thoroughly tested, and **it never
ran**. `DesktopLaunchConfig::new` and `from_args` both left `session_state` as
`None`; `--session-state <path>` was the only way to set it and there was no
default. `DesktopRuntime::save_session_state` therefore returned `Ok(())`
immediately, and every restart of a normally-launched `legion-desktop` discarded
the open tabs, the active buffer, the explorer expansion, the panel state and
the dock layout.

Ten tests in `crates/legion-desktop/tests/session_restore.rs` passed throughout,
because every one of them constructs the runtime through a helper that hands it
an explicit session path — a path the product itself never supplied. The gap was
not in the feature; it was between the feature and the only launch path a person
uses.

Found by inspection during the 2026-08-17 interactive dogfood session
(`../../dogfood/2026-08-17-interactive-gui-journal.md`), which is also where the
class of defect — tested, green, and not wired to the running app — was already
established four times over.

## What changed

**A default session path for interactive launches.**
`DesktopLaunchConfig::default_session_state_path` returns
`<workspace_root>/.legion/session.json`, applied in `from_args` when
`--session-state` was not given.

Per workspace rather than per user, and beside the workspace's other
per-workspace state (`palette_usage.json`, `proposal-audit/`) which
`enable_workspace_state_persistence` already creates and workspace scans already
exclude. Opening one repository must not restore another's tabs.

The default is applied *after* the beta config is constructed. That branch reads
`session_state` directly and substitutes its own
`DEFAULT_BETA_SESSION_STATE_PATH`, so defaulting earlier would have silently
redirected beta-smoke evidence into the workspace. `--smoke` and `--manual-perf`
are short-lived measurement harnesses and keep whatever they were given.

**A write guard, which this change made necessary.**
`persist_session_if_configured` is called from the catch-all arm of
`handle_action`, so it runs on *every* dispatched action — including each
inserted character. `DesktopSessionStore::save` is deliberately expensive: temp
file, `sync_all`, read back and re-parse to validate, atomic replace. With
`session_state` at `None` that cost was zero; switching the default on would
have put a durable fsync round-trip inside the ADR-0048 keypress budget
(p50 <16ms).

`save_session_state` now compares a serialized fingerprint of the record — with
`saved_at` zeroed, since it changes on every capture and would defeat the check
— against the last one written, and skips the write when nothing a restart would
restore has changed. Encoding a small metadata record costs microseconds against
milliseconds for the durable write. The guard is what makes saving on every
action, and therefore surviving a crash, affordable.

## Evidence

Six tests added to `crates/legion-desktop/tests/session_restore.rs`. They drive
argument parsing — the path the product actually takes — rather than a
test-constructed config.

| Test | Claim |
| --- | --- |
| `an_interactive_launch_persists_its_layout_by_default` | A bare `legion-desktop <workspace>` resolves a session path |
| `an_explicit_session_path_still_wins` | `--session-state` overrides the default |
| `measurement_harnesses_do_not_inherit_the_workspace_session_path` | `--smoke`, `--beta-smoke`, `--manual-perf` keep `None` |
| `a_default_launch_round_trips_open_tabs_across_a_restart` | The acceptance criterion end to end: open a file, save, reopen from a fresh config, the tab is back |
| `an_unchanged_session_is_not_rewritten` | A second save that changes nothing does not touch the file (asserted on mtime) |
| `a_changed_session_is_written_again` | Opening a tab does write, and the record contains it |

Vacuity checked on the guard test, where a passing mtime comparison could have
meant "filesystem timestamp resolution" rather than "we skipped the write":
disabling the guard fails `an_unchanged_session_is_not_rewritten` with two
distinct mtimes. The two default-path assertions cannot be vacuous — the prior
value was literally `None`.

```
cargo test -p legion-desktop --test session_restore -j 6
test result: ok. 16 passed; 0 failed
```

Full `legion-desktop` and `legion-app` suites pass; clippy and fmt clean.

## Panel sizes: the half the task was reverted for

The earlier revert note read: *"`splitter_fraction` and `collapsed` are persisted,
validated by legion-storage, and reloaded, but no renderer reads either… Restart
restores the record, not the layout the user sees."* That is now closed for
`splitter_fraction`.

`view/dock_geometry.rs` turns a stored fraction into a panel `default_size` on
the way in, and a rendered panel size back into a fraction on the way out. Both
directions divide by the same denominator — the shell's inner rect, captured
once before any dock is placed — because measuring against `ui.available_*`
after a panel is placed yields the *remaining* space, and the panels would creep
on every restart.

Two rules for the write side were implemented and discarded before the third,
and both failures are worth recording because both looked right:

1. **"Report the rendered size."** Broke
   `product_mode_changes_preserve_projected_editor_and_panel_state` immediately:
   on the first frame the rendered size is the geometry default, so every
   restored fraction was overwritten with it. Manual's left dock went 0.32 →
   0.198 the moment the shell rendered.
2. **"Report it when it differs from the size we requested."** Also wrong. egui
   honours `default_size` only until it has a remembered size of its own; after
   that the panel legitimately sits somewhere we did not ask for, and every
   frame reads as a resize.

A drag is a *transition*, so the third rule looks for one: a side is reported
only when its pixel size changed between consecutive frames, and only when the
shell itself did not change size — a window resize moves every panel and changes
every fraction, and treating that as intent is how briefly narrowing the window
would destroy the arrangement.

One further gate was needed. `DockLayout::standard_all_modes` carries splitter
fractions that disagree with `ShellGeometry`'s constants — it was written when
nothing read them — and the constants are what the prototype-fidelity tests hold
the shell to (`325px` right rail, `192px` console). Applying the defaults'
fractions silently resized every panel in the product. `dock_layouts_user_arranged`
now distinguishes a restored or dragged arrangement, which may override the
designed sizes, from the shipped defaults, which may not.

| Test | Claim |
| --- | --- |
| `projection_rendering_applies_a_restored_left_splitter_fraction` | Two renders at 0.18 and 0.42 produce different explorer widths — the assertion whose absence kept this task open |
| `projection_rendering_reports_no_dock_drag_when_nothing_moves` | A settled layout reports nothing to persist |
| `a_resized_panel_is_persisted_and_survives_a_restart` | Drag → stored → written → reopened → still there |
| `an_unmoved_splitter_does_not_write_the_session` | An idle frame does not reach the disk |
| `a_panel_that_was_not_rendered_is_not_recorded_as_collapsed` | `None` means "not drawn", never "dragged to zero" |
| 12 unit tests in `view/dock_geometry.rs` | Clamping, inverted ranges, degenerate bases, window-resize rejection, appear/vanish |

Vacuity checked on the load-bearing one: reverting the left panel to
`default_size(geometry.left_width)` fails
`projection_rendering_applies_a_restored_left_splitter_fraction` with `294` for
both fractions — exactly the old behaviour.

`collapsed` remains unread, and deliberately so: no renderer affordance can set
it, so there is no user state for restore to honour. If a collapse control is
added, restoring it belongs with that work.

## Nothing AI-relevant is persisted

Unchanged by this work and still enforced: `WorkspaceSessionRecord` is a
metadata-only DTO, `DesktopSessionStore::save` runs `validate_record` and
`reject_raw_source_markers` before writing, and the existing
`session_restore_store_rejects_raw_source_markers_in_payload_field` and
`session_restore_store_allows_marker_like_benign_metadata` tests cover both
directions. `.legion/session.json` is gitignored alongside the other
per-workspace runtime artifacts.

## Unrelated flakes observed while running these suites

Two `legion-app` tests fail intermittently under parallel load and pass in
isolation, both in the conflict-detection family:

- `control_trust_surfaces::dirty_text_preserved_on_rejected_stale_and_conflict_outcomes`
  — `conflict disk: Os { code: 2, kind: NotFound }`
- `workspace_vfs_integration::workspace_vfs_integration_conflicted_registered_save_preserves_dirty_buffer_and_disk`
  — expected `Conflict | Stale`, got neither

Not caused by this change — it touches `legion-desktop` only — and not fixed
here. Recorded because a flake nobody wrote down is a flake that gets
re-diagnosed, and Phase 1's exit gate needs green CI on three platforms.

## Scope note

The task's `files` field names `crates/legion-storage/src/layout.rs`. The
implementation went through the desktop session record instead, which is where
the working restore path already lives. `legion-storage` does contain a complete
`DockLayoutRepository` — `save_dock_side_layout` and friends, including a
splitter fraction — with **zero callers anywhere in the workspace**. It was left
alone rather than wired up as a second, parallel store for the same data; that
duplication is worth resolving on its own terms, not as a side effect of this
task.
