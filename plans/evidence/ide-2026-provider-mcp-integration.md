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
