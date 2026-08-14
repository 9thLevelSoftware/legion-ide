# ADR-0046: Surface Expansion Freeze Until Manual Mode Daily-Driver

## Status

Accepted

## Context

The Legion IDE workspace contains 30 crates. Four of those crates are confirmed
zero-fan-in -- nothing else in the workspace depends on them:

- `legion-remote-transport`
- `legion-retention`
- `legion-telemetry`
- `legion-vscode-compat`

These substrate crates were built ahead of the core editor being daily-drivable.
The core editor gates (PR-UI-001, PR-LANG-001) remain at "substrate validated",
not "product workflow validated". Master Plan v0.2 already identifies this risk
and calls for scope discipline until Manual mode is a boring, excellent IDE.

## Decision

1. **No new workspace member crates** may be added to `Cargo.toml` `[workspace]`
   members.

2. The four zero-fan-in crates (`legion-remote-transport`, `legion-retention`,
   `legion-telemetry`, `legion-vscode-compat`) remain in the workspace but
   **product activation is gated** on PR-UI-001 reaching "product workflow
   validated" in the product-readiness ledger.

3. Deferred gates (PR-VSC-002, PR-ENT-001, PR-ENT-002) stay deferred.
   Undeferring requires an amendment to this ADR with justification.

4. Existing crates may receive bug fixes, test improvements, and internal
   refactors. The freeze applies to **product surface activation**, not
   maintenance.

## Consequences

- Prevents scope creep while the core editor is unproven.
- Zero-fan-in crates preserve their substrate investment.
- Clear unfreeze criteria: PR-UI-001 promotion to "product workflow validated".
- Any exception requires an ADR amendment, creating a decision record.

## References

- Master Plan v0.2 section 5.3 (scope freeze guidance)
- Product Readiness Ledger (PR-UI-001 current status: Substrate validated)
- Course Correction Plan W6 finding
