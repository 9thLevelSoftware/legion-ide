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

Current local ticket counts: **6 resolved, 246 ready-for-agent, 2 needs-info**.
The entries below distinguish accepted slices from implementation and diagnostic
progress. Later narrative sections retain the earlier checkpoint history.

- 01: resolved and independently reviewed; integrated as `c2a6578`.
- 02: resolved and independently reviewed; integrated as `dfcdd42`.
- 03: resolved and independently reviewed; integrated as `cb817998`.
- 05: resolved at the A02 app workflow/storage-reopen boundary; independent
  review PASS and source integrated as `9cff366`. [Save-conflict evidence](../../plans/evidence/ide-2026-ticket005-save-conflict-recovery.md)
  records 15 unique focused passing tests and preserved original/recovery-copy
  bytes across conflict and actual store reopen. No native GUI, abrupt-kill,
  broader A02 or ticket 04 acceptance is inferred. The composed source checks are
  recorded in the [wave integration evidence](../../plans/evidence/ide-2026-provider-mcp-integration.md).
- 40: explicit provider core integrated as `77166f7`, with reviewed workspace
  session metadata persistence in `928275e` and reviewed native settings UI
  implementation in `1fb8180` (integrated through `f3f9436`). The independently
  reviewed asynchronous connection check and installed-ceiling admission repair
  are integrated as `ee6896f`; [connection-check evidence](../../plans/evidence/ticket-040-provider-connection-check.md)
  records 20 distinct focused passing tests across the lane's recorded revisions.
  Native profile/credential entry/reopen and live qualification remain open.
- 63: reviewed named MCP HTTP core integrated as `ac8087f` and bounded HTTP
  metadata persistence in `1da8848`. The reviewed wave1 settings/async operation
  slice and provider/MCP drain composition are included in this checkpoint;
  [settings evidence](../../plans/evidence/ide-2026-ticket063-mcp-settings.md)
  records 25 distinct focused worker passes and the separate integration regression.
  Native settings, stdio/server activation and real-peer qualification remain open.
  Two historical Windows `0xc0000409` aborts remain unresolved. One completed
  two-test debugger diagnostic did not reproduce the abort; no repair or
  uninterrupted green-suite claim is made. Root authorized local partial
  implementation while retaining this native/product/release qualification blocker.
- 88: provisional language/platform inventory integrated as `18ef5f7`; remains
  needs-info pending host/configuration/fixture ratification, without scope reduction.
- 235: resolved and independently reviewed; integrated as `7b69a45`.
- 155: bounded [extension contract](../../plans/completion/ide-2026-extension-execution-contract.md)
  documentation integrated as `2b2b989` after root review. It remains
  ready-for-agent: authorized API/runtime/limit/ADR work is actionable, while
  actual representative/host qualification is not invented or ratified.
- 169: resolved for the bounded [remote-workspace authority contract](../../plans/completion/ide-2026-remote-workspace-authority-contract.md)
  after root independent review and the final documentation gate. This accepts
  planning only, not S5-05 protocol implementation or A18. Actual hosts, keys,
  credentials, image/agent/tool identities and runtime evidence remain external
  prerequisites for tickets 170 onward.
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
  bounded product accessibility repair passed independent review and integrated
  as `caf1821`. Its frozen offline MSI 0.0.3 passed packaging verification and a
  native read-only check of the exact complete README, clean tab and editor
  geometry. The later frozen r4 source `fb9982f` passed packaging and actual
  Computer Use Explorer/open/close/editor-point prerequisites, but native UIA
  focus still returned the root with both editor focus properties false.
  No typing/save was attempted; ticket 04 remains unaccepted. Component-only
  compact-layout coverage (`3471753`) and delivery diagnostics (`21e22f`) do not
  reproduce or repair this native gap. Computer Use remains stopped after Escape.
- Credential prerequisite: typed missing-key classification and isolated native
  Windows storage qualification integrated as `0b8dd68`; profile UI/live gates remain open.

The composed provider/MCP regression passed one test covering four active/retired
MCP and response-order branches. Default desktop all-targets, offline lib/bins,
dependency policy and no-egui-textedit checks passed once on the composed source.
MCP composition and its reviewed evidence were committed as
`ddba84fe5da05aa81d722e698b004d7629de4ec6`. A default-feature desktop executable
built successfully from that clean source in 36.24 seconds; exact path/hash are
recorded in the integration evidence. The later ticket 169 documentation batch
does not change production source or require a rebuild. No GUI, live
provider/peer, full-workspace/platform or release qualification is inferred.

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

[Candidate r3 evidence](../../plans/evidence/ide-2026-ticket004-candidate-r3.md)
records reviewed source `caf1821`, installer/executable identities, successful
offline verification/staging, and exact complete native UIA observation. The
reference checkout remained clean. This resolves the small-document observation
prerequisite without promoting native edit/save or large-document accessibility.

The subsequent attended r3 attempt detected owner foreground and UIA successfully,
then blocked before driver input because global README name matching selected
three unrelated surfaces. Owned-child cleanup caused the reported immediate
close. The same r3 receipt records restored session metadata as a test-isolation
gap, a fresh-session probe, and native Explorer Dialog/Button ancestry with
screenshots. Reviewed driver repair `5d7b298` integrated locally after 11 distinct
focused tests; actual scoped file/close point checks passed with duplicate labels
present. A separate native prerequisite remains: the foreground app reports
window-root keyboard focus after editor click. A bounded product focus repair is
implemented with 18 distinct focused tests and default/offline compilation
passing; [focus repair evidence](../../plans/evidence/ide-2026-ticket004-editor-focus-prerequisite.md)
records reviewed source `fb9982f` and the pending native r4 gate. The strict driver guard
remains, and no typing/save or ticket 04 acceptance is claimed.

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
