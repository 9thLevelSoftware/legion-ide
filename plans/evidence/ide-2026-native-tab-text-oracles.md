# Native tab and document oracle checkpoint

Date: 2026-10-09. Ticket 04 remains needs-info. Native clicks now succeeded;
complete open/edit/save and input conformance remain unqualified.

## Attended access-preserving driver run

After the owner replied "ready now", the coordinator ran the archived `b88f7e0`
driver `legion-input-driver-preserved-access-b8a44dd7.exe`, verified SHA-256
`b8a44dd7c28920b9326317a494b79d06b156d55a0c649b832822aa1b51f7f7bc`.
The product/reference identities remain those in the
[journey receipt](ide-2026-ticket004-native-journey.md).

Report: `D:/legion-ide-2026-notes/ticket004-native-journey-attended-preserved-access.toml`.
The owned product was PID 53584, HWND `0x27088c`. Exact foreground and UIA startup
passed. The driver clicked the Explorer drawer once, found the exact visible
README.md entry, and submitted its guarded file-selection click. The former
SendInput access-denied failure did not recur. It then blocked on its clean-tab
name predicate, with report code 3 / outer PowerShell code 1. It did not reach
text entry or save. Reference Git status was empty and README SHA-256 remained
`5da9ac0a7844b4f215829bc2a6523d120fdbf97e3e1759234aa12623be29bc9a`.

## Correct published tab properties

The old driver searched accessible names for a debug-style `[buffer ...]` suffix
and ` +` dirty marker. At frozen product source `07d2187`, `view/tab_strip.rs`
publishes a Tab with the exact filename, selected state and description
`Unsaved changes` when dirty. `view.rs::tab_rows` produces the debug-style string,
but that string is not the native tab's accessibility label. Cached
`accesskit_windows 0.32.1/src/node.rs` maps Tab to UIA TabItem, selection to
SelectionItem, and description to FullDescription (not HelpText).

On base `b88f7e0`, the driver now requires one visible exact-name TabItem,
SelectionItem.IsSelected=true, and FullDescription either empty (clean) or exactly
`Unsaved changes` (dirty). Missing/ambiguous/inactive/hidden targets, property read
errors and unknown descriptions block. The oracle is used before editing, after
typing and after saving. Full-document comparison, per-input foreground guards,
exact disk bytes and Git checks remain unchanged. No product source was edited
in this slice.

Independent review found one P2: the shared subtree enumeration skipped failed
GetElement calls, allowing a partial tree to appear unique. The coordinator
changed that path to propagate the indexed error. No inaccessible element can
silently disappear from a uniqueness check.

## Read-only native oracle observations

The isolated helper under `D:/legion-ide-2026-notes/ticket004-tab-observer/` opened
the same frozen product with explicit `--workspace` and `--file README.md` setup,
queried UIA, and closed only its owned child. It did not activate a window,
inject input or save. This setup is a read-only oracle diagnostic and cannot
substitute for Explorer input or the native journey.

- `ticket004-readonly-tab-oracle.txt`: ten samples reported the selected target
  tab as Clean. All ten found zero full-document TextPatterns; helper exit 1.
- `ticket004-readonly-text-patterns.txt`: a follow-up adds one bounded pattern
  inventory. README contains 8461 UTF-8 bytes / 8413 Unicode scalar values; the
  tree had 147 elements. Observed TextPatterns were StaticText fragments such as
  visible line text, line numbers and labels, without an exact complete document.
  Ten tab samples remained Clean; the full-document check remained unavailable.
  This helper also exited 1. Neither failure is reclassified as a full pass.

Both logs are in `D:/legion-ide-2026-notes/`. The helper copied the tab-oracle
source before the final strict-enumeration review fix; its successful clean-tab
observations do not exercise enumeration-error handling. Its product digest was
`184c81243778b12aa2dadccc51a33bc0551dfaa1eb4f5c607f96449cb12218b5` and baseline
digest matched the unchanged README above.

The final diagnostic helper executable SHA-256 was
`df703152d807c6f8af9e88107a89328479ddc72e7bf4c992b0d207f93ae06a3a`;
its copied `observe.rs` SHA-256 was
`99ef894cfbaa1c2ca36a1a17bdaee727e5d5fe2a947de2c0f04ad125fc547090`.
The reference checkout was checked again after both probes: clean Git status and
the same README hash.

The next bounded prerequisite is a product-side editor text accessibility repair
using app-produced text projections and bounded snapshot APIs. Turing owns the
isolated `native-editor-accessibility` worktree. No text-oracle weakening,
test-only product hook or incomplete-prefix substitute is authorized by this
receipt. No further attended run should occur until the document oracle is
available on the identified candidate.

## Focused checks

- `cargo test --locked -p legion-input-driver --test driver_contract selected_tab_oracle_ --target-dir D:/legion-ide-2026-tools/native-input-target`: initial compiler-red on missing observation API, then 2 passed / 25 filtered. Logs `ticket004-tab-oracle-red.log` and `ticket004-tab-oracle-green.log`. These model checks cover clean/dirty/clean and negative ambiguity/name/selection/visibility/unknown-state cases; they are not native dirty/save evidence.
- `cargo build --locked -p legion-input-driver --target-dir D:/legion-ide-2026-tools/native-input-target`: passed after the strict-enumeration repair, 0.47 seconds.
- `rustfmt --check --edition 2024 crates/legion-input-driver/src/observe.rs crates/legion-input-driver/src/journey.rs crates/legion-input-driver/tests/driver_contract.rs`: passed after formatting those files.

Built driver SHA-256:
`e416820272207a88f872978d99b7188a51f4a81aa460394fac1beea966052f74`.
`git diff --check` and existing xtask `docs-hygiene` passed. Pauli's final
independent review passed all five files after confirming the P2 enumeration fix;
no material findings remain in this slice. No acceptance checkbox is promoted;
observed pointer progress is distinct from the blocked document stage.
