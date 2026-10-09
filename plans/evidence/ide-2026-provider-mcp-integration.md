# Provider, persistence and MCP integration checkpoint

Date: 2026-10-09. Integration source:
`ac8087f7b1c0a6f872ffd7715ee6eedefeb0e40e` on
`codex/ide-2026-integration`. No product acceptance is promoted.

## Reviewed changes

- Provider core `77166f7`: explicit named endpoint/model configuration,
  route-bound secure credentials, refusal without fixture fallback, bounded MiMo
  protocol options and stale-prediction invalidation.
- Persistence `928275e`: metadata-only profiles through existing workspace
  session storage, atomic failure handling and Manual restoration.
- MCP core `ac8087f`: named HTTP peer configuration/health, scoped credentials
  and local revocation. Review fixes bind keys to the full configuration and
  redact transport failures before workflow metadata retention.

Detailed independent reviews and behavioral checks are retained in
[provider evidence](ticket-040-explicit-provider-core.md) and
[MCP evidence](ide-2026-ticket063-named-mcp-peer.md). Native driver and provisional
platform work are separately recorded in
[ticket 04 evidence](ide-2026-ticket004-native-journey.md) and
[configuration inventory](../completion/ide-2026-language-platform-configurations.md).

## Combined verification

Planned after the two app/protocol changes were combined, to catch integration
errors that their individual checks could not establish. Ran once from the clean
ticket-063 checkout at exact source `ac8087f`, while the reviewed commit was being
fast-forwarded unchanged into integration:

```text
cargo check --locked -p legion-desktop --all-targets --target-dir D:/legion-ide-2026-tools/qualification-target
```

Result: exit 0; finished in 40.36 seconds. Log:
`D:/legion-ide-2026-notes/provider-mcp-combined-compile-ac8087f.log`.
This compiles desktop targets and their app/protocol dependencies. It is not a
test execution, workspace-wide gate, native journey, real-provider/peer call,
three-platform check or release qualification. Previously successful behavioral
checks were not repeated for this compile checkpoint.

## Unresolved outcomes

- Ticket 04 remains needs-info: the latest attended attempt passed main-window
  selection, foreground and COM/UIA bootstrap, then found zero exact Explorer
  README.md elements before input. No open/edit/save outcome passed; clone clean.
- Ticket 40 remains open for native profile/credential UI, actual OS-keyring
  durability/revocation and bounded live Singapore Token Plan MiMo qualification.
  Credentials have not been supplied through that secure flow; no live call ran.
- Ticket 63 remains open for UI/persistence, stdio/server activation and real-peer
  qualification. Its earlier isolated test-process abort `0xc0000409` is still
  unexplained; a passing diagnostic rerun is not a fix or stability proof.
- Ticket 88 keeps every required matrix cell provisional where host, tool,
  fixture or owner evidence is missing. No installation or ratification is
  inferred from the planning inventory.

Canonical acceptance entries remain unchanged. Original planning WIP stays in
`D:/legion-ide`; no push, publication, signing or deployment was performed.

The new checkpoint documentation passed `cargo run -p xtask --target-dir
D:/legion-ide-2026-tools/qualification-target -- docs-hygiene` (exit 0).

## Provider UI and MCP persistence integration follow-up

On 2026-10-09, independently reviewed provider settings UI `1fb8180` was
integrated through `f3f9436`, reporting-only native driver diagnostics through
`200f8fb`, and MCP HTTP metadata persistence through
`1da8848c658bad31fb1e7cc806f48fe52bd244c9`. Merger checks confirmed ancestry,
scope and clean integration state. No push or native/live run was part of merging.

The coordinator ran once on clean `1da8848`:

```text
cargo check --locked -p legion-desktop --all-targets --target-dir D:/legion-ide-2026-tools/qualification-target
```

Result: exit 0, 28.39 seconds. Combined output is retained at
`D:/legion-ide-2026-notes/provider-ui-mcp-persistence-combined-1da8848.log`.
This checks compilation of the composed desktop targets after both new slices;
it is not runtime or native qualification. The focused suites in the
[provider UI receipt](ticket-040-explicit-provider-core.md) and
[MCP persistence receipt](ide-2026-ticket063-mcp-persistence.md) were not repeated.

Native UI/profile credential entry/reopen, live MiMo responses, MCP native UI and
real peers remain unqualified. The earlier unexplained `0xc0000409` remains open.
No ticket or canonical acceptance checkbox is promoted.

## Wave1 settings and shared-drain composition

This later checkpoint supersedes the earlier current-state descriptions above
without replacing their historical evidence. Base integration source was
`ee6896f1e340113cfa1cddbd873f2bc7cf0cb8e7`. It already includes accepted ticket 05's
app/storage save-conflict slice, reviewed provider connection-check source,
bounded ticket 155 contract documentation and ticket 04 component diagnostics.
At this source checkpoint, counts were 5 resolved / 247 ready-for-agent / 2 needs-info.

The frozen [MCP settings handoff](ide-2026-ticket063-mcp-settings.md) was verified
against all 45 manifest entries. Worker patch SHA-256:
`4f86b9073081ce49e3aa54ce25f7feac84a190127aa0ec7f19d2cb19ee686fa5`.
Pauli independently reviewed the complete patch: PASS for bounded local partial
implementation, with the crash qualification blocker retained below. Shared
desktop hunks were composed without copying over provider actions or polling.

The coordinator independently reviewed the integrator's two production changes
and new regression. Installing a denying ceiling first preserves provider
cancellation, then uses the common mode setter; that setter revokes pending MCP
grants before testing the existing shared drain counter. Provider completion
reconciles the ceiling only after releasing its handoff lane. MCP polling
reconciles before/after results, including retired-only drain. New admissions
remain denied while a displayed non-Manual mode represents unfinished drain.

Verified final raw file identities for the integrator-authored delta:

| File | SHA-256 |
| --- | --- |
| `crates/legion-app/src/lib.rs` | `777ccf63ac066d6d9b8b8de3b1651fa053e924ec5af77eab4a96fe5d439d46ac` |
| `crates/legion-app/src/provider_configuration.rs` | `db0bfaf6b268be5ac08d8fd38d8e5bb47d38f927de2b9556e8253378da26f23c` |
| `crates/legion-app/tests/provider_mcp_drain_composition.rs` | `0254fb232d14ccb59e144015f42a523a532878549cb10c40ab634c05cbcae38e` |

The production diff is retained externally as
`D:/legion-ide-2026-notes/wave1-provider-mcp-production-composition-ee6896f.patch`,
SHA-256 `027b6653e1a53807271c435b3560afa4c20e8ae0e68bc57f2d3a0d70b1b6bdae`.
No production edit followed the coordinator's author-delta review.

### Combined behavioral evidence

```text
cargo test -p legion-app --test provider_mcp_drain_composition -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target -- --nocapture
```

Result: exit 0, 1 test passed in 0.32 seconds (compile 3.73 seconds), covering
active and retired MCP workers with both HTTP response-release orders. Independent
held loopback servers prove both requests are live before lowering the signed
ceiling to Manual. Public observations establish immediate admission denial,
honest intermediate mode, final Manual after both drains, rejected late success,
no delegated sandbox allocation and unchanged editor/disk bytes. For retired MCP
there is no per-peer completion projection; the test does not treat a timed poll
as proof that the retired worker completed.

Log: `D:/legion-ide-2026-notes/wave1-provider-mcp-composition-ee6896f-2.log`.
The first invocation failed on the fixture's policy wire spelling `Delegate`
instead of `Delegates`, before issuing either request. That setup failure is
retained in `wave1-provider-mcp-composition-ee6896f-1.log` in the same directory;
it is not a product behavioral red. Only this failed new check was rerun after
the fixture correction. Subsequent rustfmt changed whitespace only; no successful
test was repeated.

### Composed source gates

Commands ran once from integration on the reviewed, unchanged production delta:

```text
cargo check -p legion-desktop --all-targets -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target
cargo check -p legion-desktop --no-default-features --features offline --lib --bins -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target
cargo run -p xtask -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target -- check-deps
cargo run -p xtask -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target -- no-egui-textedit
```

| Check | Result | Log under `D:/legion-ide-2026-notes/` |
| --- | --- | --- |
| Default desktop all-targets | Exit 0, 20.99 seconds | `wave1-composed-desktop-alltargets-ee6896f.log` |
| Offline desktop lib/bins | Exit 0, 6.31 seconds | `wave1-composed-desktop-offline-ee6896f.log` |
| Dependency policy | Exit 0 | `wave1-composed-check-deps-ee6896f.log` |
| No egui TextEdit in the editor canvas | Exit 0 | `wave1-composed-no-egui-textedit-ee6896f.log` |

Scoped formatting of the integrator's three Rust files passed; the frozen worker
files retain their recorded formatting evidence. Offline compilation reports 42
app warnings for unused imports/dead code; the existing vendored epaint f32
fallback warning also remains. No unrelated warning cleanup was performed.
Final docs-hygiene is intentionally deferred until the separate ticket 169
documentation handoff. The default-feature desktop executable will be built
after the local source commit and bound to its exact SHA and executable SHA-256.
No runtime/native or full-workspace/platform result is inferred from compilation.

### Retained qualification limits

The two historical Windows `0xc0000409` MCP test-process aborts remain unresolved.
The first debugger capture attempt failed during harness startup before any test
completion. A separately authorized corrected synchronous LLDB attempt reused the
same frozen EXE/PDB and completed the exact two selected tests, both passing with
inferior exit 0. This single diagnostic did not reproduce the abort; no repair,
cause or uninterrupted green suite is claimed. The complete chronology and
frozen identities are retained in
`D:/legion-ide-2026-notes/ticket063-lldb-20261009-162447-e096a38e/receipt.md`.
Root permits local partial implementation while retaining the intermittent abort
as a native/product/release qualification blocker. No further crash rerun is part
of this checkpoint.

Ticket 04 remains gated by the r4 native root-focus discrepancy; no typing/save
ran and Computer Use remains stopped. Ticket 40 still needs native profile and
credential workflows plus live-provider qualification. Ticket 63 still needs
native settings, real peers, stdio/server and broader cancellation/recovery
qualification. Ticket 88 remains provisional. Ticket 155's bounded document is
not runtime compatibility or representative/host ratification. Ticket 169 is a
separate pending documentation handoff. The old offline r4 installer predates
these provider/MCP changes and is not their executable artifact. No GUI launch,
live remote call, signing, push, PR or product acceptance is part of this wave.

## Committed default executable and documentation follow-up

The reviewed MCP composition, test and evidence were committed locally as
`ddba84fe5da05aa81d722e698b004d7629de4ec6`. The integration checkout was clean
before and after this single default-feature build:

```text
cargo build -p legion-desktop -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target
```

Result: exit 0, 36.24 seconds, dev profile (unoptimized with debug information).
Executable: `D:/legion-ide-2026-tools/qualification-target/debug/legion-desktop.exe`.
Size: 87,169,536 bytes. SHA-256:
`ea5527c21f008e074d8844fbf2a9d54343d53692298ae2666a990fc9621baecf`.
Source tree: `edbd564a6e558e03d4f79eb3a2e43b626b87bd1c`.
The external binding receipt is
`D:/legion-ide-2026-notes/wave1-default-desktop-ddba84fe.json`; build output is
`D:/legion-ide-2026-notes/wave1-default-desktop-build-ddba84fe.log`.
The executable was not launched, packaged, signed or published. Its existence
does not qualify the unresolved Windows abort or any native/live outcome.

Root independently reviewed ticket 169's [remote authority contract](../completion/ide-2026-remote-workspace-authority-contract.md):
PASS for planning only. The two frozen worker SHA-256 values were verified before
import: contract `25a33a86821d65efb6037dc6300f32ceae1d8185b8e5a796ec2111973223e592`,
issue `179ccfbff573df1c57e9dd8fe779b56ba3e54f1d92598e63f05f02df63af46dc`.
Integration restores the issue's original generic verification requirement and
records the authorized planning acceptance in the contract, four criteria and
resolved status. These integration edits supersede those worker byte identities;
the worker remains unchanged. No canonical register or runtime source changes.

The documentation-only resolution raises local counts to 6 resolved / 246
ready-for-agent / 2 needs-info. Actual host/key/credential/engine/image/tool identities remain explicit
external prerequisites for tickets 170 onward. This accepts neither S5-05's
larger protocol package nor A18 product qualification. This later documentation
commit does not change the executable's source binding or require a rebuild.

The single final composed documentation gate passed after the intended planning
acceptance/status/count edits:

```text
cargo run -p xtask -j 2 --target-dir D:/legion-ide-2026-tools/qualification-target -- docs-hygiene
```

Result: exit 0 (Cargo 0.41 seconds). Log:
`D:/legion-ide-2026-notes/wave1-final-docs-hygiene-ddba84fe.log`.
Only this observed result was added afterward; no link or acceptance scope changed
and the successful validator was not repeated. Canonical registers remain
unchanged, so their separate validators were not invoked.
