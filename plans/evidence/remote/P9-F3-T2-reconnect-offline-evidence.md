# P9.F3.T2 remote transport reconnect/offline evidence

Date: 2026-06-15

## Scope

This note records evidence for the Legion remote transport path in `crates/legion-remote/src/lib.rs` and the HTTP cloud-lane integration tests in `crates/legion-remote/tests/cloud_lane_http_transport.rs`.

## Evidence recorded

- Reconnect flow is explicit in the runtime:
  - `RemoteRuntime::begin_reconnect()` transitions the session to `Reconnecting` and marks network health `Disconnected`.
  - `RemoteRuntime::complete_reconnect()` requires the reconnecting state before it restores healthy network state.
- Offline resume flow is explicit in the runtime:
  - `RemoteRuntime::mark_offline()` marks the session offline.
  - `RemoteRuntime::offline_resume_manifest(...)` produces a metadata-only manifest.
  - `RemoteRuntime::handle_transport_envelope(...)` accepts the offline-resume envelope path.
- Transport-level HTTP coverage remains active through the integration suite in `crates/legion-remote/tests/cloud_lane_http_transport.rs`.

## Validation run

- `cargo test -p legion-remote remote_reconnect_and_offline_resume_are_explicit -- --nocapture` — passed.
- `cargo test -p legion-remote --test cloud_lane_http_transport -- --nocapture` — passed.

## Acceptance note

The remote transport reconnect/offline behavior is now backed by explicit runtime test coverage and a durable evidence note under `plans/evidence/remote/`.
