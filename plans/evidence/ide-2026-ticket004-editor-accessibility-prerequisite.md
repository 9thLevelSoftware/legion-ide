# Ticket 004 bounded editor accessibility prerequisite

Date: 2026-10-09. Worker: `codex/ide-2026-native-editor-accessibility`.
Source integration base: `6eae87d9c074523a779af19908287c19b71f4ad3`.
Disposition: bounded small-buffer implementation ready for independent source
review. Native UIA/TextPattern qualification and candidate packaging are pending.
This receipt does not establish native input, release, or production acceptance.

## Approved scope and baseline

The initial assessment stopped because the existing projection offered complete
small-buffer preview text but no exact whole-document large-source port or
editability metadata. The coordinator then explicitly approved complete bounded
small-buffer accessibility using `small_buffer_preview`, narrow app/UI metadata,
and necessary constructors/tests. ADR-0015's large/degraded restriction remains
absolute. No new snapshot/chunk read seam or large-source accumulation was added.

The coordinator-supplied read-only receipt
`D:/legion-ide-2026-notes/ticket004-readonly-text-patterns.txt` reported the clean
selected tab, but zero exact complete README TextPatterns (8,461 bytes,
8,413 Unicode scalar values; 147 enumerated elements). The visible code labels
provided fragments only. The reviewed driver correction is in integration
`6eae87d` and `plans/evidence/ide-2026-native-tab-text-oracles.md`. This worker
performed no native probe, launch, OS input, or independent reproduction of those
native observations. Frozen candidates and strict baseline/marker/disk oracles
were left untouched.

## Implemented authority and coverage

`ActiveBufferProjection` now has optional app-produced accessibility metadata:
buffer/snapshot/version identity, exact byte length, editability, small-preview
coverage, and primary directed anchor/head byte endpoints. Identity comes from
`EditorEngine::current_snapshot`; endpoints come from `directed_carets` and
`buffer_byte_offset`, never normalized viewport selections.

`EditorEngine` currently has no read-only buffer mode. `BufferMode` is Normal or
Degraded, and `apply_edits_with_caret_policy` accepts buffer transactions without
a file-permission, workspace-trust, clean-state, or save-approval editability
flag. The app therefore projects `editable: true`; no fictional read-only mode
was introduced. Public app/rendered tests verify editability and insertion in an
untrusted workspace. The desktop text-input role consumes that flag independently
of save authorization. Workspace proposal, trust, fingerprint, version,
generation, and conflict gates continue to govern disk writes.

The dedicated desktop helper publishes the actual code viewport's
`MultilineTextInput` node and ordered `TextRun` children, outside viewport text
virtualization. Its globally stable control ID is `legion_editor_document` and
its accessible label is `Editor document`. That control retains identity across
edits and tab switches so a native observer can keep its TextPattern object.
The parent bounds are the visible code area intersected with the UI clip rect,
not full scroll content. Text runs contain source only, without gutters or
line-label decorations. Rendered consumer updates remove trailing old-tab runs;
closing the last clean tab removes the control. Dirty close continues to prompt
and preserve the document.

Before text allocation the helper requires matching buffer, snapshot and
version, Normal viewport mode, non-degraded Full projection, an exact preview,
matching full byte length, and byte-budget eligibility. Missing/invalid metadata,
missing preview, degraded source, or a publication budget/representation failure
produces an `Editor document` Group with `Complete text unavailable: <reason>`,
no text-run children and no document range. App preview eligibility does not
promise that the desktop's additional run/metadata budgets will accept it.

## Publication budgets and caching

- Exact UTF-8 text: at most 1 MiB, below the existing 5 MiB snapshot full-cache limit.
- Selectable extended grapheme units: at most 65,536.
- Text-run children: at most 1,024, plus the one parent control.
- Character-length payload: at most 65,536 bytes, one `u8` per unit.
- Selection byte-boundary entries: at most `(65,536 + 1,024) * size_of::<usize>()`
  bytes (532,480 bytes on this Windows 64-bit target).
- Each run contains at most 255 units. A unit exceeding the `u8` byte-length
  representation causes unavailable coverage; it is never split or truncated
  to manufacture a complete document.

Unicode segmentation preserves combining sequences, non-BMP text and CRLF
(as one selectable unit), original newline bytes and trailing/blank lines.
An empty run supplies an actual empty document range and end-of-document caret.
Immutable run segmentation is cached by app identity/coverage/editability;
caret-only changes do not invalidate it. Egui creates fresh accessibility nodes
each enabled frame, so the helper copies cached nodes for that mandatory update
without re-segmenting or assembling another full document string. These are
individual payload/node limits, not a measured aggregate heap/per-frame timing
claim. No performance benchmark was run.

The text model's 5 MiB full-cache budget and existing bounded source APIs are
unchanged. The helper does not call `TextSnapshot::text`, materialize large
snapshots, add a TextEdit-owned editor buffer, or obtain editor/write ownership.
ADR-0032 and dependency-policy notes record this existing adapter boundary.
`unicode-segmentation` reuses workspace pin `=1.13.2` for display units only;
`accesskit_consumer 0.36` is dev-only and was already resolved in the lockfile.
No new package version or internal crate edge was introduced. The grapheme gate
now allows the pinned desktop declaration only with the explicit ticket-004
policy authorization; editor/app/UI ownership remains forbidden.

Pauli's three P2 source findings were repaired in the approved adapter scope.
Same-identity previews are compared against cached run value bytes through an
iterator, without allocating another full string. Contradictory bytes poison
that cache entry as unavailable until fresh identity; intervening unavailable
projection frames do not clear the entry. Storage fragments receive reciprocal
`previous_on_line`/`next_on_line` links using each frame's run IDs, only when the
predecessor has no actual LF/CRLF terminator. Consumer line queries therefore
observe the complete logical line rather than 255-unit storage boundaries.

## Native action and geometry limitations

Focus, SetTextSelection and ScrollIntoView are not advertised or implemented by
this helper. Current editor caret/selection is observable, but native range
mutation is unqualified. Pauli's source review reported that Windows
`accesskit_windows 0.32.1` can forward SetFocus, TextRange.Select and
ScrollIntoView and return success even for unadvertised/ignored actions. This
slice does not fork that adapter and does not claim guaranteed rejection or
observation-only enforcement.

The semantic child UIs use egui's default non-focusable hover sense. Egui routes
an accessibility focus request through its focus-interest registry; these nodes
do not register focus interest or a TextEditState. The headless full-product
regression injects an unsolicited Focus request at the actual editor node, then
verifies that ordinary Enter and Space still edit the buffer. This is not native
Windows action qualification. Per-run hit-testing/bounding rectangles, scroll
range actions, and native screen-reader behavior remain unqualified; only the
parent's positive visible viewport bounds are covered headlessly.

The narrow `workflow.rs` provenance fix additionally requires a ROOT-tree Focus
request whose target node equals the widget actually focused after paint.
An ignored semantic-document request cannot reclassify unrelated retained rail
focus as navigation. A retained actual rail-button regression verifies Enter and
Space reach the editor, and the existing control-focus/modal activation and
restoration regression verifies legitimate accessibility Focus still works.

## Behavioral reds and corrected assumptions

The pre-implementation behavioral reds were:

1. `complete_document_preserves_unicode_newlines_and_offscreen_text`: one
   complete editor control expected, zero found.
2. `active_document_metadata_names_exact_editable_small_snapshot`: app metadata
   expected, absent.
3. `selection_preserves_directed_unicode_endpoints_without_advertising_actions`:
   primary directed caret expected, not published.

Separately, two test-fixture assumptions were corrected, not counted as missing
product behavior. The selection fixture initially used UTF-16 column 4 where
this typed pointer seam requires UTF-8 byte column 7, yielding
`NotUtf8Boundary { offset: 4 }`; after correction it reached behavioral red 3.
The lifecycle test initially assumed Undo made a buffer saved/clean. It instead
correctly retained dirty state and a close prompt. The corrected test observes
that preservation and uses the public close route on a clean final tab to verify
control removal. Only affected failed checks were rerun.

The public dependency gate also produced a supported policy red on the new
approved desktop Unicode declaration: `only legion-text may own it`. The narrow
policy/enforcement amendment preserves pinning and rejection of other owners.

Three review behavioral reds were observed separately, one vertical slice each:

1. `review_unsolicited_document_focus_does_not_reclassify_retained_rail_focus`:
   retained real rail focus plus ignored document Focus dropped Enter; actual
   text was `" seed"` instead of `"\n seed"`.
2. `review_warm_cache_rejects_contradictory_preview_until_fresh_identity`:
   warming `alpha`, then projecting same-identity/same-length `bravo` incorrectly
   retained MultilineTextInput rather than publishing unavailable coverage.
3. `review_storage_fragments_preserve_consumer_logical_line_queries`:
   the first query of a 300-grapheme line stopped at 255 instead of including the
   full line and CRLF terminator.

Two further test assumptions were corrected separately from these behavioral
reds. The initial headless Explorer click did not retain widget focus, so the
fixture uses public egui memory to restore the real rendered button's ID without
a navigation event. After the line-link repair all three real line queries were
correct, but an out-of-range assertion assumed AccessKit immediately returned
None: this consumer exposes a final document-end empty range first. The test now
checks that documented source behavior, then None at the following index. Only
these affected failed checks were rerun; no second product repair was required.

## Exact verification commands and counts

All Cargo commands below used the one qualification target. The one public-app
and ten rendered/product tests passed before the pause; they were not repeated
when resuming for compatibility and repository gates.

```text
cargo test -p legion-app --test editor_accessibility_projection --target-dir D:/legion-ide-2026-tools/qualification-target active_document_metadata_names_exact_editable_small_snapshot -- --exact
  behavioral red, then green: 1 passed

cargo test -p legion-desktop --test editor_document_accessibility --target-dir D:/legion-ide-2026-tools/qualification-target complete_document_preserves_unicode_newlines_and_offscreen_text -- --exact
  behavioral red, then green: 1 passed

cargo test -p legion-desktop --test editor_document_accessibility --target-dir D:/legion-ide-2026-tools/qualification-target selection_preserves_directed_unicode_endpoints_without_advertising_actions -- --exact
  fixture error, corrected fixture reached behavioral red, then green: 1 passed

cargo test -p legion-desktop --test editor_document_accessibility --target-dir D:/legion-ide-2026-tools/qualification-target qualification_ -- --nocapture
  5 passed, 1 lifecycle-assumption failure
cargo test -p legion-desktop --test editor_document_accessibility --target-dir D:/legion-ide-2026-tools/qualification-target qualification_stable_document_updates_after_edits_and_removes_closed_tab_runs -- --exact
  corrected lifecycle expectation: 1 passed; the other 5 were not repeated

cargo test -p legion-desktop --test editor_document_accessibility --target-dir D:/legion-ide-2026-tools/qualification-target edge_ -- --nocapture
  2 passed

cargo test -p legion-desktop --test manual_input_conformance --test desktop_workflow --target-dir D:/legion-ide-2026-tools/qualification-target
  7 passed: 3 IME/clipboard/palette input tests and 4 editing/save/conflict/quit tests

cargo check -p legion-desktop --all-targets --target-dir D:/legion-ide-2026-tools/qualification-target
  passed, including affected desktop fixture constructors
cargo check -p legion-desktop --lib --bins --no-default-features --features offline --target-dir D:/legion-ide-2026-tools/qualification-target
  passed for the offline native product feature set

cargo test -p xtask --bin xtask --target-dir D:/legion-ide-2026-tools/qualification-target grapheme_dependency_gate_covers_pin_owner_and_policy_cases
  1 passed: pinned policy-authorized desktop accepted; absent authorization,
  unpinned desktop and other owners rejected

cargo check -p legion-ui --tests --target-dir D:/legion-ide-2026-tools/qualification-target
  passed, including affected UI fixture constructors

cargo run -p xtask --target-dir D:/legion-ide-2026-tools/qualification-target -- check-deps
  policy red described above, then passed after narrow authorization/enforcement fix

cargo test -p legion-desktop --test editor_document_accessibility --target-dir D:/legion-ide-2026-tools/qualification-target review_unsolicited_document_focus_does_not_reclassify_retained_rail_focus -- --exact
  fixture setup correction, behavioral red, then green: 1 passed
cargo test -p legion-desktop --test editor_document_accessibility --target-dir D:/legion-ide-2026-tools/qualification-target review_warm_cache_rejects_contradictory_preview_until_fresh_identity -- --exact
  behavioral red, then green: 1 passed
cargo test -p legion-desktop --test editor_document_accessibility --target-dir D:/legion-ide-2026-tools/qualification-target review_storage_fragments_preserve_consumer_logical_line_queries -- --exact
  behavioral red, repaired line queries passed with terminal-range assumption
  failure, corrected terminal-range expectation: 1 passed
cargo test -p legion-desktop --test keyboard_nav --target-dir D:/legion-ide-2026-tools/qualification-target product_mode_escalation_supports_keyboard_confirm_escape_and_focus_restoration -- --exact
  1 passed: legitimate Focus, modal activation, Tab/Shift+Tab, Escape/restoration
```

Formatting used `rustfmt --edition 2024 --config skip_children=true` on the
changed tracked/untracked Rust paths, followed by the same formatter on
`xtask/src/main.rs` after the policy amendment. Review repairs were formatted
with the same command on the helper, workflow and rendered test file.
The existing vendored epaint future float-literal warning was observed; offline
compilation additionally reported 42 app feature/unused-code warnings in
unchanged code. Neither check used warning denial or establishes clippy success.

```text
$changedRust = @((git diff --name-only -- '*.rs'), (git ls-files --others --exclude-standard -- '*.rs')) | ForEach-Object { $_ } | Sort-Object -Unique
rustfmt --check --edition 2024 --config skip_children=true $changedRust
  passed before Pauli repairs; unchanged Rust files were not rechecked
rustfmt --check --edition 2024 --config skip_children=true crates/legion-desktop/src/workflow.rs crates/legion-desktop/src/view/editor_accessibility.rs crates/legion-desktop/tests/editor_document_accessibility.rs
  passed on the final three affected Rust files

cargo check -p legion-desktop --all-targets --target-dir D:/legion-ide-2026-tools/qualification-target
cargo check -p legion-desktop --lib --bins --no-default-features --features offline --target-dir D:/legion-ide-2026-tools/qualification-target
  both passed again only because the reviewed product-source repairs changed
  desktop compilation after the prior checks; no successful tests were repeated

cargo run -p xtask --target-dir D:/legion-ide-2026-tools/qualification-target -- no-egui-textedit
  passed
git merge --ff-only codex/ide-2026-integration
  Already up to date at 6eae87d; no commit or overwrite
git status --short (original D:/legion-ide)
  matched the preserved original WIP listed below
```

Unique passing tests across these focused invocations: 23 (1 public app, 13
editor-document renderer/product contracts, 7 input/save regressions, 1 dependency
policy contract and 1 legitimate control-focus regression). This is not a
workspace-wide test, clippy, native or production qualification result.

## Changed file ownership

- App/UI projection and constructors: `crates/legion-app/src/lib.rs`,
  `crates/legion-ui/src/ui.rs`, `crates/legion-ui/src/ui_shell_tests.rs`.
- Desktop view/helper and narrow focus provenance: `crates/legion-desktop/src/view.rs`,
  `crates/legion-desktop/src/view/editor_accessibility.rs`,
  `crates/legion-desktop/src/workflow.rs`.
- Dependency declarations/enforcement: `Cargo.lock`,
  `crates/legion-desktop/Cargo.toml`, `xtask/src/main.rs`.
- New focused contracts: `crates/legion-app/tests/editor_accessibility_projection.rs`,
  `crates/legion-desktop/tests/editor_document_accessibility.rs`.
- Necessary existing desktop fixture constructors:
  `crates/legion-desktop/src/view/streamed_multiline_projection_tests.rs`,
  `crates/legion-desktop/tests/accessibility.rs`,
  `crates/legion-desktop/tests/manual_renderer_evidence.rs`,
  `crates/legion-desktop/tests/projection_rendering.rs`,
  `crates/legion-desktop/tests/user_journey_rendering.rs`.
- Architecture/evidence: `plans/adrs/ADR-0032-editor-render-path.md`,
  `plans/dependency-policy.md`, and this new receipt.

## Work preservation and remaining gates

No commit, push, native launch/input, live provider, packaging, or frozen-product
edit was performed. Integration `6eae87d` contains the coordinator's reviewed
input-driver changes; this worker did not edit that driver. The original
workspace's modified `GLOSSARY.md`/`docs/INDEX.md` and untracked `.scratch/`,
`.omp/plans/FRONTIER_INTEGRATION_PLAN.md`, and research evidence were preserved.

Pauli full source review found the three P2 issues recorded above; fixes await
independent re-review. The coordinator owns offline
MSI rebuild, the read-only r3 exact/unique/visible/enabled positive-area
TextPattern probe, and any later attended edit/save journey. Complete large
source remains deliberately unavailable and requires a separate reviewed
architecture/policy and native-range qualification effort. No acceptance or
zero-egress relaxation is implied by these local checks.

## Final gate receipt

```text
cargo run -p xtask --target-dir D:/legion-ide-2026-tools/qualification-target -- docs-hygiene
  passed: documentation hygiene checks passed
git diff --check
  passed: no whitespace errors; only Git LF-to-CRLF notices
git diff --no-index --check -- NUL plans/evidence/ide-2026-ticket004-editor-accessibility-prerequisite.md
  no whitespace errors; exit 1 denotes the new-file difference, only LF-to-CRLF notice
```

This receipt-only result appendix was added after the successful documentation
and whitespace gates; those successful gates were not repeated. All source
repairs and architecture notes were present when the gates ran. The work remains
uncommitted for Pauli's independent re-review and coordinator-owned native gates.
