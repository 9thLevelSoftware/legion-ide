# SPIKE-000: Platform Boundary Proof

## Status

Accepted

Accepted at: 2026-05-14T02:07:05Z

## Objective

Validate that `legion-platform` is constrained to OS service abstractions and does not absorb editor logic, window ownership, model ownership, or high-level request routing.

## Scope

- Filesystem, path normalization, process spawning, PTY, watcher, environment, and time abstractions.
- Platform error normalization for OS boundary failures.
- Explicit exclusion of editor, window, model, and request-routing ownership.

## Evidence artifacts

- `plans/evidence/phase-0/platform-boundary-api-map.md`
- `plans/evidence/phase-0/cargo-check-workspace-all-targets.txt`
- `plans/evidence/phase-0/cargo-test-workspace-all-targets.txt`
- `plans/evidence/phase-0/cargo-clippy-workspace-all-targets.txt`

## Accepted API ownership map

| Public API | Platform-owned OS concern | Not platform-owned |
|---|---|---|
| `PlatformError` | OS boundary error normalization | Editor errors, window errors, model errors, request-routing errors |
| `PlatformError::PermissionDenied` | OS permission failures | Trust inference or security policy |
| `PlatformError::NotFound` | OS path absence | Workspace identity or file model authority |
| `PlatformError::Encoding` | File decoding failure | Text-model ownership |
| `PlatformError::SymlinkLoop` | Path resolution failure | Workspace tree authority |
| `PlatformError::PathTooLong` | Path length failure | Large-file editor policy |
| `PlatformError::AtomicReplaceUnsupported` | Filesystem atomic-replace capability | Save lifecycle ownership |
| `PlatformError::WatcherOverflow` | Watcher overflow signal | Recovery policy or tree state ownership |
| `PlatformError::ProcessSpawnFailure` | Process spawn failure | Terminal command authorization |
| `PlatformError::PtyUnavailable` | PTY backend availability | Terminal session semantics |
| `PlatformError::Timeout` | OS/process timeout reporting | Request routing |
| `PlatformError::Cancelled` | Cancellation propagation | Application orchestration |
| `PlatformError::Io` | Generic I/O fallback | Domain mutation authority |
| `PathNormalizationService` | Normalize, canonicalize, and compare paths | Workspace authorization and identity |
| `FileSystemService` | File read/write/list/hash primitives | Workspace VFS, save approval, conflict semantics |
| `ProcessService` | OS process execution primitive | Terminal command policy |
| `PtyService` | PTY backend primitive | Terminal UX/session model |
| `WatcherService` | OS watcher snapshot primitive | Workspace tree refresh and bounded recovery policy |
| `EnvironmentService` | Process environment lookup/normalization | Runtime launch policy |
| `TimeService` | Clock, sleep, deadline primitive | Runtime scheduling policy |
| `ProcessRequest` | Process launch DTO | Command palette or request router DTO |
| `ProcessRequest::new` | Process launch constructor | Trust or route inference |
| `ProcessResult` | Process completion DTO | Terminal/application interpretation |
| `PtyRequest` | PTY launch DTO | Terminal authorization |
| `PtySession` | PTY backend descriptor | User-facing terminal model |
| `NativeFileSystem` | Native filesystem adapter | Workspace identity, trust, tree, save orchestration |
| `NativeProcessService` | Native process adapter | Security policy |
| `NativeWatcherService` | Native watcher adapter | Workspace recovery and tree state |
| `NativePtyService` | Native PTY adapter placeholder | Terminal shell UX |
| `NativeEnvironmentService` | Native environment adapter | Application environment policy |
| `NativeTimeService` | Native time adapter | Scheduler policy |
| `shell_title` | Legacy static spike label | Window ownership, renderer state, shell model |

## Boundary decision

The platform boundary is accepted. `legion-platform` owns only OS-adjacent primitives. Editor state, window state, domain models, and request routing are explicitly not platform-owned.
