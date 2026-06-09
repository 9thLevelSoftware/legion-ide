# Phase 0 Platform Boundary API Map

Status: Accepted

Accepted at: 2026-05-14T02:07:05Z

Source API file: `crates/legion-platform/src/lib.rs`

## Ownership rule

`legion-platform` owns OS-facing filesystem, path, process, PTY, watcher, environment, and time abstractions only. It does not own editor buffers, editor transactions, shell/window state, project models, workspace identity, trust policy, request routing, AI/provider logic, or UI command dispatch.

## Public API ownership map

| Public API | OS concern owned by platform | Explicit non-platform ownership |
|---|---|---|
| `PlatformError` | Normalized errors for filesystem, process, PTY, watcher, timeout, cancellation, and I/O boundaries | Not an editor, window, model, or request-routing error authority |
| `PlatformError::PermissionDenied` | OS permission failure mapping | No trust decision ownership |
| `PlatformError::NotFound` | OS path absence mapping | No workspace identity ownership |
| `PlatformError::Encoding` | OS/text decoding failure mapping | No editor text-model ownership |
| `PlatformError::SymlinkLoop` | OS path-resolution failure mapping | No workspace tree authority |
| `PlatformError::PathTooLong` | OS path-length failure mapping | No editor large-file policy ownership |
| `PlatformError::AtomicReplaceUnsupported` | OS filesystem atomic-write capability mapping | No save lifecycle ownership |
| `PlatformError::WatcherOverflow` | OS watcher overflow signal | No recovery policy ownership beyond surfacing the signal |
| `PlatformError::ProcessSpawnFailure` | OS process spawn failure mapping | No terminal command authorization ownership |
| `PlatformError::PtyUnavailable` | OS PTY backend availability mapping | No terminal session policy ownership |
| `PlatformError::Timeout` | OS/process timeout reporting | No request-routing ownership |
| `PlatformError::Cancelled` | Caller cancellation propagation | No application orchestration ownership |
| `PlatformError::Io` | Generic OS I/O fallback | No domain-level mutation ownership |
| `PathNormalizationService` | Path normalization, canonicalization, and base containment primitives | Workspace owns path authorization and identity |
| `FileSystemService` | Read/write/list/hash filesystem operations | Workspace owns VFS authority and save decisions |
| `ProcessService` | Execute OS process requests | Security and terminal layers own command policy |
| `PtyService` | Spawn PTY backends | Terminal subsystem owns transcript/session semantics |
| `WatcherService` | Produce watcher snapshots/events from OS paths | Workspace actor owns tree refresh and bounded recovery |
| `EnvironmentService` | Read and normalize process environment variables | Runtime/application layers own launch intent |
| `TimeService` | Wall-clock milliseconds, sleep, and deadline comparison | Runtime/application layers own scheduling policy |
| `ProcessRequest` | OS process launch DTO | Not a command-palette or request-router DTO |
| `ProcessRequest::new` | Convenience constructor for OS process launch DTO | No trust inference or routing ownership |
| `ProcessResult` | OS process completion output DTO | Terminal/application owns interpretation |
| `PtyRequest` | OS PTY launch DTO | Terminal policy owns authorization |
| `PtySession` | OS PTY session descriptor | Terminal subsystem owns user-facing session model |
| `NativeFileSystem` | Native filesystem adapter | Workspace owns file identity, trust, tree, and save orchestration |
| `NativeProcessService` | Native process adapter | Security owns allow/deny decisions |
| `NativeWatcherService` | Native watcher snapshot adapter | Workspace owns overflow recovery and tree state |
| `NativePtyService` | Native PTY adapter placeholder | Terminal subsystem owns shell/session UX |
| `NativeEnvironmentService` | Native environment adapter | Runtime/application owns environment policy |
| `NativeTimeService` | Native time adapter | Runtime/application owns scheduling and timeout policy |
| `shell_title` | Legacy static spike label only | Window ownership is not platform-owned; future shell/window state belongs to UI/application projections |

## Boundary decision

The platform boundary is accepted. Editor state, window state, domain models, and request routing are explicitly not platform-owned.
