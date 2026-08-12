# Phase 6 Future Surface Deferral Audit

## Status

Phase 6 collaboration activates only the local operation-log collaboration runtime, protocol DTOs, metadata audit/replay records, security policy controls, storage/observability metadata paths, editor transaction-source bridging, and UI presence projections. It does not activate Phase 7 remote development, terminal/process execution, hosted telemetry, cloud providers, raw source retention, or direct workspace mutation.

## Deferred Surfaces

- Remote workspace authority remains deferred.
- Terminal and process execution remain deferred.
- Hosted telemetry and hosted model/provider egress remain deferred.
- Raw collaboration transcripts and full source snapshots are not durable defaults.
- Collaboration does not grant filesystem, project, editor, app, UI, storage, or remote internals ownership.
- Collaboration durable file effects remain proposal-mediated through existing workspace/save preconditions.

## Enforcement Notes

- `legion-collaboration` has no dependency on app/UI/editor/project/remote/terminal internals.
- `legion-security` denies collaboration runtime sessions by default and denies non-loopback collaboration transport egress under air-gap policy.
- `legion-storage` and `legion-observability` reject raw-source or raw-transcript collaboration audit markers.
- `legion-ui` collaboration commands emit intents only and preserve projections.
- `legion-app` routes collaboration command intents to explicit app-owned requests, owns deterministic local session composition, and still does not grant UI, collaboration runtime, remote, terminal, process, or direct workspace authority.
