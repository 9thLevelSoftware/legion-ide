# IDE 2026 Windows/Rust pilot configuration

Date: 2026-10-08. Ticket: 02. Status: resolved; independent review PASS.
Scope: qualification input contract for A01/A16, M0, S0-02 and S2-01.

Basis: integration `c2a65786862e759e67ebac28c32c1cbc618047f0` and
[reconciliation](ide-2026-reconciliation.md), subsequently fast-forwarded to
`87580fa`. The [execution owner decisions](../../.scratch/ide-2026-planning/execution.md)
ratify the observed Windows/Rust host, Legion-on-Legion and native equivalents.
The [specification](../../.scratch/ide-2026-planning/spec.md) remains authoritative.
This document does not promote canonical acceptance or change matrix.json,
scenarios.json, requirements.json, decisions.md or the readiness ledger.

## Selected inputs

| Input | Frozen selection and evidence limit |
| --- | --- |
| Host | Microsoft Windows 11 Pro x64, version 10.0.26200, build 26200. |
| Hardware | ASUSTeK ROG Strix G18 G814JIR_G814JIR; Intel Core i9-14900HX, 24 cores / 32 logical processors; 32 GiB RAM. GPU, display scale and power profile have not been pinned; record them before performance scoring. |
| Rust | stable-x86_64-pc-windows-msvc; rustc 1.98.1 (48a229cea 2026-09-01), Cargo 1.98.1 (797e8a9bc 2026-08-05). Preserve MSVC; no silent GNU substitution or stable-channel upgrade. |
| Language server | rust-analyzer 1.98.1 (48a229ce 2026-09-01). Observed installed components: clippy, rustfmt, rust-src, wasm32 std. |
| Representative repository | Legion IDE itself, source baseline c2a65786862e759e67ebac28c32c1cbc618047f0; Cargo.lock SHA-256 0709c888cba93763226663f9314dee5198bf2dc1387d8b8c334422df3f08d6c7. Integration qualification candidate source 87580fa; record full resolved commit and lockfile digest for each run. Use disposable clones, preserving original and worker WIP. |
| Candidate build | Coordinator-owned checkout D:/legion-ide-2026-candidate at 87580fa, offline feature; package-native.ps1 Version 0.0.1, Format wix, OutDir D:/legion-ide-2026-tools/pilot-candidate-87580fa. Build/package exit 0; offline SKU, source 87580fae51293ca619a71535a184ef1a96a00b9c. MSI SHA-256 f73b50214c81bdb1498672268385d9439443bc420e4fb6099901fd4602d07976. Package verifier exit 1; native acceptance remains pending ticket04. |
| Debug adapter candidate | CodeLLDB 1.12.3 Windows x64; official codelldb-win32-x64.vsix, 50,943,475 bytes; archive SHA-256 a916e509308dac817732f63ca604a8b93ed29cd16f38a2fa9f0b64ed58e8f51a, verified during approved isolated setup. Adapter: D:/legion-ide-2026-tools/codelldb-1.12.3/extracted/extension/adapter/codelldb.exe. Digest identifies the archive, not a separately hashed executable. No VS Code extension installed or compatibility asserted. |
| Native driver | Existing legion-input-driver built at integration 53b766c, unchanged driver source; D:/legion-ide-2026-tools/native-input-target/release/legion-input-driver.exe. Probe report D:/legion-ide-2026-notes/driver-session.toml: exit 0, attached, interactive_session true, input desktop Default. No injected input or product workflow evidence. |
| Mode and network | Manual, zero AI egress, metadata-only retention. Pre-provision dependencies for locked/offline commands; unavailable cache blocks the affected command. No provider credentials required and no automatic network fallback. Native equivalents satisfy only demonstrated initial pilot workflows; extension/remote scope remains required. |

Observation sources, read only: D:/legion-ide-2026-notes/pilot-prerequisites.md
and debugger-candidate.md; candidate build evidence is recorded in
D:/legion-ide-2026-notes/pilot-candidate-87580fa.md. Earlier missing-driver/debugger observations are
superseded only for the selected paths. cl.exe/devenv absence on PATH does not
prove absence of installed MSVC tooling; successful driver build is narrower
build evidence. Physical keyboard, touchpad and mice reported OK, not conformance.

## Frozen pilot scenario set

Use the complete existing steps, external oracles, recovery cases and sensitive
artifact policy of each ID in [scenarios.json](scenarios.json). These groups freeze
the initial Windows set; none removes broader configuration or program scope.
Product journeys use CFG-WIN11-X64-PRODUCT-JOURNEY except
SC-MANUAL-OFFLINE-30M-WIN, which requires CFG-WIN11-X64-MANUAL-OFFLINE.
That scenario requires a separately labelled, installed Manual/offline artifact
with recorded hash, metadata and inventory, distinct from the signed stable
installer, plus OS per-process network capture started before launch and
attributed to the complete product process tree. Missing artifact or unavailable
capture/attribution blocks the scenario; offline build identity alone is not
acceptance. Preserve its full thirty-minute steps, external oracles, recovery
cases and metadata-only capture policy. Rust scenarios retain
CFG-WIN11-X64-RUST-BIN, CFG-WIN11-X64-RUST-LIB and
CFG-WIN11-X64-RUST-WORKSPACE. Legion's binary, library and multi-crate workspace
are the representative forms; downstream owners record exact source positions,
selected tests/debuggee and expected values before running, preserving all cases.
A missing atom in these anchors must be registered by its owning ticket, not
silently counted as coverage.

| Frozen existing IDs | Downstream work / observable result |
| --- | --- |
| SC-MANUAL-OPEN-TYPE-SAVE; SC-MANUAL-EXPLORER-FILE-OPS; SC-MANUAL-SAVE-FAILURE-RECOVERY | 04, 05, 12: native input, exact disk bytes, Save/Save All/Save As, denial and conflict preserve dirty text. |
| SC-MANUAL-EDIT-INPUT-RECOVERY; SC-MANUAL-EDIT-SEMANTICS; SC-MANUAL-VIM-MODAL | 06, 07: selection, Unicode, undo/redo, real composition and Vim intent/effect routing. |
| SC-MANUAL-WORKBENCH-RESTORE; SC-MANUAL-LAYOUT-MANUAL-MODE | 08–11, 21: tabs/splits/layout/canvas and crash recovery; retain editable-card and derived/person-edge distinctions from reconciliation. |
| SC-MANUAL-NAVIGATE-FILES-SYMBOLS; SC-MANUAL-REFACTOR-SEARCH-REVIEW | 13: navigation/search/replace, cancellation and reviewed exact changes. |
| SC-MANUAL-TERMINAL-TUI; SC-MANUAL-TERMINAL-SESSIONS | 14: real child process, control keys, resize, exit status and cancellation cleanup. |
| SC-LANG-LSP-LIFECYCLE-RUST; SC-LANG-REFACTOR-RUST | 15, 16: real server, language ranges, proposal cancellation/application and stale rejection. |
| SC-LANG-BUILD-TEST-RUST; SC-LANG-DEBUG-RUST; SC-LANG-TEST-DEBUG-POLICY-RUST | 17, 18: real discovery/build/test, launch/attach/breakpoint/step/variables/evaluation/termination and denied tools. |
| SC-MANUAL-GIT-HISTORY-RECOVERY; SC-MANUAL-SCM-STATUS-REMOTE | 19, 20: exact staging/commit, conflict and history; external remote effects require separately declared endpoints/authority, never inferred from local Git. |
| SC-PLATFORM-A11Y-WIN; SC-MANUAL-LARGE-FILE-STREAMING; SC-MANUAL-OFFLINE-30M-WIN | 23–25: keyboard/AT observation, frozen responsiveness workloads and OS-observed Manual isolation. |

Ticket22 retains selective/reversible migration through ticket03's inventory;
native keymap support is not import or extension compatibility. Ticket26 retains
all its blockers and five real working days of attempts, fallback and evidence,
with zero unresolved data-loss, unauthorized-write or workflow-blocking defects.
This set does not replace separate assisted acceptance, full language/platform
qualification, or final ten-day/two-independent-session obligations.

## Reproducible routes and evidence

Run downstream commands from the disposable Legion repository root. Record full
candidate commit, lockfile, package/executable digests, host/tool identities,
workspace path, command/environment, external oracle and actual outcome per run.
Product launch uses the packaged executable on that workspace, not cargo-run as
native acceptance. The package directory must contain legion-desktop.exe.

```powershell
# Existing native input harness; ticket04 must select the extracted executable directory.
cargo run -p xtask -- native-product-acceptance --out-dir D:/legion-ide-2026-tools/pilot-native-evidence-87580fa --package-dir D:/legion-ide-2026-tools/pilot-candidate-87580fa --driver D:/legion-ide-2026-tools/native-input-target/release/legion-input-driver.exe
# Frozen real Rust build/test commands, run in product and host for comparison.
cargo build --workspace --locked --offline
cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict --locked --offline
# Independent disk/Git identity oracles in the disposable workspace.
git rev-parse HEAD
Get-FileHash Cargo.lock -Algorithm SHA256
git status --porcelain
git diff --cached
git diff
```

The harness does not orchestrate every frozen scenario. Their native steps and
external oracles must still be performed and attached by their owners. Debuggee
launch arguments/PDB, test discovery additions, performance datasets/targets and
AT transcript details remain pre-run inputs, not fabricated observations.

Already performed externally at 87580fa, not repeated in ticket02: with
process-local LEGION_DAP_DOGFOOD=1 and LEGION_DAP_ADAPTER set to the exact adapter
path above, `cargo test -p legion-debug --test system_adapter_dogfood --target-dir D:/legion-ide-2026-tools/qualification-target -- --nocapture`
passed 1 test, 0 failed/ignored. Real initialize/disconnect proves DAP connectivity
only; launch/step, MSVC/PDB variables/evaluation, policy, native UI and crash
recovery remain unqualified. CodeLLDB MSVC enum-decoding limitations must be
reported by ticket18. Its unsupported --version flag exited 1; package.json pins
1.12.3, and that flag result is not a protocol failure.

Candidate package verification used scripts/verify-native-package.ps1 with exact
SourceSha, ReleaseVersion 0.0.1 and candidate WorkspaceRoot; exit 1. Checksum,
metadata, MSI ProductVersion and extraction structure passed. Headless beta-smoke
failed: both searches reported completed Running/results0; Assist proposal was
refused with offline.ai_feature_disabled. Build emitted 40 legion-app offline
unused-code warnings, unchanged. This is package/build evidence with a failed
verification gate, not native-input or product qualification. Evidence:
D:/legion-ide-2026-tools/pilot-candidate-87580fa/PACKAGE-EVIDENCE.txt,
VALIDATION-SUMMARY.toml in the same directory, and
D:/legion-ide-2026-candidate/target/release-smoke/windows-x64-msi/smoke/beta-smoke.md.
The bounded follow-up is to inspect the smoke Manual/offline contract and
asynchronous search completion before repair; do not weaken assertions or enable
AI to obtain Manual qualification. No native-input journey has run.

## Remaining prerequisites and authority

- Ticket04: resolve the candidate package-verifier failure, identify installed/extracted
  executable directory and digests/manifest, bind source and lockfile identities,
  and capture real native keyboard/pointer/text/clipboard/command disk/UIA oracles.
- Ticket07/23: actual CJK IME installed and active for the product window,
  composition and committed-byte observation, interactive UIA/assistive-technology
  session and retained human/native evidence. Attachment alone supplies none.
- Ticket15–18: pinned tool processes, cached dependencies/linker availability,
  frozen source positions and debuggee/PDB/expected values; real Rust workflow
  compatibility and adapter-denial/recovery results. Handshake is a prerequisite.
- Ticket24: exact representative large-file datasets, GPU/display/power context,
  baseline and frozen targets before scoring; retain bounded text/cache budgets.
- Ticket03/22 and 26: migration inventory/import demonstration and real pilot
  observation with all attempts/fallback. No missing prerequisite counts as pass.
- Owning downstream tickets reconcile canonical provisional configuration joins
  before qualification acceptance. This bounded document leaves those records
  unchanged. Other hosts/languages, signed distribution, remote/extension/team
  services, assisted providers and final independent audits retain their owners.

UI emits intents; app coordinates; editor owns buffers; workspace owns gated
writes. Save fingerprints/versions/generations and proposal review remain intact.
Driver/provider output never grants mutation authority. Signing descriptors remain
dry-run/no-production-signer. This publication is not runtime or release acceptance.

## Documentation verification

Planned before editing: `cargo run -p xtask -- docs-hygiene`,
`cargo run -p xtask -- verify-completion-register --root .`, and
`git diff --check`, once each. Results are recorded in ticket02; these checks
validate documentation/register consistency only. Independent review PASS on 2026-10-08; sole P2 offline mapping resolved.


Actual results: docs-hygiene exit 0 (documentation hygiene checks passed);
verify-completion-register exit 0 (register structure only; all 419 acceptance
values remain unassessed). Each ran once; no runtime/native checks were executed.
Tracked patch: git diff --check exit 0, with an LF-to-CRLF normalization notice; the new untracked document is outside that Git check. Independent review PASS on 2026-10-08; sole P2 offline mapping resolved.
