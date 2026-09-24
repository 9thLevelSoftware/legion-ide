# Remote development scope audit

Date: 2026-09-05

This audit expands `COMP-SCOPE-FAMILY-18` from baseline commit `6b95f76`.
It uses the approved family table, the linked Stage 5 plan, the production
qualification plan, the four remote ADRs, current remote source, and retained
remote evidence. Every source path below is literal and repository-relative;
no historical status is treated as product acceptance.

| ID | Outcome boundary | Current trace and classification | Product limit |
| --- | --- | --- | --- |
| COMP-REMOTE-001 | Finite SSH/container matrix and real acceptance oracles | No canonical remote matrix or scenarios found; `docs/superpowers/plans/2026-09-04-ai-team-completion.md` S5-07 and `docs/superpowers/plans/2026-09-04-production-qualification.md` define the required future inputs. **absent** | This is a Stage 0/S0-02 prerequisite; host/image/version/OS/tool/identity entries and external oracles remain unassessed. |
| COMP-REMOTE-002 | SSH auth, pinned host identity, credentials, and secret hygiene | `crates/legion-remote/src/lib.rs` `RemoteConnectionSpec`/`plan_ssh_session` validate metadata; `plans/adrs/ADR-0023-remote-transport-security.md` defines policy. **partial** | No SSH connector, host-key exchange, keyring flow, or real host evidence. |
| COMP-REMOTE-003 | Dev-container parse/provision/image/mount/env/port policy | `RemoteDevcontainerConfig` and `plan_devcontainer_session_from_json` parse image/Dockerfile labels, users, folders, features, and mounts. **partial** | No engine launch, image digest verification, mount/env enforcement, or container evidence. |
| COMP-REMOTE-004 | Remote identity, negotiation, scoped handles, replay, health | `RemoteSessionRuntime`, `RemoteTransportStateMachine`, protocol descriptors, and transport tests cover metadata identity/state. **partial** | No shipped client/agent authority path or packaged remote session. |
| COMP-REMOTE-005 | Remote file list/read/stat/watch/refresh | `RemoteFilesystemSnapshot`, seeded fixture files, and filesystem handlers exist in `crates/legion-remote/src/lib.rs`. **partial** | No remote Explorer, watch stream, real filesystem, or host-observed external change. |
| COMP-REMOTE-006 | Remote edit/save with fingerprints and proposal preconditions | `validate_mutation_gate`, `write_file`, stale/conflict outcomes and tests enforce proposal metadata in the fixture harness. **partial** | No actual remote save service or local/remote dirty-buffer product journey. |
| COMP-REMOTE-007 | Remote search and bounded cancellable tools | The approved S5-07 scope names search; no remote search implementation or product route is present. **absent** | Local `COMP-NAV-008` remains canonical for search semantics. |
| COMP-REMOTE-008 | Remote terminal/task/process execution | `handle_process_descriptor` and `handle_pty_descriptor` validate bounded descriptors and cancellation metadata. **partial** | ADR-0024 explicitly says no process/PTY spawning; no remote terminal or task UX. |
| COMP-REMOTE-009 | Remote LSP for all three language groups | `handle_lsp_descriptor` and protocol DTOs are descriptor-only. **partial** | No remote LSP child/session or packaged Rust/TS/JS/Python route; local `COMP-LANG-004/007` remain canonical. |
| COMP-REMOTE-010 | Remote build/test/debug with real effects | No remote build/test/debug dispatcher or debug descriptor product path found. **absent** | Local `COMP-BTD-*`/`COMP-LANG-011/012` remain canonical for semantics. |
| COMP-REMOTE-011 | Remote extension/agent-side tooling lifecycle | Current extension source is local metadata/WASM substrate; no remote extension path. **absent** | Existing extension lifecycle rows remain canonical; no remote runtime evidence. |
| COMP-REMOTE-012 | Authorized port forwarding lifecycle | ADR-0025 defines endpoint policy and listener restrictions, but no forwarding service or UI exists. **absent** | No port bind, collision, close, or external connectivity evidence. |
| COMP-REMOTE-013 | Cancellation, limits, crash cleanup, and bounded shutdown | Runtime errors, cancellation tokens, frame/output limits, and descriptor validation exist. **partial** | No real child/agent process supervisor proving no orphan after failure. |
| COMP-REMOTE-014 | Disconnect/offline/reconnect/resume | `RemoteSessionTransport`, `RemoteTransportStateMachine`, and `crates/legion-remote/tests/transport_reconnect_offline.rs` provide forced-drop, replay-window, token/checkpoint, and session-binding tests. **partial** | Transport metadata is tested; no remote UX or real peer reconnect. |
| COMP-REMOTE-015 | Agent install/upgrade/rollback/close/remove and recovery | Agent package descriptors, digest/signature references, activation, and health metadata exist. **partial** | No installer, atomic activation, rollback drill, remote cleanup, or low-disk/permission recovery. |
| COMP-REMOTE-016 | Privacy/egress disclosure and dirty-work preservation | Default-deny policy, redaction hints, metadata-only audit, proposal preconditions, and canonical `COMP-TRUST-*`/`COMP-PRES-*` dependencies exist. **partial** | No product egress review dialog or remote dirty-buffer journey; no raw source/secret evidence may be inferred. |
| COMP-REMOTE-017 | Packaged real SSH/container qualification | S5-15 and S6 plans require packaged app, real endpoints, all three language groups, external effects, failure/recovery, and independent review. **absent** | No packaged remote endpoint run, desktop controls, or accepted EvidenceRun exists. Git is included through COMP-REMOTE-018. |
| COMP-REMOTE-018 | Remote Git status, diffs, staging, commits, branches, conflicts, remote verbs, and recovery | S5-07 explicitly requires remote Git and externally observed Git changes; `crates/legion-remote/src/lib.rs` has no Git dispatcher. **absent** | Depends on canonical COMP-SCM-001/002/003/006/009/010 and adds only their remote authority/integration and packaged evidence. |

## Exact deduplication boundaries

The replacement retains canonical ownership of shared outcomes. `COMP-PRES-001`,
`COMP-PRES-003`, `COMP-PRES-004`, and `COMP-PRES-008` own general save,
restart, dirty text, and conflict preservation. `COMP-LANG-004` and
`COMP-LANG-007` own language-server lifecycle and proposal-mediated language
edits; `COMP-LANG-011` and `COMP-LANG-012` own build/test/debug semantics.
`COMP-TERM-004` and `COMP-TERM-011` own local PTY behavior and terminal
recovery. `COMP-SCM-001`, `COMP-SCM-002`, `COMP-SCM-003`, `COMP-SCM-006`,
`COMP-SCM-009`, and `COMP-SCM-010` own local Git semantics and packaged Git
evidence; `COMP-REMOTE-018` adds the remote repository boundary only.
`COMP-TRUST-004` and `COMP-TRUST-005` own audit-before-success,
redaction, capability, and egress policy. Remote rows reference these IDs as
dependencies or boundaries and add only remote authority, endpoint, and
packaged integration requirements.

## Evidence limits

`plans/evidence/phase-7/remote-architecture-map.md` and
`plans/evidence/remote/P9-F3-T2-reconnect-offline-evidence.md` establish the
deterministic harness and transport reconnect behavior. They explicitly do not
establish SSH/dev-container UX, remote terminal/LSP/filesystem product paths,
or real peer qualification. The older ADRs are not used to infer permanent
deferral: they are recorded as current substrate and security boundaries while
the approved Stage 5 scope remains required.
