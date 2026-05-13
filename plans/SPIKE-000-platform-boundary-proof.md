# SPIKE-000: Platform Boundary Proof

## Status

Draft, required before Spike 1A scale.

## Objective

Validate that `devil-platform` is constrained to OS service abstractions and does not absorb editor/window logic or host high-level editor contracts.

## Scope

- Confirm crate responsibilities align with:
  - keychain abstraction,
  - filesystem helpers,
  - process spawning,
  - window abstraction stubs required by shell integration.
- Validate interaction surfaces are consumed through protocol contracts.

## Acceptance Criteria

- `devil-platform` exposes narrowly scoped service traits and data structures.
- `devil-platform` has no feature flags that pull in editor text, tracker, or provider semantics.
- Build/test of a representative boundary slice passes without direct coupling from `devil-ui` editor logic into `devil-platform` internals.
- Architectural review notes map every `devil-platform` API call to an OS concern.

## Required artifacts

- Platform boundary API list in the charter.
- Any necessary split/refactor notes captured before Spike 1A.
- Sign-off that `devil-platform` is not the authority for editor state or model request routing.
