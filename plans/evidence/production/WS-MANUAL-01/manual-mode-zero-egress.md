# Manual Mode Zero-Egress Smoke

Date: 2026-06-19

## Contract

Manual mode can open, edit, save, and search a trusted local workspace without hosted provider dispatch, agent context retrieval, autonomous writes, telemetry export, or network target records.

## Verification Command

`cargo test -p legion-app --test manual_zero_egress`

Companion renderer-model check:

`cargo test -p legion-desktop --test manual_renderer_evidence manual_renderer_evidence_names_zero_egress_trust_boundary -- --exact`

## Evidence Rules

- The test must operate through `AppComposition` and `CommandDispatchIntent`, not direct buffer mutation.
- The test must assert Manual product mode.
- The app-level test must assert no assisted-AI, inline-prediction, delegated runtime, tool-permission, agent context, or hosted provider activity is created by the Manual open/edit/save/search path. Manual save may project local pending human-review metadata for the proposal it creates; that review metadata must not activate delegated runtime work.
- The desktop renderer evidence test must assert Manual trust-boundary rows that name no provider dispatch and no agent context.
- This smoke does not prove OS-level network denial. A later sandbox/firewall packet capture may strengthen the row, but this test is the required app-level regression guard for WS-MANUAL-01.
