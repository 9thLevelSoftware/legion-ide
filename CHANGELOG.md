# Changelog

## Unreleased

The workspace package version is `0.1.0`. `packaging/Packager.toml` starts at
`0.0.0`, and `scripts/package-native.sh` accepts only `0.0.N` with `N >= 1`.
Those are not one shipped version yet. This changelog does not assign a
release version.

- `legion-observability::proposal_created_event` retains its deprecated
  `(proposal, causality_id, sequence)` signature for one compatibility release.
  New callers should use `proposal_created_event_with_transition` to preserve
  transition timestamps and diagnostics.
