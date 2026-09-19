# ADR-0056: Native input acceptance harness

- Status: Proposed
- Date: 2026-09-08
- Owners: desktop platform / production qualification
- Requirement: `COMP-PLAT-002` (package `S1-02`), supporting `COMP-P1-F1-T1-1`
- Depends on: [ADR-0048](ADR-0048-renderer-strategy.md) (renderer strategy and
  the custom code canvas), the `windowed-gui-e2e` command in
  [`xtask/src/windowed_gui_e2e.rs`](../../xtask/src/windowed_gui_e2e.rs),
  blocker `BLK-2026-09-08-02` in
  [`plans/completion/decisions.md`](../completion/decisions.md)
- Supersedes: none

## Context

`COMP-PLAT-002` in
[`plans/completion/requirements.json`](../completion/requirements.json) requires
that the **packaged** native input path carries keyboard, pointer, text,
clipboard, IME/CJK and command input through the authoritative UI/app route.
The row is `implementation: partial`, `acceptance: unassessed`, and it carries
both open defects in the register
([`plans/completion/defects.json`](../completion/defects.json)).

Two things are missing, and they are different things:

1. **An instrument.** `xtask windowed-gui-e2e` builds an unsigned layout and
   runs one open/edit/save journey. It proves a window can be created and one
   journey completes. It observes no input class, has no per-class result, and
   has no notion of a driver being unavailable.
2. **A truthful unavailable result.** The recurring failure in this codebase is
   a harness that cannot reach its subject, exits 0, and is later read as a
   pass. `windowed-gui-e2e` already guards the inverse case at one point —
   *"exit 0 without `window_created = true` is not GAP-01.1"* — but it has no
   vocabulary for "this machine cannot answer the question".

The existing in-process substrates —
[`crates/legion-desktop/tests/input_conformance.rs`](../../crates/legion-desktop/tests/input_conformance.rs),
[`manual_input_conformance.rs`](../../crates/legion-desktop/tests/manual_input_conformance.rs),
[`clipboard_smoke.rs`](../../crates/legion-desktop/tests/clipboard_smoke.rs) and
[`ime_smoke.rs`](../../crates/legion-desktop/tests/ime_smoke.rs) — are component
evidence. They feed synthesized `egui` events into an in-process harness. They
cannot distinguish a working native input path from a test harness talking to
itself, because both the sender and the receiver are the same process. Whatever
they prove, they cannot prove `COMP-PLAT-002`.

## Decision

A separate command, `xtask native-product-acceptance`, implemented in
[`xtask/src/native_product_acceptance.rs`](../../xtask/src/native_product_acceptance.rs),
is the native input acceptance harness. It is defined by three properties:

1. **Out-of-process observation.** Input is injected at the OS level by an
   external driver process, and every observation is read from outside the
   product process. No assertion inside the product counts.
2. **A blocked result is a first-class outcome** with its own exit code, its own
   status string, and an exact prerequisite the owner can act on.
3. **`xtask` never links `legion-desktop`.** The packaged product is reached
   only as a subprocess, as `windowed-gui-e2e` already requires.

### Why a separate command and not a flag on `windowed-gui-e2e`

`windowed-gui-e2e` answers "can this build open a window and complete one
journey". Its subject is a *development* package it builds itself, its result is
one boolean journey, and it has one failure mode. This harness answers "does the
**installed** product's native input path conform, per class", its subject is an
owner-installed package it must not build, its result is six per-class
observations, and it has four distinct outcomes. Folding them together would
force `windowed-gui-e2e`'s single failure axis onto a four-outcome contract, and
the first thing to be lost would be the blocked/failed distinction — the one
thing this command exists to protect. The two commands also have different
promotion futures: `windowed-gui-e2e` is already on a 3-OS clock, and this one
has never produced a result at all.

### What is observed, and by which external oracle

The harness drives the packaged product and reads each result through an oracle
that lives outside the product process. Windows is the only host this decision
enables; the oracles below are the Windows oracles, and the macOS and Linux
equivalents are named for completeness only, since those rows stay blocked.

| Input class | Injected by | External oracle |
| --- | --- | --- |
| Keyboard | OS key injection (`SendInput` on Windows; `CGEventPost` on macOS; `XTEST`/`libei` on Linux) | The UI Automation `TextPattern` of the product's editor element, read from the driver process, for caret line/column and selection after each key; plus the window's UIA `Name`/dirty affordance. This is the oracle that would re-observe Home and End. |
| Pointer | OS pointer injection at screen coordinates derived from the UIA bounding rectangle of the target element | UIA focus and selection state read after the click, plus the caret position reported by `TextPattern`. Coordinates are never taken from the product's own geometry API. |
| Text | OS key injection of a marker string including non-ASCII and multi-code-point graphemes | The file's bytes on disk after an authoritative save, read by the driver with an independent SHA-256, compared against the expected UTF-8 encoding; plus the UIA text of the editor element. |
| Clipboard | The driver sets the **system** clipboard (`OpenClipboard`/`SetClipboardData` with `CF_UNICODETEXT`) before the paste gesture, and reads it back after a copy gesture | The system clipboard itself, owned by the OS and read by the driver, is the oracle. A copy is confirmed by the driver reading the OS clipboard; a paste is confirmed by on-disk bytes after save. The product's internal clipboard abstraction is never consulted. This is deliberately the hard case: an in-process clipboard fake would pass while the real one is broken. |
| IME/CJK | A real installed IME (Microsoft IME for Japanese on Windows) driven through OS key injection of the composition sequence, not a synthesized commit event | Two oracles in sequence: the UIA `TextPattern` of the editor during composition, to confirm the composition region exists and is not committed early; and the on-disk bytes after save, to confirm the committed code points. Also deliberately the hard case: a synthesized `egui` IME commit proves nothing about the platform TSF path. |
| Command | OS chord injection (for example `Ctrl+S`) **and**, as the contrast route, the command palette driven by key injection | On-disk file bytes plus SHA-256 before and after, read by the driver from a host shell, and the product's dirty indicator read through UIA. A chord that changes the UIA state but not the bytes, or the reverse, is a conformance failure. This contrast pair is exactly the one recorded in `DEF-2026-09-05-02`. |

Windows accessibility already has a walking probe on this host,
[`scripts/a11y-uia-walk.ps1`](../../scripts/a11y-uia-walk.ps1); the driver's UIA
oracle is the same class of instrument and is expected to reuse its approach.

A run may report `passed` only when all six classes are observed to conform
**and** the driver recorded that a native window existed. Exit 0 is the record
of those positive facts, never the absence of a failure.

### Exit-code contract

| Code | Status | Meaning |
| --- | --- | --- |
| `0` | `passed` | Every one of the six classes observed to conform, from outside the product, on a real window. |
| `1` | `conformance-failed` | The harness reached the product and the product deviated. **The product is wrong.** |
| `2` | `operational-error` | The harness could not operate: output directory not creatable, report not writable, or a driver that ran but produced no readable result. |
| `3` | `blocked` | This host cannot answer the question. |

These four are distinct and must stay distinct. Collapsing `1` and `3` into a
single nonzero would make a CI job unable to tell "the product is wrong" from
"this machine cannot answer the question", which is how an unavailable host
becomes a silent pass. A test asserts the two concrete integers and fails if
they are ever made equal.

Blocked is nonzero, always. It is never `passed`, never `skipped`, and never
`0`. The report always carries a `prerequisite` string naming what the owner
must supply, in the shape `plans/completion/decisions.md` already uses. The
report is written even when the run is blocked; a blocked run that leaves no
artifact is unauditable.

### What "the driver is available" means, and how it is probed

Availability is **probed**, never inferred. `cfg!(windows)` is not evidence that
a driver exists, and an environment variable that happens to be set on a
developer's machine is not evidence that a desktop session exists. A Windows CI
runner with no interactive session must report blocked, and under this decision
it does.

- **Windows.** The driver is a workspace crate,
  [`crates/legion-input-driver`](../../crates/legion-input-driver/src/main.rs) —
  a leaf binary with no internal dependencies, which nothing else in the
  workspace may depend on. Discovery requires a built driver to exist as a file
  on disk, probing these paths in order and taking the first that exists:
  `target/release/legion-input-driver.exe`,
  `target/debug/legion-input-driver.exe`, then
  `tools/native-input-driver/legion-input-driver.exe`. The third is retained so
  a binary an owner staged by hand still works; a freshly built driver outranks
  a stale staged one. Discovery reads the **filesystem** and nothing else:
  `xtask` never builds the driver, because a harness that builds its own
  instrument mid-run cannot report a clean blocked result, and the driver crate
  being a workspace member is not evidence that a driver binary exists. If none
  of the candidates exists, the run is blocked and **no child process is
  started** — a harness that spawns a windowed binary on a headless machine
  hangs CI. If one does, the harness next
  requires the packaged product executable to be present in the package
  directory; an absent package is blocked, not failed, because the product is
  not implicated. Only then does the harness ask the driver itself, as a
  subprocess, whether it is attached to an interactive input desktop; a driver
  that cannot attach reports it, and the run is blocked with the
  interactive-session prerequisite.
- **macOS.** Would additionally require Accessibility and Input Monitoring
  consent for the driver binary. Not enabled by this decision.
- **Linux.** Would additionally require a real display session and an injection
  path (`XTEST` under X11, or `libei`/portal consent under Wayland). Not enabled
  by this decision.

### macOS and Linux stay blocked

Native GUI automation is resumed on the Windows host only. Every macOS and Linux
native-input row stays blocked on `BLK-2026-09-08-02` — *"A macOS host and a
Linux host with the packaged native product installed and a real display
session, able to run the windowed GUI e2e suite."* On those hosts this command
reports blocked with that prerequisite and exits `3`.

**A Windows result never substitutes for a macOS or Linux row.** `COMP-PLAT-002`
names four configurations; a green Windows run assesses the Windows one and
leaves the other three exactly where they are. No packaged, cross-OS or release
readiness may be claimed from Windows evidence.

### Relationship to the open defects

`DEF-2026-09-05-01` (Home/End did not move the reported caret) is
`status: fixed-awaiting-verification`. `DEF-2026-09-05-02` (an injected
`Control_L+s` chord left the buffer dirty and the file unchanged, while the
command palette route saved) is `status: open`, with an explicitly unconfirmed
hypothesis. Both are `severity: P1` with
`invalidates_required_outcome: true`.

The Home/End source fix landed in
[`crates/legion-desktop/src/workflow.rs`](../../crates/legion-desktop/src/workflow.rs),
which now maps `Home` and `End` to line boundaries and, with the command
modifier, to document boundaries. **No native run has re-observed the key since
that change.** This harness is the instrument that would eventually re-observe
it — the keyboard-class oracle above is exactly the missing observation — and
until this command runs for real against a packaged product on a host with a
driver, both defects stay exactly where the register has them.

This ADR does not verify either defect, does not close either defect, and does
not move `COMP-PLAT-002`. Publishing a decision and registering a command shape
produces no acceptance evidence. `COMP-PLAT-002` remains
`implementation: partial`, `acceptance: unassessed`.

### Not a gate

Like `windowed-gui-e2e`, this command is not a standing gate and not
merge-blocking. No workflow under `.github/workflows/` references it, and
[`xtask/tests/native_product_acceptance.rs`](../../xtask/tests/native_product_acceptance.rs)
asserts that none does. Wiring an unproven harness into a merge gate would turn
an instrument that has never produced a result into a blocker; promoting it has
to be a deliberate act that edits that test.

## Consequences

The blocked path is specified and enforced before any real run exists, which is
the only time it gets examined honestly: once green runs start arriving, nobody
re-reads the exit-code semantics. The cost is that the driver is a real piece of
work with its own OS-specific injection and oracle code, carried in this
repository rather than sourced from outside it — and that until it has been
built **and** a packaged product has been staged, the harness still reports
blocked on every host, including this one. Building the driver removes one of
those two prerequisites and not the other.

Keeping the driver in the workspace has a second cost worth naming: the
workspace now contains a binary whose whole purpose is to inject OS input into
another process. It is a leaf with no internal dependencies, nothing may depend
on it, it is never linked by the product, and
[`plans/dependency-policy.md`](../dependency-policy.md) admits its `windows`
features for OS input injection and UI Automation observation only. It gains no
product runtime edge, no PTY and no job-object ownership.

Driver discovery and child-process launching are injected boundaries, so the
command's outcome logic is testable headlessly, with no display session, no
driver and no packaged binary. That is also what lets a test prove that an
unavailable driver starts no child process.

## Rejected alternatives

- **A `--input-conformance` flag on `windowed-gui-e2e`.** Rejected above: it
  would inherit a single failure axis and lose the blocked/failed distinction.
- **Extending the in-process `egui` conformance tests.** They are component
  evidence by construction; the sender and the receiver are the same process, so
  no amount of extension makes them observe the platform input path.
- **Linking `legion-desktop` into `xtask` to inspect product state directly.**
  Forbidden by the existing `xtask` constraint, and it would reintroduce exactly
  the in-process oracle this decision rejects.
- **Treating an unavailable driver as a skip.** A skip exits 0 and is
  indistinguishable from a pass in any aggregate. This is the failure this
  decision exists to prevent.
- **Inferring availability from `cfg!(windows)` or an environment variable.**
  Both would report available on a headless Windows CI runner and produce either
  a hang or a false result.
- **One nonzero code for every failure.** Rejected: it destroys the only signal
  that separates a product defect from an unavailable host.
- **Keeping the driver an owner-installed external binary.** Rejected by the
  amendment below. It made `BLK-2026-09-08-03` an owner prerequisite that no
  owner could discharge without writing the same code, and left the harness
  permanently inert for a reason inside this repository's control.
- **Letting `xtask` build the driver when it is missing.** Rejected: a harness
  that builds its own instrument mid-run has no clean blocked result to report,
  and a build failure would arrive dressed as a product outcome.
- **Adding a product-side hook, feature flag, environment variable or IPC
  channel so the driver can see inside the product.** Rejected — it is the
  in-process oracle again under another name. The driver observes only through
  UI Automation, the system clipboard, and bytes on disk.

## Acceptance gates for implementation

Ordered increments, each of which must preserve the blocked-path tests:

1. **This packet.** The decision plus the command shape: registered command,
   four distinct exit codes, injected discovery and launch boundaries, a written
   report on the blocked path, and the eight tests. Produces no acceptance
   evidence.
2. The Windows driver, built in this repository: OS injection plus the UIA,
   clipboard and on-disk oracles, and an honest interactive-session handshake.
   Delivered by the amendment below. Producing no acceptance evidence either:
   an instrument that exists has still observed nothing.
3. A first real run against an owner-installed package, whose result — whatever
   it is — is recorded as a Windows-only observation.
4. Only after (3), a defect-verification pass that could move
   `DEF-2026-09-05-01` and `DEF-2026-09-05-02`, and only through the register's
   normal route.

No increment may claim a macOS or Linux result, and no increment may promote
this command into a PR gate without the owner deciding to.

## Amendment 2026-09-08 — the driver is built in this repository

Owner ruling: the native input driver is **not** an external prerequisite. It is
[`crates/legion-input-driver`](../../crates/legion-input-driver/src/main.rs), a
workspace member built with `cargo build -p legion-input-driver --release`.

**What changed.**

- The external-binary assumption is gone. "What *the driver is available* means"
  above now describes an in-repo workspace crate and an ordered candidate list
  (`target/release`, `target/debug`, then the retained
  `tools/native-input-driver/` path), probed on the filesystem, first existing
  file wins.
- `PREREQUISITE_DRIVER_MISSING` in
  [`xtask/src/native_product_acceptance.rs`](../../xtask/src/native_product_acceptance.rs)
  names that build command instead of an installation, and the retained staged
  path is documented after it as the fallback discovery still probes.
- `BLK-2026-09-08-03` — *"the external native input driver installed at
  `tools/native-input-driver/legion-input-driver.exe`"* — is **retired by
  construction**: the thing it asked an owner to supply is now built from
  source in this repository. `BLK-2026-09-08-04` (a packaged native product
  staged into the package directory) is **not** retired and still blocks every
  run.
- The Consequences section no longer says the harness is inert until the owner
  supplies a driver, and names the second cost of carrying an input-injection
  binary in the workspace.

**What did not change, and is not weakened by the driver existing.**

- **Out-of-process observation.** The driver reads the product only through UI
  Automation, the system clipboard and bytes on disk. No product-side test hook,
  feature flag, environment variable or IPC channel exists or may be added, and
  no assertion inside the product counts.
- **The four-outcome exit contract.** `0` passed, `1` conformance-failed, `2`
  operational-error, `3` blocked, pairwise distinct.
- **Blocked is nonzero, always** — never `passed`, never `skipped`, never `0` —
  and always carries an exact prerequisite. The report is written even when the
  run is blocked.
- **A class the driver did not observe is never reported as conforming.** The
  driver emits no line at all for such a class.
- **`xtask` never links `legion-desktop`,** and never builds the driver.
- **Not a gate.** No workflow under `.github/workflows/` references this
  command, and the test asserting that stands unchanged.
- **macOS and Linux stay blocked on `BLK-2026-09-08-02`,** and *a Windows result
  never substitutes for a macOS or Linux row.* The driver's non-Windows build
  writes a blocked report naming that blocker and exits nonzero.

**This amendment produces no acceptance evidence.** No run happened, no window
opened, no input class was observed. `COMP-PLAT-002` remains
`implementation: partial`, `acceptance: unassessed`, and `DEF-2026-09-05-01` and
`DEF-2026-09-05-02` stay exactly where the register has them.

## Amendment 2026-09-08 — a blocked driver outcome is a blocked run

Before this amendment,
[`xtask/src/native_product_acceptance.rs`](../../xtask/src/native_product_acceptance.rs)
ended its run with a two-arm `if/else`: a complete six-of-six pass, or
`conformance-failed` with exit `1`. That `else` swallowed every other
post-driver outcome — including the driver's own `EXIT_BLOCKED` (`3`).

That is not a stylistic problem. On this host `observe_ime_cjk` in
[`crates/legion-input-driver/src/main.rs`](../../crates/legion-input-driver/src/main.rs)
reads the product window's keyboard layout, finds it is not a CJK primary
language id, and correctly returns `ClassOutcome::Blocked` with an exact
prerequisite. The first real windowed run would therefore have produced a driver
that behaved perfectly, an `ime-cjk` class that honestly reported *blocked*, and
a harness artifact saying **the product failed native input conformance** — an
artifact eligible to enter `plans/completion/defects.json` against a product
that did nothing wrong.

**Decision: the driver's process exit code is dispatched on exhaustively.**

| What the driver did | Harness `status` | Harness exit |
| --- | --- | --- |
| exit `0`, and its own result records a created window and all six classes `conforms` | `passed` | `0` |
| exit `1` (`EXIT_CONFORMANCE_FAILED`) — it ran to completion and observed a genuine deviation | `conformance-failed` | `1` |
| exit `3` (`EXIT_BLOCKED`) — it could not establish an oracle, or a class was blocked | `blocked` | `3`, carrying the driver's own exact prerequisite verbatim |
| exit `2` (`EXIT_OPERATIONAL_ERROR`) | `operational-error` | `2` |
| any other exit code | `operational-error` | `2` |
| exit `0` contradicted by its own result (no window, or fewer than six `conforms`) | `operational-error` | `2` |
| a stated `exit_code` field that disagrees with the process exit code | `operational-error` | `2` |

**`conformance-failed` is reachable from exactly one of those rows:** a driver
that ran to completion and reported a deviation itself. It is never reached by
inference from a missing marker, a short class list, an unreadable field or a
catch-all arm.

The last two rows are not technicalities. `conformance_exit_code` in
[`crates/legion-input-driver/src/report.rs`](../../crates/legion-input-driver/src/report.rs)
returns `0` if and only if the report holds a created window and six conforming
classes, so a driver that exits `0` while its own result says otherwise is a
broken instrument, not a broken product. Reporting that as `conformance-failed`
would be the same bug in a new place.

**Decision: a blocked class is never a defect against the product.** `blocked`
is one of the three class outcomes, and `ime-cjk` on a host with no CJK input
layout is the worked example: the oracle cannot be established, saying so is the
driver behaving correctly, and the run is `blocked` with that class's own
prerequisite. The report now also names which classes blocked and which
deviated, in `input_classes_blocked` and `input_classes_deviating` beside the
existing `required_input_classes` and `input_classes_observed`, so the
distinction is visible in the artifact and not only in the code.

**Decision: the driver composes a blocked run's prerequisite from its blocked
classes.** A run with five conforming classes and one blocked class is blocked
with a non-empty `observations` list and no top-level prerequisite, which
previously wrote `prerequisite = ""` — a blocked result nobody can act on. The
driver now composes the top-level prerequisite from the blocked classes' own
`detail` strings, naming each class, and a blocked report never renders an empty
prerequisite. The harness propagates that string **verbatim**: it is not
reworded, not prefixed, and not merged with one of the harness's own
prerequisites. If it is ever handed a blocked result that states no reason, the
harness says exactly that (`PREREQUISITE_DRIVER_STATED_NO_REASON`) rather than
inventing a host prerequisite.

**What did not change, and is not weakened by any of this.**

- **Out-of-process observation.** The driver reads the product only through UI
  Automation, the system clipboard and bytes on disk. No product-side test hook,
  feature flag, environment variable or IPC channel exists or may be added, and
  no assertion inside the product counts. This amendment touches no product
  crate.
- **The four-outcome exit contract.** `0` passed, `1` conformance-failed, `2`
  operational-error, `3` blocked, pairwise distinct.
- **Blocked is nonzero, always** — never `passed`, never `skipped`, never `0` —
  and always carries an exact prerequisite. The report is written even when the
  run is blocked.
- **The deviates-outranks-blocked ranking inside the driver.** A class observed
  to deviate still yields `EXIT_CONFORMANCE_FAILED` even when another class is
  blocked, so a real product finding is never hidden behind an unavailable
  oracle. `ClassOutcome` still has exactly three variants, a blocked class still
  emits its own `input_class_<class> = "blocked"` line, and no path can now
  produce `conforms` that could not before.
- **A class the driver did not observe is never reported as conforming,** and
  the six class markers plus `window_created` remain literal wire-protocol
  substrings.
- **`xtask` never links `legion-desktop`,** and never builds the driver.
- **Not a gate.** No workflow under `.github/workflows/` references this
  command, and
  [`xtask/tests/native_product_acceptance.rs`](../../xtask/tests/native_product_acceptance.rs)
  asserts that, unchanged.
- **macOS and Linux stay blocked on `BLK-2026-09-08-02`,** and *a Windows result
  never substitutes for a macOS or Linux row.* The driver's non-Windows build
  still writes a blocked report naming that blocker and exits nonzero, and
  `BLK-2026-09-08-04` (a packaged native product staged into the package
  directory) is not retired.

**This amendment produces no acceptance evidence.** No run happened, no window
opened, no input class was observed. The correct claim is narrow: the harness
can now tell a blocked host from a failing product, and it has told neither.
`COMP-PLAT-002` remains `implementation: partial`, `acceptance: unassessed`, no
defect is filed, and `DEF-2026-09-05-01` and `DEF-2026-09-05-02` stay exactly
where the register has them.
