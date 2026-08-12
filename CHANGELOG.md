# Changelog

## Unreleased

- `legion-observability::proposal_created_event` retains its deprecated
  `(proposal, causality_id, sequence)` signature for one compatibility release.
  New callers should use `proposal_created_event_with_transition` to preserve
  transition timestamps and diagnostics.
