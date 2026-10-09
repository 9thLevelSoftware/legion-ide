# Ticket 63 bounded HTTP configuration persistence

Status: implementation and focused checks complete; Euclid independent review PASS received for all seven files in patch SHA256 `D07D2A9911F050D2BC0C4D50219F8CB69AA99398A09C74E892FAA31FB706532C`. Coordinator authorized the bounded local commit. Ticket 63 remains open.

## Boundaries

Worktree `D:/legion-ide-2026-workers/ticket-063`, branch `codex/ide-2026-ticket-063`. Started at integrated `1f647cc`; fast-forwarded without conflicts to `26821c7526db6d04a59ea8be8f68782b9d5c10b9` before handoff. This includes the coordinator's reviewed storage NoEntry fix `0b8dd68`. No storage or provider configuration/UI implementation was edited. Passing tests were not repeated for the integration merge.

After review PASS, fast-forwarded through provider UI integration `f3f9436` and native diagnostic integration `200f8fb`. Both merges were conflict-free and changed no files owned by this persistence slice. Only this evidence and issue 63 received post-review edits to record approval/integration; no semantic changes or repeated successful checks. No push, native run or live peer operation was performed.

The agreed public seams are AppComposition capture/restore and DesktopSessionStore save/load followed by public inspection or credential revocation. This extends the [reviewed HTTP core](ide-2026-ticket063-named-mcp-peer.md), using ticket 63/spec/authority boundaries and the TDD skill. No separate store, atomic writer, transport, orchestrator, inference route, UI or process runtime was introduced.

- App `named_mcp_peer.rs`: private version-1 HTTP configuration codec, strict unknown-field rejection, 64 KiB JSON limit, at most 32 unique peers, endpoint maximum 4096 bytes, existing role/protocol/transport validation and secret-pattern screening. Display metadata and exact validated HTTP endpoint/authentication/scopes persist. Credential values and supplied secret references, grants, revisions, runtime/client state and health do not.
- Protocol `WorkbenchSettingsRecord`: optional `named_mcp_peer_configuration_json`; absent legacy field restores an empty peer set. Strict codec records do not silently migrate unsupported versions or malformed fields to defaults.
- Shared app `lib.rs`: only workbench record initialization and capture/restore. Preflight idle state, MCP parse/prepared map including revision overflow, memory parse/service construction, and provider validation happen before active configuration mutation. Provider's existing restore validates before its first mutation; remaining MCP installation is infallible. Successful restore invalidates old named runtimes/registries/grants, creates ungranted Configured peers, and returns to Manual.
- Desktop `session.rs`: validates MCP metadata before save and load using the same app codec. Existing temp/write/sync/atomic replacement implementation is unchanged. Offline builds reject a present MCP configuration field explicitly, rather than silently discard it; absent legacy fields remain supported.
- New `crates/legion-desktop/tests/named_mcp_peer_persistence.rs`: real reopened files and a real Windows replacement-denial handle; synthetic in-memory SecretStore and loopback listener with no accepted connection. The binding oracle is successful revocation of the preexisting endpoint-bound secret after reopen.

Stdio declarations explicitly fail capture in this HTTP-only persistence slice; no launch details/arguments are written. Stdio/server persistence and activation remain future bounded work. Secret references are deterministically derived from restored configuration using the existing reviewed binding; no arbitrary persisted reference is accepted. No SecretStore call is made by the codec or restore.

## Exact commands and logs

All commands run from the worktree above. Logs are local under `D:/legion-ide-2026-notes/`. Each cargo command redirected both streams with `*> D:/legion-ide-2026-notes/<log>`; logs were inspected with `Get-Content ... -Tail ...`. Cargo failure status is taken from the captured compiler/test result, not the PowerShell tail command's exit code.

| Exact command | Log | Observed result |
| --- | --- | --- |
| `cargo test -p legion-desktop --test named_mcp_peer_persistence reopen_preserves_configuration_binding_without_grants_secrets_or_connections -- --exact` | `ticket63-persist-reopen-red.log` | Initial harness failure: whole-record serde Value conversion rejected a u128 (`number out of range`). No product conclusion. |
| Same exact command | `ticket63-persist-reopen-red2.log` | Expected red after narrowing Value inspection to settings: missing captured MCP field. |
| Same exact command | `ticket63-persist-reopen-green.log` | 1 passed. Reopen binding, no retained credential/grant/health, Manual denial, permission denial, no connection. |
| `cargo test -p legion-desktop --test named_mcp_peer_persistence invalid_mcp_metadata_preserves_saved_bytes_and_all_active_domains -- --exact` | `ticket63-persist-invalid-red.log` | Expected red: unsupported record version was accepted. |
| Same exact command | `ticket63-persist-invalid-green.log` | 1 passed across malformed JSON/version/size, unknown fields, credential URL, excessive endpoint, protocol/role mismatch, duplicates and peer count. Save/load/restore reject; old bytes and active settings/provider/MCP grant/memory survive. |
| `cargo test -p legion-desktop --test named_mcp_peer_persistence legacy_reopen_clears_mcp_grants_without_deleting_bound_credentials -- --exact` | `ticket63-persist-legacy-red.log` | Expected red: existing app remained Assist on restore. |
| Same exact command | `ticket63-persist-legacy-green.log` | 1 passed. Missing legacy field clears peers/grants, preserves stored secret, returns to Manual. |
| `cargo test -p legion-desktop --test named_mcp_peer_persistence -- --skip reopen_preserves_configuration_binding_without_grants_secrets_or_connections --skip invalid_mcp_metadata_preserves_saved_bytes_and_all_active_domains --skip legacy_reopen_clears_mcp_grants_without_deleting_bound_credentials` | `ticket63-persist-remaining.log` | 2 passed: provider/memory atomic rejection and actual Windows publication failure/reopen. 1 harness assertion failed: expected error prefix omitted existing `phase 4`. |
| `cargo test -p legion-desktop --test named_mcp_peer_persistence capture_rejects_unpersistable_launch_details_without_running_a_process -- --exact` | `ticket63-persist-capture-green.log` | 1 passed after correcting expected prefix; only affected failed test rerun. No process launched. |
| `cargo test -p legion-desktop --test provider_profile_persistence` | `ticket63-persist-provider-regression.log` | 5 passed once, covering provider reopen, invalid metadata, legacy, memory atomicity and real publication failure. |
| `rustfmt --check --edition 2024 --config skip_children=true crates/legion-app/src/named_mcp_peer.rs crates/legion-app/src/lib.rs crates/legion-protocol/src/lib.rs crates/legion-desktop/src/session.rs crates/legion-desktop/tests/named_mcp_peer_persistence.rs` | `ticket63-persist-format.log` | Passed (empty output), after one formatting pass with the same file list and without `--check`. |
| `cargo clippy -p legion-desktop --lib --test named_mcp_peer_persistence -- -D warnings` | `ticket63-persist-clippy.log` | Passed once. Existing patched `epaint` dependency future-compatibility float warning remains. |
| `cargo check -p legion-desktop --no-default-features --features offline --lib` | `ticket63-persist-offline.log` | Passed once after integration merge; 42 existing app warnings. |
| `git diff --check` | `ticket63-persist-diff-check.log` | Passed; only Git line-ending conversion warnings. |

The six new tests passed through the targeted runs above; no combined repeat of successful new tests was performed. The first three behavior gaps received explicit red/green cycles; existing provider/memory rejection and the reused writer's Windows failure path passed on first exercise. No product repair failed twice.

## Remaining gates

Independent delta review passed and the coordinator authorized the bounded local commit. No ticket acceptance checkbox is credited. Native MCP configuration input/UI, stdio/server paths, live peer authentication/tool invocation, named real-peer selection/endpoint/runtime prerequisites (tickets 64/65), non-Windows replacement behavior and full product/release qualification remain open. Existing core evidence's Windows `0xc0000409` abnormal abort remains unresolved; this slice neither repairs it nor reclassifies its diagnostic rerun as proof of repair. Coordinator-reported native synthetic keyring qualification is a separate prerequisite, not MCP native qualification.
