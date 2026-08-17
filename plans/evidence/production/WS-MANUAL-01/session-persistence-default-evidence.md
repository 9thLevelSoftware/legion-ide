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

## Nothing AI-relevant is persisted

Unchanged by this work and still enforced: `WorkspaceSessionRecord` is a
metadata-only DTO, `DesktopSessionStore::save` runs `validate_record` and
`reject_raw_source_markers` before writing, and the existing
`session_restore_store_rejects_raw_source_markers_in_payload_field` and
`session_restore_store_allows_marker_like_benign_metadata` tests cover both
directions. `.legion/session.json` is gitignored alongside the other
per-workspace runtime artifacts.

## Scope note

The task's `files` field names `crates/legion-storage/src/layout.rs`. The
implementation went through the desktop session record instead, which is where
the working restore path already lives. `legion-storage` does contain a complete
`DockLayoutRepository` — `save_dock_side_layout` and friends, including a
splitter fraction — with **zero callers anywhere in the workspace**. It was left
alone rather than wired up as a second, parallel store for the same data; that
duplication is worth resolving on its own terms, not as a side effect of this
task.
