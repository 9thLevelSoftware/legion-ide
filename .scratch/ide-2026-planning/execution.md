# Implementation execution record

The user invoked `implement-spec` on 2026-10-08. The approved specification and
254-ticket graph are the execution contract; the earlier planning-only annotation
describes specification creation and does not negate this later authorization.

- Integration branch: `codex/ide-2026-integration`.
- Integration checkout: `D:/legion-ide-2026-integration`.
- Original checkout: `D:/legion-ide`, preserved with its existing planning WIP.
- Base: `28928fe`; approved artifacts committed as the integration starting point.
- Local ticket statuses in this integration checkout are authoritative for execution.
- No push, public PR, provisioning, signing or product acceptance is implied.

## Active work

- 01: resolved and independently reviewed; integrated as `c2a6578`.
- 02: resolved and independently reviewed; integrated as `dfcdd42`.
- 03: resolved and independently reviewed; integrated as `cb817998`.
- 40: explicit provider core integrated as `77166f7`, with reviewed workspace
  session metadata persistence in `928275e` and reviewed native settings UI
  implementation in `1fb8180` (integrated through `f3f9436`). Native input,
  profile credential entry/reopen, connection checking and live qualification remain open.
- 63: reviewed named MCP HTTP core integrated as `ac8087f` and bounded HTTP
  metadata persistence in `1da8848`; UI, stdio/server activation and real-peer
  qualification remain open.
- 88: provisional language/platform inventory integrated as `18ef5f7`; remains
  needs-info pending host/configuration/fixture ratification, without scope reduction.
- 235: resolved and independently reviewed; integrated as `7b69a45`.
- Native candidate prerequisite repair: independently reviewed and integrated as
  `07d2187`; rebuilt MSI verification/staging passed, recorded in `9f85092`.
- 04: partial external driver fixes in `177caa5` and reviewed compact-drawer
  navigation in `00ae938`; the latest attended attempt passed foreground/UIA
  checks but Windows accepted zero of three events from the first drawer click.
  Reporting-only diagnostics in `200f8fb` subsequently captured Windows error 5
  (access denied), with successful foreground/UIA checkpoints and zero accepted
  events. A bounded repair preserves existing input-desktop access, with a native
  non-injecting regression red/green and independent review PASS. The attended
  `b88f7e0` run then successfully clicked the drawer and file; it stopped before
  typing because the driver expected an obsolete debug-style tab label. The
  corrected typed tab oracle observes Clean in a read-only native probe. The
  editor still exposes only text fragments, blocking exact full-document UIA
  observation. Native editing/save remain unqualified; a bounded product
  accessibility repair is assigned in an isolated worktree.
- Credential prerequisite: typed missing-key classification and isolated native
  Windows storage qualification integrated as `0b8dd68`; profile UI/live gates remain open.

The offline candidate at source `87580fae51293ca619a71535a184ef1a96a00b9c`
packaged successfully (MSI SHA-256
`f73b50214c81bdb1498672268385d9439443bc420e4fb6099901fd4602d07976`).
`verify-native-package.ps1` passed checksum, metadata, version and extraction,
but failed headless workflow smoke: both search results were still `Running`,
and the Assist proposal was refused with `offline.ai_feature_disabled`.
The bounded repair addresses asynchronous smoke completion and the offline
Manual smoke contract; it must not weaken acceptance or enable AI implicitly.
Raw evidence remains in `D:/legion-ide-2026-tools/pilot-candidate-87580fa/`.
Earlier native attempts were blocked without accepted input. The latest
access-preserving driver accepted the guarded Explorer drawer/file clicks, but
open/edit/save and full input conformance remain unqualified. Partial repairs
address helper-window selection, STA startup and accidental access reduction.

A subsequent read-only UIA trace observed the compact Explorer drawer with no
file entries across 60 samples over 15 seconds. The reviewed driver now opens
that real control through guarded OS input when needed. Its first attended
navigation attempt timed out while the owner was away; an owner-requested retry
reached the drawer but SendInput accepted zero events. After access preservation,
guarded clicks succeeded and the run reached the tab-observation stage.
The [diagnostic receipt](../../plans/evidence/ide-2026-native-sendinput-diagnostic.md)
records both the failure and the reviewed error-reporting change. No fresh
attendance is inferred from an unanswered availability question.

[Tab/text oracle evidence](../../plans/evidence/ide-2026-native-tab-text-oracles.md)
records the obsolete driver label predicate, successful clean-tab observation,
and the remaining full-document accessibility gap. Direct `--file` probe setup is
not credited as Explorer or native editing acceptance.

The combined provider/persistence/MCP source at `ac8087f` passed one desktop
all-targets compile. [Integration evidence](../../plans/evidence/ide-2026-provider-mcp-integration.md)
records the exact source and command. This is compile evidence only; native,
OS-keyring, live-provider/peer and full-workspace/platform qualification are not
established. The MCP checkpoint's isolated `0xc0000409` test-process abort remains
unexplained despite its successful diagnostic rerun.

After UI and MCP persistence integration, clean source `1da8848` passed one new
desktop all-targets compile in 28.39 seconds. The same integration receipt records
that composed-source check. Focused runtime suites were not repeated; native/live
gates and the unexplained prior abort remain open.

[Native keyring prerequisite evidence](../../plans/evidence/ide-2026-native-keyring-prerequisite.md)
records synthetic-only Windows store/read/replace/revoke across fresh processes,
with cleanup. It is not native profile UI or live authentication acceptance.
[Windows toolchain inventory](../../plans/evidence/ide-2026-windows-toolchain-inventory.md)
adds installed compiler/linker/SDK/component identities without changing tools.

## Verification policy

Use approved AppComposition/native/process/storage boundaries. Documentation-only
work uses existing document/register validators rather than artificial runtime
tests. Plan checks before edits, run each once, and repeat only affected failures
after supported fixes. Apply the TDD skill for new runtime behavior. Independent
review precedes integration of consequential worker changes.

## Owner decisions during execution

On 2026-10-08 the user answered “Yes to both” to these concrete preflight choices:

- Pilot basis: observed Windows 11 Pro x64 build 26200 on the i9-14900HX / 32 GiB
  machine, Rust/Cargo/rust-analyzer 1.98.1, Legion-on-Legion, and native
  Rust/edit/test/debug/Git workflows. This ratifies these observed values only;
  missing debugger and native-input driver remain explicit prerequisites.
- Native equivalents may satisfy essential initial pilot workflows. Installed
  Containers 2.5.2 and Remote Containers 0.469.0 belong to later required
  extension/remote qualification, not the initial pilot critical path. Their
  presence is not compatibility evidence and the full obligations remain.

Source inspection and host observations are recorded outside the checkout in
`D:/legion-ide-2026-notes/pilot-prerequisites.md`; they are not product acceptance.

The user subsequently approved CodeLLDB 1.12.3 Windows x64 as a qualification
candidate and its isolated download/extraction. The official VSIX was downloaded
to `D:/legion-ide-2026-tools/codelldb-1.12.3/` and its SHA-256 matched
`a916e509308dac817732f63ca604a8b93ed29cd16f38a2fa9f0b64ed58e8f51a`.
The adapter exists at `extracted/extension/adapter/codelldb.exe`. This records
artifact integrity and availability only: no extension was installed and no
DAP/MSVC compatibility or native product acceptance is claimed.

The existing `legion-input-driver` crate built successfully with
`cargo build -p legion-input-driver --release --target-dir D:/legion-ide-2026-tools/native-input-target`.
Its `--probe-session` handshake returned exit 0, `status = "attached"`,
`interactive_session = true`, and desktop `Default`; the report is retained at
`D:/legion-ide-2026-notes/driver-session.toml`. The driver binary is at
`D:/legion-ide-2026-tools/native-input-target/release/legion-input-driver.exe`.
This supersedes the missing-driver preflight observation for this selected path.
No input was injected and no packaged product/native workflow was qualified.

## Completion limits

On 2026-10-09 the owner selected MiMo 2.6 Pro as the initial inexpensive testing
candidate, clarified an existing Xiaomi subscription, and supplied the dedicated
Token Plan endpoints. The pilot profile uses the OpenAI-compatible route
`https://token-plan-sgp.xiaomimimo.com/v1` with model `mimo-v2.6-pro`.
The supplied alternative is `https://token-plan-sgp.xiaomimimo.com/anthropic`.
Credentials remain pending through secure storage, with no live inference yet.
Subscription exhaustion must not silently switch to pay-as-you-go. Official
protocol details and dated source links are in
`D:/legion-ide-2026-notes/mimo-testing-candidate.md`.

Keep implementation, experiments and product acceptance separate. Real provider,
native/AT host, formal-worker, signing/service and human-observation prerequisites
remain explicit and block only the dependent work. Do not close a ticket by
substituting simulated evidence or by counting missing access as a negative result.
