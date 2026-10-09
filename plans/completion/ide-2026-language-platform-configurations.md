# IDE 2026 language and platform configuration candidates

Date: 2026-10-09. Ticket 88, M4 / S0-02 / S2-01 / A16. Inventory baseline:
177caa550aadcd5513fabd8b128c624dfa074cf5. Status: planning attachment for review;
owner/host ratification incomplete, no language/platform acceptance promoted.

## Authority and coverage

The [approved specification](../../.scratch/ide-2026-planning/spec.md),
[S2-01 contract](../../docs/superpowers/plans/2026-09-04-manual-language-completion.md),
[matrix](matrix.json), [reconciliation](ide-2026-reconciliation.md),
[pilot02](ide-2026-pilot-configuration.md) and
[migration03](ide-2026-migration-contract.md) govern this attachment. The matrix
remains the canonical configuration register. Its 42 required cells and existing
IDs are preserved: 32 language cells across four OS/architecture groups, four
product-journey cells, three Manual/offline cells and three stable/signed cells.
Provisional matrix markers remain unchanged; this document does not ratify them.

Labels below mean **ratified** by the existing pilot owner decision, **observed**
by this bounded read-only inventory, **proposed** as an exact candidate requiring
owner approval and artifact verification, or **missing** because no usable host,
tool identity or fixture freeze is supplied. A public release is not an installed
tool or a successful product integration. All runtime and native qualification
remains downstream. No imports, installations, credentials, network fallback,
tool process qualification or OS input was performed.

## Host and accessibility inputs

| Matrix prefix / host | OS and hardware input | AT and native-input configuration | Decision / owner |
| --- | --- | --- | --- |
| CFG-WIN11-X64 | Ratified Windows 11 Pro x64 10.0.26200/build 26200; ROG Strix G18, i9-14900HX, 32 GiB. OS reobserved unchanged. GPU drivers from supplied notes: NVIDIA 32.0.16.1714, Intel 32.0.101.6790; active adapter, DPI/power and OS update revision not frozen. | Observed Narrator.exe file/product 10.0.26100.8972; propose this exact installed Narrator with Windows UIA. No Narrator session exercised. Supplied language inventory is en-US/0409:00000409 only; CJK composition remains missing. Native driver source 177caa5 is a reviewed partial instrument, not qualified input. | Initial Windows/Rust owner decision persists; no reapproval requested. New language/AT pins require S0-02 owner. Tickets 04/07/23 own input/IME/AT gates; latest ticket04 Explorer README UIA lookup found zero before input. |
| CFG-MAC15-X64 | Proposed macOS Sequoia 15.7.9 x64, within existing macOS 15 scope; hardware, build number, image/installer digest and interactive Intel host missing. This is an exact candidate, not an observed installation or latest-release assertion. | Propose bundled VoiceOver on that exact OS plus native AX. VoiceOver binary version, permission state, human observer, keyboard/layout/IME and native external driver missing. | S0-02 owner must supply host/patch/build and explicit approval; tickets 89/91/92 own native input/AT/performance. |
| CFG-MAC15-ARM64 | Proposed macOS Sequoia 15.7.9 arm64; hardware/build/image missing. Native aarch64 processes required; Rosetta is not arm64 evidence. | Same proposed OS-bundled VoiceOver/AX, independently frozen on arm64. No host session available. | Same owner and downstream tickets; x64 success cannot stand in for arm64. |
| CFG-LINUX-UBUNTU-X64 | Proposed Ubuntu Desktop 24.04.5.1 LTS amd64, exact image filename ubuntu-24.04.5.1-desktop-amd64.iso. ISO digest, installed kernel/build/hardware and interactive display session missing. | Propose Orca 46.1 with AT-SPI; exact distro package revision, AT-SPI version and chosen Wayland/X11 session missing. Do not silently switch compositor when a path fails. | S0-02 owner must ratify host/image/session and confirm compatible packaged AT stack; tickets 93/94/95 own input/AT/performance. |

Official candidate sources: [Apple macOS 15 updates](https://support.apple.com/en-us/120283),
[Ubuntu desktop image index](https://releases.ubuntu.com/24.04.5/),
[GNOME Orca 46 sources](https://download.gnome.org/sources/orca/46/).
AT interfaces: [Narrator](https://support.microsoft.com/en-us/accessibility/windows/narrator/complete-guide-to-narrator),
[VoiceOver](https://support.apple.com/guide/voiceover/welcome/mac),
[Orca](https://help.gnome.org/orca/). These sources identify candidates/interfaces;
they do not establish Legion accessibility or host availability.

## Named language tools, runners and debuggers

The same proposed language versions below apply to all four host groups unless
a platform-specific target is named. Cross-platform availability and product
resolution must be checked separately; there is no automatic alternate adapter.
Use absolute approved artifact paths and cached dependencies, not opportunistic
PATH selection. Freeze each archive/executable digest and all transitive package
locks before qualification; a version pin alone is incomplete provenance.

| Stack | Runtime/server/formatter | Build and test runner | Debugger | Evidence and missing input |
| --- | --- | --- | --- | --- |
| Rust | Windows ratified rustc/Cargo/rust-analyzer 1.98.1; rustc commit 48a229ceaefd4985c50990b14116b6d856af0985, Cargo 797e8a9bc. rustfmt/clippy/rust-src installed components recorded by pilot02; exact component build identities still required. Propose toolchain 1.98.1 and matching components on other hosts. | Cargo 1.98.1 build/test, locked/offline. Windows target x86_64-pc-windows-msvc; Mac x86_64-apple-darwin / aarch64-apple-darwin; Linux x86_64-unknown-linux-gnu. Exact MSVC/linker/SDK, Xcode/CLT and Linux compiler/linker package versions missing. | CodeLLDB 1.12.3: approved Windows x64 artifact; propose corresponding native Mac x64/arm64 and Linux x64 artifacts. MSVC/PDB launch/variables/enum behavior unqualified; no GNU substitution. | Windows versions reobserved; other hosts unobserved. Rust test harness is Cargo's actual runner, not a separate fabricated framework version. Tickets 15–18 retain real server/debuggee and denial/recovery qualification. |
| TypeScript browser | Observed Node 24.19.0/npm 11.17.0; propose these exact runtimes on each host, TypeScript 6.0.3, typescript-language-server 6.0.0, Prettier 3.6.2. | tsc 6.0.3 build/source maps; Playwright Test 1.55.1. Proposed Chromium 140.0.7339.186 revision 1193, headed for browser/debug journeys. | Microsoft js-debug 1.104.0 standalone DAP candidate, using the pinned Chromium process. Launch/attach/source-map/path behavior missing. | Old retained server evidence used TLS 6.0.0/TS 6.0.3 under Node 24.19.0; archives not reverified here, and no native workflow inferred. Tickets 96–99 own TS browser outcomes. |
| JavaScript browser | Node 24.19.0/npm 11.17.0; TLS 6.0.0 with TypeScript 6.0.3 supporting JS/checkJs; Prettier 3.6.2. | Separate .js project, syntax/semantic checks with checkJs; Playwright Test 1.55.1 and Chromium 140.0.7339.186/revision 1193. | js-debug 1.104.0 with the same explicit browser pin; JS launch/attach/breakpoints require separate evidence. | Tickets 104–107; TS tests/results cannot be relabelled JavaScript qualification. |
| TypeScript Node | Node 24.19.0/npm 11.17.0; TS 6.0.3/TLS 6.0.0/Prettier 3.6.2. | tsc with explicit source maps; Node 24.19.0 built-in node:test, e.g. node --test against compiled tests. Exact commands and discovery expectations must be frozen with fixture. | js-debug 1.104.0, explicit Node executable and DAP launch/attach arguments. | Tickets 100–103, separate TS subproject/evidence within combined TSJS-NODE matrix cell. |
| JavaScript Node | Node 24.19.0/npm 11.17.0; TLS 6.0.0/TS 6.0.3 checkJs/Prettier 3.6.2. | Separate .js subproject; Node 24.19.0 built-in node:test, passing/failing discovery and exit-code oracles. | js-debug 1.104.0, separate JS debuggee and expected values. | Tickets 108–111. Combined matrix ID does not collapse independent TS and JS coverage. |
| Python application | Propose CPython 3.13.7, isolated venv; Pyright 1.1.400 under Node 24.19.0; Black 25.1.0. PATH python is only a Windows Store alias and --version returned not-found guidance. | CPython 3.13.7 compileall/application task; stdlib unittest discovery bound to that interpreter (not an independently versioned library). | debugpy 1.8.16 in the same venv, python -m debugpy; real launch/attach, variables/evaluation and termination missing. | Pyright historical evidence is server substrate, not runnable Python/native acceptance. Interpreter/cache/venv identity and absolute path missing. Tickets 112–115. |
| Python package | Same CPython/Pyright/Black/debugpy pins, separate package venv. | setuptools 80.9.0 backend with build 1.3.0 frontend; freeze wheel/build dependencies including wheel package before use. stdlib unittest 3.13.7; test built installed package in a second isolated environment to avoid checkout-import false positives. | debugpy 1.8.16 against installed package entry point and explicit interpreter. | Backend/frontend are proposals; no build/wheel/lock digest exists yet. Tickets 116–119. |

These are bounded candidate choices, not a promise of compatibility or an
instruction to install old releases. Owner review must consider maintained
patches/security and any resulting re-freeze; a changed version needs a new
explicit contract, never a floating latest range or automatic upgrade.

Official pin sources (checked read-only 2026-10-09):

- [Node 24.19.0 artifacts](https://nodejs.org/download/release/v24.19.0/) and
  [node:test for that version](https://nodejs.org/download/release/v24.19.0/docs/api/test.html).
- [TypeScript 6.0.3](https://github.com/microsoft/TypeScript/releases/tag/v6.0.3),
  [TLS 6.0.0](https://github.com/typescript-language-server/typescript-language-server/releases/tag/v6.0.0)
  (requires Node >=22.22.2), [Prettier 3.6.2](https://github.com/prettier/prettier/releases/tag/3.6.2).
- [Playwright 1.55.1](https://github.com/microsoft/playwright/releases/tag/v1.55.1),
  [tagged Chromium version/revision](https://raw.githubusercontent.com/microsoft/playwright/v1.55.1/packages/playwright-core/browsers.json),
  [js-debug 1.104.0](https://github.com/microsoft/vscode-js-debug/releases/tag/v1.104.0).
- [CPython 3.13.7](https://www.python.org/downloads/release/python-3137/),
  [Pyright 1.1.400](https://github.com/microsoft/pyright/releases/tag/1.1.400),
  [Black 25.1.0](https://github.com/psf/black/releases/tag/25.1.0),
  [debugpy 1.8.16](https://github.com/microsoft/debugpy/releases/tag/v1.8.16),
  [setuptools 80.9.0](https://github.com/pypa/setuptools/releases/tag/v80.9.0),
  [build 1.3.0](https://github.com/pypa/build/releases/tag/1.3.0).
- [CodeLLDB 1.12.3 artifacts](https://github.com/vadimcn/codelldb/releases/tag/v1.12.3),
  [Rust target definitions](https://doc.rust-lang.org/rustc/platform-support.html).
  Other-platform Rust 1.98.1 availability/digests still require native provisioning
  inventory; this document does not derive them from current Rust platform docs.

## Exact matrix-to-project mapping

Each row below expands into the four full existing IDs listed. Hosts inherit
their OS/AT table entries; tools inherit the language stack. Fixtures are proposed
qualification inputs until owner-approved and frozen. Never run against original
source/WIP; use disposable clones. No artificial fixture was created in this slice.

| Project category and existing IDs | Named project identity and remaining freeze | Workflow owners |
| --- | --- | --- |
| Rust binary: CFG-WIN11-X64-RUST-BIN; CFG-MAC15-X64-RUST-BIN; CFG-MAC15-ARM64-RUST-BIN; CFG-LINUX-UBUNTU-X64-RUST-BIN | Windows pilot uses Legion binary at c2a65786862e759e67ebac28c32c1cbc618047f0, Cargo.lock SHA256 0709c888cba93763226663f9314dee5198bf2dc1387d8b8c334422df3f08d6c7; propose same source for other hosts. Named binary/debuggee/source position/expected values still to freeze. Optional existing bench-rust-cli wordtally 0.1.0 at baseline177caa5 tree573e1bebd3fd52bc7a6ec94d6059821a5f50a3ac is inventory only, not replacement of approved Legion pilot. | 15–18; cross-host owners must be assigned. |
| Rust library: CFG-WIN11-X64-RUST-LIB; CFG-MAC15-X64-RUST-LIB; CFG-MAC15-ARM64-RUST-LIB; CFG-LINUX-UBUNTU-X64-RUST-LIB | Same Legion freeze, select library/tests and executable test debuggee explicitly. Existing bench-rust-lib miniconf 0.1.0 tree dcd1358baf15c05ffb1b52251f2266fe430a6df2 is optional inventory only. Fixture lockfile and complete manifest hashes missing for optional candidates. | 15–18. |
| Rust workspace: CFG-WIN11-X64-RUST-WORKSPACE; CFG-MAC15-X64-RUST-WORKSPACE; CFG-MAC15-ARM64-RUST-WORKSPACE; CFG-LINUX-UBUNTU-X64-RUST-WORKSPACE | Ratified Legion-on-Legion multi-crate reference above; preserve all pilot02 scenario cases. New product candidate revision/package digest remains distinct from reference project revision. | 15–18; 89–95 platform prerequisites. |
| TS browser: CFG-WIN11-X64-TS-BROWSER; CFG-MAC15-X64-TS-BROWSER; CFG-MAC15-ARM64-TS-BROWSER; CFG-LINUX-UBUNTU-X64-TS-BROWSER | Proposed project legion-qualification-ts-browser, src/main.ts plus cross-file model.ts, emitted JS/source maps, package.json/tsconfig and browser tests. No repository/commit/lockfile/assets/digests supplied. Existing mockups is a separate design app, not a frozen native IDE language fixture. | 96–99. |
| JS browser: CFG-WIN11-X64-JS-BROWSER; CFG-MAC15-X64-JS-BROWSER; CFG-MAC15-ARM64-JS-BROWSER; CFG-LINUX-UBUNTU-X64-JS-BROWSER | Proposed distinct legion-qualification-js-browser, src/main.js/model.js with JSDoc/checkJs and browser tests; repository/commit/lockfile/digests missing. It must include independent JS semantic edits and debug expectations. | 104–107. |
| TS/JS Node: CFG-WIN11-X64-TSJS-NODE; CFG-MAC15-X64-TSJS-NODE; CFG-MAC15-ARM64-TSJS-NODE; CFG-LINUX-UBUNTU-X64-TSJS-NODE | Proposed legion-qualification-node with two separately locked ts-node and js-node subprojects, distinct entrypoints, passing/failing tests and multi-file edits. Both source/lock identities and explicit run/debug commands missing. | 100–103 and 108–111, separate evidence. |
| Python app: CFG-WIN11-X64-PY-APP; CFG-MAC15-X64-PY-APP; CFG-MAC15-ARM64-PY-APP; CFG-LINUX-UBUNTU-X64-PY-APP | Proposed legion-qualification-python-app, app/__main__.py/model.py/tests, pyright config and interpreter/venv manifest. Source revision, dependency locks/digests and discovery oracle missing. | 112–115. |
| Python package: CFG-WIN11-X64-PY-PKG; CFG-MAC15-X64-PY-PKG; CFG-MAC15-ARM64-PY-PKG; CFG-LINUX-UBUNTU-X64-PY-PKG | Proposed legion-qualification-python-package with pyproject.toml, src-layout package, console entrypoint/tests, sdist/wheel expected contents and isolated installed-package test. Revision/lockfile/build backend dependencies/artifact hashes missing. | 116–119. |

Tree IDs above were read from Git at 177caa5; they identify committed fixture
content, not an executed test or SHA256 manifest. GP-1 Rust tree
46b3771e800642741c334c6885002628269a3d79 is explicitly a smoke template in its
README; it cannot replace representative native project qualification. Historical
[live-server evidence](../evidence/full-product-resume-2026-09-09/live-language-server-evidence.md)
identifies retained TLS/Pyright archive checks and intermittent loaded-host results;
neither archive presence nor those component runs establish A16 native journeys.

## Broader cells and handoff gates

The ten non-language required cells remain intact, with distinct evidence:

| Existing IDs | Required prerequisite / owner |
| --- | --- |
| CFG-WIN11-X64-PRODUCT-JOURNEY; CFG-MAC15-X64-PRODUCT-JOURNEY; CFG-MAC15-ARM64-PRODUCT-JOURNEY; CFG-LINUX-UBUNTU-X64-PRODUCT-JOURNEY | Each host's packaged native executable, UIA/AX/AT-SPI and input/AT observer; tickets 04/23/89/91/93/94 and platform performance owners. Windows source07d2187 offline MSI0.0.2 is a staged unsigned candidate; latest driver attempt blocked at Explorer lookup before input. It is not signed stable evidence. Mac/Linux package identities missing. |
| CFG-WIN11-X64-MANUAL-OFFLINE; CFG-MAC15-ARM64-MANUAL-OFFLINE; CFG-LINUX-UBUNTU-X64-MANUAL-OFFLINE | Separately identified installed Manual/offline artifact and OS per-process-tree network capture for complete prescribed interval. Capture tool/version, artifact digest and attribution still missing; ticket25 plus platform/privacy owners. No inferred zero-egress pass from offline SKU. |
| CFG-WIN11-X64-STABLE-SIGNED; CFG-MAC15-ARM64-STABLE-SIGNED; CFG-LINUX-UBUNTU-X64-STABLE-SIGNED | Retain ticket235 artifact/signer/update contracts; actual signer/trust verifier/notarizer and signed artifact absent. Descriptors stay dry-run/no-production-signer. No signing secrets read or requested here. |

Containers 2.5.2 and Remote Containers 0.469.0 remain later required targets under
migration03. They are not initial pilot dependencies or compatibility evidence.
Local matrix selection does not waive SSH/container/remote-language/debug tickets,
formal-platform availability, source-linked web/test trace scope, recovery cases,
or independent observation periods.

Concrete decisions still required before canonical ratification:

1. S0-02 owner assigns actual macOS Intel/arm64 and Ubuntu host operators,
   hardware/image/build/session records and approves or replaces the exact OS/AT
   candidates. Windows initial pilot ratification remains valid; only new pins
   and unobserved host details need decisions.
2. S2-01 tool owner reviews the proposed server/formatter/runner/debugger package
   tuple, exact artifact hashes/paths/dependency caches and platform linker/SDK
   versions, plus source-map, MSVC/PDB and debugpy interpreter policy. No fallback
   server/debugger/runtime is approved by this inventory.
3. Project owners 96–119 supply real representative source repositories and
   immutable revisions/locks/manifests for all proposed TS/JS/Python projects;
   Rust owners freeze named target positions/tests/debuggees within Legion.
4. AT/input owners supply human/native observers, permission/input-method states,
   active CJK composition and actual accessibility-provider evidence on each host.
   Missing host/AT resources block dependent outcomes, not remove required cells.
5. After these decisions, a separately reviewed canonical matrix/approval-reference
   update must replace matching placeholders without changing IDs or required
   coverage; registry agreement/runtime tests belong to that implementation slice.
   No such matrix edit or acceptance-record update is made here.

## Planned verification and bounded inventory method

Planned before edits, once each: existing xtask docs-hygiene, existing
verify-completion-register --root . (structure only), and git diff --check. Use
D:/legion-ide-2026-tools/qualification-target for Cargo outputs, preserving source
and other workers' targets. No artificial runtime tests for these planning docs.
Results are recorded in issue88; independent review precedes any commit.

Actual checks, run once on this planning patch:

- `cargo run -p xtask --target-dir D:/legion-ide-2026-tools/qualification-target -- docs-hygiene`: exit 0, documentation hygiene passed.
- `cargo run -p xtask --target-dir D:/legion-ide-2026-tools/qualification-target -- verify-completion-register --root .`: exit 0, register structure passed; 419 acceptance entries remain unassessed. This assesses no evidence or acceptance.
- `git diff --check`: exit 0. Only this attachment and issue88 are changed; canonical matrix/decisions/requirements/scenarios/readiness records are untouched.

No successful validator is repeated to record these results. Ticket88 remains
needs-info; this bounded inventory is review-ready, not a completed ratification.

Independent documentation review PASS, relayed by coordinator on 2026-10-09,
applies to bounded inventory only: all 42 IDs/scope retained, observed/proposed/
missing inputs distinguished, no acceptance promoted. Provisional tool versions
are not security or compatibility endorsements; future provisioning requires a
current maintained-patch review. Actual Mac/Linux host availability has been
requested by the coordinator and remains pending. Partial docs commit authorized;
ticket88 remains needs-info. Integration tip177caa5 confirmed current before commit.

Read-only inventory used targeted matrix/spec/pilot/migration/authority reads,
Get-CimInstance Win32_OperatingSystem, Get-Command tool availability, rustc -Vv,
cargo/rust-analyzer/node/npm/python/git version commands, Narrator file metadata,
approved adapter/driver path existence and git ls-tree fixture identities. No
user settings, tokens, service data or tool configuration secrets were read.
Python command failure is recorded honestly as an alias, and absent PATH entries
do not prove that no isolated installation exists. No Mac/Linux host was surveyed.
