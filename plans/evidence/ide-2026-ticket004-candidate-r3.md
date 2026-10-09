# Ticket 004 candidate r3: native full-document observation

Date: 2026-10-09. Ticket 04 remains incomplete. The read-only Windows document
prerequisite passed; native typing/save, recovery and full input conformance
remain unqualified. No fresh attendance is inferred from the earlier ready reply.

## Reviewed source and package

Pauli independently reviewed the 19-file bounded editor accessibility repair,
identified three P2 issues, and passed the final fixes: retained-button focus
provenance, contradictory warmed-cache previews, and logical-line continuity
across text-run fragments. The reviewed patch was committed and fast-forwarded
into integration as `caf1821f4499d6af858aed9992acbedfc4cb7087`, with both worktrees
clean. The [implementation receipt](ide-2026-ticket004-editor-accessibility-prerequisite.md)
records 23 unique focused passing tests and the compile/dependency/canvas/docs/
format checks. Its pending-review wording describes the worker handoff; this
receipt records the subsequent final PASS. No full-workspace or clippy pass is
claimed.

A new clean detached checkout at `D:/legion-ide-2026-candidate-r3` freezes that
exact source. Candidate-r2 and its prior evidence were preserved.

- Offline unsigned MSI version: `0.0.3`.
- Package: `target/native-package/output/legion-desktop-windows-x64-msi.msi`.
- MSI size: 14,323,712 bytes.
- MSI SHA-256: `23a063dd26f95374fb5b9c42e856ee6fc50d3947924149c69edb9015181d7d42`.
- Extracted/staged executable SHA-256: `01629d85d9be7456d7d6cb7aadd6fdb522f718391405fd106bad82377c20b6a4`.
- Staged executable: `D:/legion-ide-2026-candidate-r3/target/native-input-acceptance/package/legion-desktop.exe`.

The package build ran once with `scripts/package-native.ps1 -Version 0.0.3
-Format wix`; release compilation finished in 2m14s and packaging exited 0.
The existing epaint warning and 42 offline app warnings remain recorded; they
were not reclassified as warning-free compilation.

`scripts/verify-native-package.ps1` ran once with the exact package directory,
version, source SHA and frozen workspace root. Its summary passed checksum,
metadata, MSI version, extraction structure and headless Manual smoke
(`smoke_exit = 0`). The temporary PowerShell default for Start-Process window
style was Hidden and was restored afterward. No frozen script was edited.

`scripts/stage-native-acceptance-package.ps1` then staged the four-file payload
from the verifier's fresh `target/release-smoke/windows-x64-msi/staging`
extraction. MSI and executable digests matched their verified sidecars.
`STAGING-EVIDENCE.toml` records unsigned status. Frozen Git status remained clean.

These are local packaging and smoke checks, not signing, clean-machine,
cross-platform, release or native editing acceptance.

## Independent read-only Windows observation

The reviewed helper at
`D:/legion-ide-2026-notes/ticket004-tab-observer-r3/` copied the final driver
observation/session sources from `6eae87d`. Pauli's helper review found one P2:
an unavailable optional selection query could block valid document observation.
After making that query diagnostic only, its affected rebuild and final review
passed. Exact document, clean tab and visible/enabled positive-area geometry
remained mandatory.

- Helper SHA-256: `597533a6fcf2d2d84a0dab97d1187ba7712e52eae292d9ec2e1c20e0cf91d686`.
- Main source SHA-256: `178623bcea1ab927c5535e3535d95bccfa672d683f99029c11ebe838f8750b98`.
- Copied observe source SHA-256: `a9bcbed41c31a53322c909f9d06b57d7e597ffa2adea1c66fbf8f17eaf941446`.
- Copied session source SHA-256: `6cc70461fb5f05d62eaf72401dd8b7c1c852e07a0d5c2f0f1f04d27ee9810362`.

The helper ran once with `--observe-clean-tab` after its and the product's hashes
were checked. It launched the packaged product with explicit `--workspace` and
`--file` diagnostic setup, queried UIA and terminated only its owned child. It
sent no input, made no explicit activation call and saved nothing. Direct file
setup is not Explorer journey evidence.

The first sample passed: selected tab `Clean`, one exact complete document,
control type 50004, enabled, on screen, and rectangle `(161,328)-(1117,656)`.
UIA returned all 8,461 bytes / 8,413 Unicode scalar values, including the CRLF
terminators, with exactly the baseline SHA-256. There were 148 UIA elements.
The optional selection query returned an empty selection; this does not qualify
native selection mutation. Observer and outer command exited 0.

Reference checkout: `D:/legion-ide-2026-tools/ticket004-reference-c2a6578`, HEAD
`c2a65786862e759e67ebac28c32c1cbc618047f0`. It remained Git-clean with README
SHA-256 `5da9ac0a7844b4f215829bc2a6523d120fdbf97e3e1759234aa12623be29bc9a`.

Raw logs, preserved outside the checkout:

- `D:/legion-ide-2026-notes/ticket004-candidate-r3-package.log`
- `D:/legion-ide-2026-notes/ticket004-candidate-r3-verification.log`
- `D:/legion-ide-2026-notes/ticket004-candidate-r3-readonly-text-patterns.log`

## Attended attempt and Explorer diagnosis

After fresh owner availability, candidate r3 ran with the reviewed
`legion-input-driver-tab-oracle-e4168202.exe`, SHA-256
`e416820272207a88f872978d99b7188a51f4a81aa460394fac1beea966052f74`.
The owner foregrounded the exact product window (PID 16776, HWND `0xb0e48`).
Foreground and UIA bootstrap passed, but navigation blocked before any driver
input: `Explorer label "README.md" must identify exactly one visible element;
found 3`. The report and outer command returned exit 3. The driver's owned-child
cleanup then closed the window; this observation does not establish an app
crash or missed owner click. The report is
`D:/legion-ide-2026-notes/ticket004-native-journey-candidate-r3-attended.toml`.

Read-only role and ancestry traces identified a README TabItem, breadcrumb Text,
and Excerpts Button. None was an Explorer file row. The earlier explicit-file
diagnostic had naturally persisted normal ignored `.legion/session.json`
metadata, causing later launches to restore README despite clean tracked Git
state. A fresh external session path supplied through the normal public
`--session-state` option produced an empty editor and only the Explorer drawer
toggle. That probe verified the default session hash was unchanged. No default
session was deleted or reset.

A separate diagnostic child of the same packaged executable then used a newly
reserved external session at
`D:/legion-ide-2026-notes/ticket004-sky-explorer-probe/session.json`. Supported
Windows desktop automation observed the actual empty editor, opened the real
Explorer drawer, and found this hierarchy:

```text
Legion IDE -> region Explorer -> dialog Explorer drawer -> button README.md
```

Clicking that dialog's README row opened the clean tab; the desktop tool reported
`focused_element = Editor document`. That field did not establish actual UIA
keyboard focus. The drawer remained open. No typing or save occurred. The diagnostic
closed its own child normally. The reference checkout remained tracked-Git-clean
with the same README SHA-256 recorded above. This validates the selector's
structural basis, not the repaired driver's native open/edit/save journey.

Preserved diagnostic artifacts outside the checkout:

- `D:/legion-ide-2026-notes/ticket004-explorer-duplicates-r3.log`
- `D:/legion-ide-2026-notes/ticket004-explorer-duplicates-context-r3.log`
- `D:/legion-ide-2026-notes/ticket004-explorer-fresh-session-r3.log`
- `D:/legion-ide-2026-notes/ticket004-sky-explorer-probe/explorer-tree.txt`
  (SHA-256 `0776020e8d3a838e74576951e5fcbdd40fe80ae8b6e78599f76f69658f7c32cb`).
- `D:/legion-ide-2026-notes/ticket004-sky-explorer-probe/explorer-drawer.jpg`
  (SHA-256 `99d4a893dd97cd9e53dd7cbc80ce45eb0f3d6a9ecc193bb4c3d3d4415dbc0e4e`).
- `D:/legion-ide-2026-notes/ticket004-sky-explorer-probe/readme-tree.txt`
  (SHA-256 `d6062394e18394a96cac6639138b83bd5bfc68e727ba9c11da96e85a84698ab1`).
- `D:/legion-ide-2026-notes/ticket004-sky-explorer-probe/readme-open.jpg`
  (SHA-256 `7a8d3e27da9b8a1b31b3fba51c87078a39d12ea368dbfe2a04391651e0c4364c`).
- `D:/legion-ide-2026-notes/ticket004-sky-explorer-probe/readme-focus.json`
  preserves the same observation's `focused_element = "100 edit Editor document"`.

## Bounded driver repair checks

The three-file driver patch scopes exact Button selection to the unique named
Explorer Dialog's descendants. Tabs, breadcrumbs and Excerpts cannot satisfy
that scope. Missing/ambiguous/read-failed observations block. Unsupported wide
layouts without a proven Explorer container also block. Each journey reserves
a new report and external session directory and passes the normal public
`--session-state` option; existing reports or session directories are refused.

After opening a file, the driver closes the scoped drawer and waits for its
absence. Pointer targets require current enabled/visible positive bounds, a
center inside the product client and any available drawer bounds, and an exact
UIA point hit. Only the verified semantic Dialog's all-zero bounds are treated
as unavailable, with that fact recorded explicitly. Typing additionally requires
the exact document element to own keyboard
focus. These guards retain the per-input exact-foreground checks and exact
complete-document, disk and Git oracles.

From `D:/legion-ide-2026-workers/ticket-004`, these filters ran with
`cargo test -p legion-input-driver --test driver_contract <filter> --target-dir
D:/legion-ide-2026-tools/native-input-target`:

| Filter | Passing tests |
| --- | --- |
| `explorer_scope_` | 2 |
| `journey_session_` | 2 |
| `journey_click_bounds_` | 1 |
| `explorer_` | 3 after drawer-close changes affected the shared snapshot |
| `journey_editor_focus_` | 1 |
| `journey_click_hit_` | 1 |
| `native_journey_missing_package_` | 1 |
| `attended_journey_flag_` | 1 |

That is 10 distinct tests and 12 successful test executions. New APIs initially
produced compile-time reds; runtime assertion failures before implementation are
not claimed. The original attended duplicate-label report remains the actual
native failure evidence. The driver build with the same target directory,
rustfmt checks of all three changed files, and `git diff --check` also exited 0.
Pauli independently passed the unchanged driver and diagnostic documentation;
that review did not establish native point-hit or focus qualification.

## Direct native guard preflight

The coordinator resumed the existing isolated diagnostic child (PID 62208) after
the owner explicitly reinvoked Computer Use. The normal session and README
baseline hashes were captured before launch. An external read-only Rust helper
at `D:/legion-ide-2026-notes/ticket004-scoped-oracle-probe/` copies the current
driver's observation/session code, verifies the existing PID's exact r3 image
path and digest, and has no input, activation or child lifecycle operation.
Supported desktop automation supplies navigation separately. Helper exit 0
means only a read-only prerequisite passed, never native journey acceptance.

The drawer toggle passed client containment and exact UIA point hit at
`(291,947)`. After the coordinator opened the drawer, the file-row sample
blocked because the semantic Dialog publishes an all-zero rectangle. The
actual row bounds were `(343,552)-(422,570)` and the native client bounds were
`(235,258)-(1195,978)`. A labelled diagnostic without the unavailable container
rectangle passed client containment and exact target hit at `(382,561)`; the
original guard still returned blocked. Source inspection confirms that the
Dialog node supplies role and label without bounds. Pauli approved a narrow
exception for this verified semantic container's all-zero rectangle, retaining
mandatory positive target/client geometry, exact subtree membership and point
hit. Positive container bounds remain binding; malformed or unreadable
properties still block. The implemented exception passed one new focused test
and the two affected click tests, followed by driver build, formatting and diff
checks. The final patch therefore has 11 distinct focused tests and 15 successful
test executions across the two bounded stages. Pauli passed the final delta.

The final native file/close sample passed with the tab, breadcrumb and Excerpts
README labels also present: exact scoped file point `(383,561)` and close-button
point `(315,329)`, both with client containment and exact target hit. The helper
sent neither action. The final helper SHA-256 is
`5dc869bcceefc3d227886899122bc3d6e34defd78212eff0136b5392c44c5ee1`;
the reviewed driver binary SHA-256 is
`c20e50aed405d037dffa3160587f7e38c681bd1a961a2c6320e9be70ec5ec421`.
The three reviewed driver files were committed and fast-forwarded into
integration as `5d7b298f9f7bdd20441a7796aec9f8ecb43f6f41`. The identical binary is
archived without overwriting earlier drivers at
`D:/legion-ide-2026-tools/legion-input-driver-scoped-explorer-c20e50ae.exe`.
The final test filters were `verified_zero_explorer_scope_` (one test) and
`journey_click_` (the two affected click tests), using the same command prefix
and target directory recorded above. No native typing/save acceptance follows
from this partial driver commit.

After desktop automation opened README, dismissed the drawer and explicitly
clicked the editor, the read-only helper observed the exact complete baseline,
Clean tab, no drawer, and exact editor point hit at `(715,568)`. Its UIA focused
element comparison nevertheless returned false. The desktop tool's separate
`focused_element` field reported Editor document; that field is not substituted
for the failing driver oracle. No text or save was sent.

An instrumented sample confirmed that the exact candidate HWND remained
foreground. `GetFocusedElement` identified its `Legion IDE` root (type 50032,
PID 62208), while the document reported both `CurrentHasKeyboardFocus = false`
and `CurrentIsKeyboardFocusable = false`. Source review traced the mismatch to
editor keyboard routing without corresponding egui/accessibility focus
ownership; egui falls back to the accessibility root. The exact focus guard is
retained. A bounded product focus-projection repair is required and assigned;
another attended run remains gated on that repair's review and native evidence.

Raw samples are preserved under
`D:/legion-ide-2026-notes/ticket004-scoped-oracle-native/`: `drawer.log`,
`file.log`, `file-geometry.log`, `file-hit-diagnostic.log`, `editor.log`, and
`editor-after-click.log`, and `editor-focus-diagnostic.log`. The diagnostic helper
changed only to print relevant geometry/focus metadata; successful prior checks
were not counted as full acceptance.

`file-and-close-final.log` records the final passing navigation guard sample.
`final-scoped-tree.txt` and `final-scoped-window.jpg` preserve the corresponding
native tree and screenshot. The coordinator closed only its diagnostic child;
README and the normal session metadata retained their pre-launch hashes and the
reference checkout remained tracked-Git-clean. The product focus prerequisite
remains unresolved on candidate r3.

## Remaining attended gate

Fresh attendance is required for the next attended run after the repair's
remaining review and native prerequisite checks. The bounded
journey does not cover scenario steps 7-8, all recovery cases, CJK/full input
conformance or human pilot durations.

The product repair deliberately supports bounded small documents. Complete
large/degraded-document accessibility and native accessibility actions remain
separate requirements; no full-document claim is made for constrained coverage.
