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
- 40: claimed on `codex/ide-2026-ticket-040`; explicit provider configuration.
- 235: resolved and independently reviewed; integrated as `7b69a45`.
- Native candidate prerequisite repair: independently reviewed and integrated as
  `07d2187`; rebuilt MSI verification/staging passed, recorded in `9f85092`.
- 04: claimed on `codex/ide-2026-ticket-004`; external native open/edit/save journey.

The offline candidate at source `87580fae51293ca619a71535a184ef1a96a00b9c`
packaged successfully (MSI SHA-256
`f73b50214c81bdb1498672268385d9439443bc420e4fb6099901fd4602d07976`).
`verify-native-package.ps1` passed checksum, metadata, version and extraction,
but failed headless workflow smoke: both search results were still `Running`,
and the Assist proposal was refused with `offline.ai_feature_disabled`.
The bounded repair addresses asynchronous smoke completion and the offline
Manual smoke contract; it must not weaken acceptance or enable AI implicitly.
Raw evidence remains in `D:/legion-ide-2026-tools/pilot-candidate-87580fa/`.
Native-input acceptance has not run.

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
