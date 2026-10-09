# Ticket04 wave2: Bounded renderer/native-focus diagnosis

Date: 2026-10-09. Component diagnosis only; native result remains blocked.

Worktree: `D:/legion-ide-2026-workers/wave2-native-focus-diagnosis`.
Branch: `codex/ide-2026-wave2-native-focus-diagnosis`.
Base: `2b2b98933f52daa270cd2ab8b6886884e3239521`.
Ticket05's review candidate remains frozen. No production, driver, provider,
MCP, save, dependency, status, or shared aggregation file changed. No commit,
native application, OS input, UIA probe, or live provider call was performed.

## Native evidence and the unresolved distinction

The existing read-only logs
`D:/legion-ide-2026-notes/ticket004-native-r4-wave1/editor-focus.log` and
`editor-coordinate-focus.log` report the same exact foreground target HWND/PID,
complete baseline document, selected Clean tab, and contained exact hit. They
also report the window root as keyboard focus and the editor as not focused or
keyboard-focusable. Neither log independently establishes a caret change from
the editor click. Pointer delivery into an editor command is therefore still
unproven in that native attempt.

Root's already-passed compact renderer regression at `3471753` was not rerun.
This work tests exactly two source-backed hypotheses and preserves the current
driver guard and the known native AT-focus discrepancy.

## Hypothesis 1: Mouse-event batching

Pinned `egui-winit 0.34.2`, `src/lib.rs`:

- `State::on_mouse_button_input` (line 791) queues individual PointerButton
  events, and `on_cursor_moved` (line 836) queues PointerMoved events.
- `take_egui_input` (line 245) takes accumulated input at paint; the API does
  not force a paint between press and release.

The existing editor-focus test helper renders press/release in separate frames.
The new test instead puts move/press/release into one RawInput for each compact
Explorer-toggle, file-open, drawer-close, blank-editor, and text-line click.
It observes the buffer opening, document focus, and a caret change from `(0,0)`
to `(1,0)` on the second line, followed by three idle frames retaining document
focus. Complete exact text and unchanged disk bytes are asserted.

Result: **passed; no focus defect reproduced** under that renderer model.

Two preliminary attempts stopped at the test's line selector, not a product
focus assertion. The global selector found two `second` nodes; limiting it to
the editor rectangle still found two. Pinned egui source explains this: a
selectable Label publishes its own same-text TextRun child (`response.rs`,
`fill_accesskit_node_common`; `text_selection/accesskit_text.rs`, lines 95–110).
The final selector requires the actual clickable widget within the editor
rectangle, excluding text-range metadata. It remains unique and then proves
caret movement. These were selector-assumption failures, not behavioral reds
justifying a production repair.

## Hypothesis 2: Late AccessKit activation on the eframe context

The product's `run_headless_full_frame` enables AccessKit on its own context
before every frame. Native eframe uses an externally supplied UI context;
`DesktopEframeApp`'s public `eframe::App::ui` calls the same `render_app_frame`.
Pinned `eframe 0.34.2`, `native/winit_integration.rs`, lines 161–184, enables
AccessKit only on `InitialTreeRequested` and requests immediate repaint. This
makes late activation a source-backed difference worth testing.

The new test directly invokes the public `eframe::App::ui` entry point with an
external egui context and `eframe::Frame::_new_kittest()` (the product ignores
the frame argument). Three frames render before AccessKit is enabled. The first
published tree after activation has the document as focus. A split-frame click
then moves the caret from `(0,0)` to `(1,0)`; three idle frames retain focus.
Text input produces exact `seed\n!second` with Dirty state, stable document focus,
and original disk bytes unchanged. The app's separate headless context remains
unfocused, confirming the external context was exercised.

Result: **passed on its first run; no focus defect reproduced**.

Neither test advertises a Focus action. Both check the real TreeUpdate focus,
consumer focused/focusable state, and exact full document. The pinned consumer
0.35 (Windows) and 0.36 (tests) both allow `is_focused_in_tree` in `is_focusable`
(`src/node.rs`, lines 92–98). Adding a Focus action is not justified by these
results. No actual Windows adapter or event loop was instantiated; synthetic
RawInput and a mock Frame do not establish native delivery or UIA propagation.

## Checks and cache handoff

Commands ran from this worktree with `-j 2`, using root's exclusively granted
warm desktop cache. D: had 1,279,518,826,496 bytes free before building.

```powershell
cargo test -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target -p legion-desktop --test native_focus_diagnosis coalesced_pointer_clicks_open_file_close_drawer_and_move_editor_caret_with_focus -- --exact
cargo test -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target -p legion-desktop --test native_focus_diagnosis late_accesskit_activation_on_external_eframe_context_keeps_document_focus_and_caret_input -- --exact
rustfmt --edition 2024 crates/legion-desktop/tests/native_focus_diagnosis.rs
rustfmt --check --edition 2024 crates/legion-desktop/tests/native_focus_diagnosis.rs
```

First Cargo command: two selector-assumption failures (0 passed / 1 failed each),
then 1 passed / 0 failed after source-supported selector corrections. Second
command: 1 passed / 0 failed. **Two unique diagnostic tests passed**, with no
successful test repeated. Default desktop/test compilation completed. Existing
vendor epaint float-literal warning at `tessellator.rs:2326` was left untouched.
Formatting applied and check exited 0. No offline build, full suite, or repeated
existing focus tests were requested or run. Root reserves composed docs gates.

The qualification target was explicitly released in the coordinator update
after the second test; no subsequent Cargo command uses it in this diagnosis.
Changed files are only
[the diagnostic tests](../../crates/legion-desktop/tests/native_focus_diagnosis.rs)
and this receipt. Independent review remains required before any commit.

## Next native observation plan (not executed)

The [ticket04 contract](../../.scratch/ide-2026-planning/issues/04-drive-one-real-native-open-edit-save-journey.md)
does not itself require GetFocusedElement to equal the document. That is the
current driver guard. It remains unchanged and current native results remain
blocked. Any alternative caret-based driver rule needs separate independent
review and root authorization before keys or typing can proceed. Do not bypass
the guard during diagnosis or relabel the known AT-focus defect as fixed.

For the next authorized native attempt:

1. Preserve exact candidate/package/clone identity, PID/HWND/foreground checks,
   selected Clean tab, unique complete-document baseline, editor viewport,
   exact point hit, and original disk/Git baseline. Record the existing UIA
   focus properties separately, without assuming a foreground HWND proves
   renderer pointer delivery.
2. Read the current document caret/selection. Choose a visible text position
   whose caret differs from that starting position, rather than blank canvas
   center. After one authorized OS click, observe an actual caret change inside
   the exact document using current TextPattern selection/range endpoints and
   the visible caret where available. Document text, Clean state, and disk must
   remain unchanged. If no caret change is observable, stop and preserve the
   delivery evidence; these passing component tests cannot decide its cause.
3. Only under a separately reviewed/root-approved caret guard, send a bounded
   navigation key and observe another caret change within the exact document.
   Neither a static focus label nor a successful input-send return establishes
   keyboard delivery. If this fails, stop before typing.
4. Only after both delivery checks pass, retain the existing exact complete
   baseline-plus-unique-marker oracle, Dirty state and unchanged disk before
   save, then OS Save with exact disk bytes, Clean state, and Git oracle. Do not
   reduce the document oracle to fragments or treat projection dispatch as OS
   input evidence.

This can supply the narrowly audited ticket04 journey evidence while the known
native AT-focus defect is still recorded. Full
[ADR-0056](../adrs/ADR-0056-native-input-acceptance-harness.md) pointer/AT coverage
and ticket23 remain unqualified. No further production hypothesis or speculative
repair is proposed from this code-only diagnosis.
