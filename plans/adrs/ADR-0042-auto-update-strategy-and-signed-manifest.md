# ADR-0042: Auto-Update Strategy and Signed Manifest Format

## Status

Accepted — WS17.T3 design decision for the release/rollback surface.

## Context

WS17.T3 needs an updater that can support staged rollout, rollback to the previous accepted build, and integrity checks without weakening the existing release-pipeline boundary. The repository already has a deterministic cargo-dist-based packaging scaffold in `xtask/release-pipeline.example.toml`, but that scaffold is only responsible for producing installer artifacts and version stamps.

The updater itself must remain a separate release-control layer. It should consume published release metadata, not infer policy from installer naming alone, and it must preserve the current fail-closed posture: no update promotion without an explicit manifest, no hidden trust in package contents, and no private signing material in the repository.

## Decision

Use a custom, Zed-style updater instead of Velopack.

The updater will:

- consume a small signed release manifest published alongside installer artifacts;
- use the manifest to decide whether a client is on the stable or preview channel;
- support staged rollout and rollback by switching manifest pointers, not by rewriting installer metadata in place;
- keep installer production in the existing cargo-dist pipeline while the updater remains a separate layer over those artifacts.

## Signed manifest format

Use a TOML manifest with a detached Ed25519 signature.

Recommended shape:

- manifest file: `release-manifest.v1.toml`
- signature file: `release-manifest.v1.toml.sig`
- signature algorithm: Ed25519

The manifest records release control data only, including:

- package name
- channel
- version
- rollout policy
- previous accepted version / rollback pointer
- artifact identifiers or URLs
- artifact SHA-256 digests
- issuance timestamp
- signer reference, not signer secrets

The manifest is a control document, not a general package index. It must stay narrow so that updater logic remains auditable and the signed surface is easy to verify.

## Why this decision

1. The repository already uses TOML for release-pipeline descriptors and version stamps, so TOML keeps the release metadata stack consistent.
2. A detached Ed25519 signature is simple to verify, portable, and fits the repository policy of keeping private signing material out of the tree.
3. A custom Zed-style updater keeps the release-control logic explicit and aligned with the existing plan/build/host split instead of tying Legion to a third-party updater’s opinionated update model.
4. Rollback semantics are easier to reason about when the manifest points to the previous accepted release directly.

## Consequences

- `cargo-dist` remains the artifact builder; it is not the updater.
- Future implementation work must publish the signed manifest as part of the release process before any client-side update activation can ship.
- The updater implementation can stay narrowly scoped to channel selection, staged rollout, download verification, and rollback selection.
- Any future update-client code must verify the detached Ed25519 signature before trusting the manifest body.
- The release-pipeline example config must record the updater strategy and manifest format so the chosen design is visible at the pipeline boundary.
