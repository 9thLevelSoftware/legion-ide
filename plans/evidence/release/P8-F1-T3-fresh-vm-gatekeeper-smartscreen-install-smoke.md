# P8.F1.T3 — Fresh-VM Gatekeeper/SmartScreen/Install Smoke Evidence

## Status

Archived checkpoint.

## Scope

This evidence file records the current release-pipeline checkpoint used by PR-REL-001 readiness review:

- release descriptor: `target/release-pipeline/version_stamp.toml`
- descriptor timestamp: `2026-06-14T00:17:07Z`
- package: `legion-desktop`
- channel: `stable`
- git SHA: `5e2824238bc9c770f986d74f794a8783832c3ffc`

## What was archived

- The release descriptor on disk is newer than the earlier WS17.T2 unsigned-beta evidence note.
- The readiness ledger now cites this file as the current release evidence checkpoint for PR-REL-001.
- The repository still does not store signing keys, notarization tokens, or other credential-bearing release material.

## Notes

- This archive is limited to repository evidence and descriptor freshness.
- Signed-installer / fresh-VM verification remains a separate manual release-step requirement until a later gate records the full install proof.
- The evidence directory for this task is now present at `plans/evidence/release/`.
