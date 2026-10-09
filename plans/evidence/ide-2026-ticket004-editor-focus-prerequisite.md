# Ticket 004: editor keyboard focus prerequisite

Date: 2026-10-09. Implementation checkpoint; native candidate-r4 qualification
remains pending. Ticket 04 and full input conformance remain unaccepted.

## Observed problem and bounded change

The [candidate-r3 receipt](ide-2026-ticket004-candidate-r3.md) records the real
Windows failure: the exact product HWND remained foreground, the clean tab and
complete document matched, and the editor center passed exact UIA hit testing.
After an actual editor click, however, `GetFocusedElement` still returned the
Legion window root and the document reported no keyboard focus. The desktop
tool's separate focus label was not substituted for that failing oracle.

The repair connects genuine canvas keyboard ownership to egui and AccessKit
focus after code clicks and file activation when no other control owns focus.
It retains focus while the canvas is present, relinquishes it when the active
buffer disappears, and distinguishes canvas focus from button focus in existing
keyboard routing. Arrow filtering covers both normal navigation and the first
arrow after activation. Other controls retain their own focus and text input.
The background pointer interaction publishes no competing accessibility node.

The UI remains a projection/intent layer. Editor buffers, proposal-mediated
workspace saves, text publication budgets and Manual-mode policy keep their
existing authorities. This change does not advertise an accessibility Focus
action or qualify unsupported native accessibility actions.

Changed Rust files:

- `crates/legion-desktop/src/view.rs`
- `crates/legion-desktop/src/view/editor_accessibility.rs`
- `crates/legion-desktop/src/workflow.rs`
- `crates/legion-desktop/tests/editor_document_accessibility.rs`
- `crates/legion-desktop/tests/editor_keyboard_focus.rs` (new)

## Targeted verification

All Cargo commands below ran from
`D:/legion-ide-2026-workers/native-editor-accessibility` with
`--target-dir D:/legion-ide-2026-tools/qualification-target`.

Seven new public renderer-event contracts in `editor_keyboard_focus` passed:

- Background click retains focus through text input, Ctrl+Home and exact save.
- Initial file activation owns otherwise unclaimed keyboard focus.
- Clicking text after palette use retains editing through Enter and Space.
- Arrow keys move the caret while preserving document focus across frames.
- The first arrow immediately after file activation preserves document focus.
- Palette text input and focus survive another file's activation.
- Real Explorer opening followed by editor click publishes focus and routes text.

Each ran with `cargo test -p legion-desktop --test editor_keyboard_focus <name>
-- --exact`. Four earlier contracts were rerun after the shared focus-filter
repair; the later first-arrow, palette and Explorer contracts were excluded from
that affected rerun. Behavioral reds demonstrated root focus after pointer
click, missing initial focus, spatial-arrow transfer and first-arrow timing.
Duplicate-label fixture lookup failures were corrected separately and are not
claimed as product behavioral reds.

The following affected regression checks also passed:

- Three individual `editor_document_accessibility` tests:
  `review_unsolicited_document_focus_does_not_reclassify_retained_rail_focus`,
  `qualification_unsolicited_accessibility_focus_preserves_enter_and_space_input`,
  and `qualification_large_degraded_document_omits_complete_text`.
- `cargo test -p legion-desktop --test manual_input_conformance --test desktop_workflow`
  (seven tests).
- `cargo test -p legion-desktop --test keyboard_nav product_mode_escalation_supports_keyboard_confirm_escape_and_focus_restoration -- --exact`
  (one test).

That is 18 distinct focused passing tests. Further passing checks:

```text
cargo check -p legion-desktop --all-targets
cargo check -p legion-desktop --lib --bins --no-default-features --features offline
cargo run -p xtask -- no-egui-textedit
rustfmt --check --edition 2024 --config skip_children=true <five changed Rust files>
git diff --check
```

The new test file also passed a whitespace check. Existing epaint and offline
app warnings remain; no warning-free, full-workspace or clippy pass is claimed.
Pauli independently passed the final five-file patch, including arrow handling,
other-control focus and the absence of a competing background accessibility
node. The reviewed source was committed and fast-forwarded into integration as
`fb9982f01562be9121b6d0d578792b9be8789948`; the product worker is clean. Packaging
and native focus evidence remain pending.

## External activation provenance

Computer Use can activate the exact returned packaged window during the existing
driver's bounded `--await-foreground` wait. Human foregrounding is not required
by ADR-0056's external OS-input model. A four-string driver correction reports
`foreground_mode=external-foreground-wait` and
`driver_automatic_activation=false`; it changes no flag, deadline, HWND check,
exit handling or input/focus guard. Its existing wait-contract test and build
passed once. The reviewed metadata correction is committed locally as
`7512cca067a4afd784c1cf95d8c33a7e675bb346`.

Prepared driver archive:
`D:/legion-ide-2026-tools/legion-input-driver-external-foreground-6b6da3a1.exe`,
SHA-256 `6b6da3a1ca42f34c7e5ce3ec14f78b43898ea72b2d02cd9df6eda6aad292b697`.
The eventual run must record `activation_source=computer-use`, exact PID/HWND,
candidate and driver identities, and the unchanged native/disk/Git oracles.
Automated activation is not human-attendance or pilot-duration evidence.

Candidate r3 and its failures remain preserved. The r4 plan is outside the
checkout at `D:/legion-ide-2026-notes/ticket004-candidate-r4-plan.md`. Native
typing/save, scenario recovery, full/CJK input, large-document accessibility,
other platforms, human pilots and signing remain separate gates.
