# P9.F3.T2 remote transport reconnect/offline evidence

**Date:** 2026-08-19 (supersedes the 2026-06-15 note; see *What the earlier note
claimed* below)
**Task:** P9.F3.T2 — activate LAN/reference remote transport with
reconnect/offline evidence
**Authorisation:** ADR-0046 Amendment 1 (owner decision, 2026-08-19)
**Acceptance:** "Reconnect and offline behaviors are recorded as evidence."
**Stop condition:** "Stop if reconnect logic is untested under a forced network
drop."

## The defect found on the way in

`legion-remote-transport` looked finished. It has a handshake, flow control, a
replay window, checkpoints, resume tokens, offline manifests, an mTLS carrier,
and a `RemoteTransportLifecycleState` enum with a `Reconnecting` variant.

Nothing ever entered `Reconnecting`.

Nine assignments to `self.state` existed in the state machine and none of them
represented losing the connection:

```
Handshaking, Active, Backpressured (x2), Active, Active, Resuming, Active
```

Two things followed, both of which read as working code:

1. **`begin_resume` accepted from any state.** A transport that had never been
   disconnected could "resume" — a second path to `Active` with weaker checks
   than the handshake. Resume is recovery; there was nothing to recover from.
2. **`reconnect_attempts` was `matches!(state, Reconnecting | Resuming) as u32`.**
   A state flag typed as a counter. It could only ever be 0 or 1, it was 0 the
   moment a session recovered, and since `Reconnecting` was unreachable the only
   way to see 1 was to catch it mid-resume. A session that had dropped twenty
   times reported zero.

So the stop condition could not be satisfied as written. Reconnect logic was not
untested — it did not exist. Only a resume path that assumed you had never been
gone.

## What the earlier note claimed

The 2026-06-15 version of this file recorded the acceptance as met, citing
`begin_reconnect()` / `complete_reconnect()` on the session runtime and
concluding that "remote transport reconnect/offline behavior is now backed by
explicit runtime test coverage".

Those methods are real and the test it cited passes. But `complete_reconnect`'s
own doc comment says it completes "after identity, cache, and version
preconditions are externally validated", and nothing external validated them:
any caller could move a session from `Reconnecting` straight back to `Active`
with no token, no manifest, and no checkpoint. The note described a state
transition and called it a recovery mechanism.

It also did not mention the transport state machine at all — the layer that
holds the actual checks — because that layer had zero fan-in and no product path
could reach it.

Recorded here rather than quietly overwritten: the claim was too strong, and the
gap it hid is the substance of this task.

## What changed

**A drop transition** — `RemoteTransportStateMachine::mark_network_drop(reason)`,
`Active`/`Backpressured`/`Reconnecting` → `Reconnecting`. What survives is
deliberate: accepted operations, the replay window, the last checkpoint and the
resume digest are kept, because they are what resume replays against. In-flight
(unacked) operations are cleared — they were on the wire when it went away and
no peer will ever drain them.

**A real counter** — `reconnect_attempts` is a `u32` field incremented per drop
and surviving recovery, not a state flag.

**Resume requires a drop** — `begin_resume` refuses unless the transport is
`Reconnecting`.

**Activation** — `crates/legion-remote/src/transport.rs` binds a
`RemoteSessionRuntime` to the state machine. `legion-remote` is already a
product dependency (`legion-app`, `legion-desktop`); `legion-remote-transport`
had none. A drop now moves both layers together, and a resume must satisfy the
transport's token and manifest checks before the session is allowed back to
`Active`. "Externally validated" finally names something.

The session-mismatch guards exist because a transport bound to one session and
driven with another would report health for a session nobody is watching.

## Forced-drop coverage

`crates/legion-remote/tests/transport_reconnect_offline.rs`, 9 tests:

| Test | What it pins |
| --- | --- |
| `a_forced_drop_moves_both_the_session_and_the_transport` | Both layers move. A session reporting `Active` over a reconnecting transport tells the user they are connected while every frame is refused. |
| `a_dropped_transport_refuses_frames_until_it_resumes` | Offline behaviour: frames are refused while disconnected. |
| `a_drop_clears_in_flight_frames_but_keeps_the_replay_window` | Queue depth drops to zero; accepted operations, sequence and checkpoint survive. |
| `resume_after_a_drop_restores_both_layers` | Token + manifest → both layers `Active`, frames accepted again. |
| `a_resume_manifest_missing_the_checkpoint_leaves_the_session_reconnecting` | A refused resume does **not** advance the session. |
| `resume_without_a_drop_is_refused` | The second-path-to-`Active` hole is closed. |
| `reconnect_attempts_counts_every_drop_rather_than_the_current_state` | Three drop/resume rounds report 3, not 0 or 1. |
| `a_transport_refuses_a_session_it_was_not_activated_for` | Binding is enforced per call. |
| `activation_refuses_a_handshake_for_a_different_session` | Binding is enforced at activation. |

"Forced" means the drop is injected at the layer that owns the connection and
the state machine must cope. `RemoteSessionTransport` drives metadata and opens
no sockets — a drop is *reported to* it, because the caller holding the
connection is the only layer that can know. That boundary is what the rest of
`legion-remote` keeps.

## Mutation testing

Each guard was broken, the suite run, and the source restored. `git status` was
clean afterwards.

| # | Mutation | Result |
| --- | --- | --- |
| M1 | drop does not set `Reconnecting` | KILLED (5 tests) |
| M2 | `reconnect_attempts` never increments | KILLED |
| M3 | `begin_resume` no longer requires `Reconnecting` | KILLED |
| M4 | drop does not clear in-flight operations | **SURVIVED, then KILLED** |
| M5 | session is not moved on drop | KILLED (4 tests) |
| M6 | session completes reconnect before the transport validates | KILLED (2 tests) |
| M7 | transport does not check session binding | KILLED |
| M8 | health reported as passed-in rather than session-derived | KILLED |

M4 is the one worth reading. Nothing tested that a drop clears in-flight
operations, although `mark_network_drop`'s doc comment asserted it *and*
justified it — an unenforced claim in a comment, which is the same failure mode
as the 2026-06-15 note above at a smaller scale.
`a_drop_clears_in_flight_frames_but_keeps_the_replay_window` was added and the
mutation re-run: KILLED.

One further correction: M1's first run reported SURVIVED. That was the harness,
not the guard — the source is CRLF and the mutation pattern used `\n`, so the
replacement silently matched nothing and the "mutant" was the original file. A
mutation that does not mutate looks exactly like a guard that holds. Re-run with
CRLF normalisation: KILLED, 5 tests.

## Verification

```
cargo test -p legion-remote --test transport_reconnect_offline   # 9 passed
cargo test -p legion-remote-transport                            # 22 passed
cargo test --workspace                                           # 325 suites ok
cargo clippy -p legion-remote -p legion-remote-transport --all-targets -- -D warnings
cargo fmt --all
```

## What this is not

This is reconnect and offline **transport** behaviour, activated and tested. It
is not remote development product UX: there is no SSH or devcontainer product
path, no remote terminal/LSP/filesystem product workflow, and no desktop surface
for connection health.

`PR-ENT-001` and `PR-ENT-002` therefore stay at *Deferred with explicit cut
line* in the readiness ledger. ADR-0046 Amendment 1 grants permission to build
these two surfaces; permission is not evidence of having built them, and the
`deferred-surfaces` gate still requires ADR, policy, tests and product evidence
per surface before either row moves.
