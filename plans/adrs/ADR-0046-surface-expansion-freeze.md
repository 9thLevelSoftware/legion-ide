# ADR-0046: Surface Expansion Freeze Until Manual Mode Daily-Driver

## Status

Accepted; amended 2026-08-19 (see [Amendment 1](#amendment-1-2026-08-19--pr-ui-001-promoted-pr-ent-002-undeferred)).

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

## Amendment 1 (2026-08-19) — PR-UI-001 promoted, PR-ENT-002 undeferred

Clause 3 requires "an amendment to this ADR with justification" to undefer a
gate. This is that amendment. It is recorded rather than assumed because the
original decision was written to make exactly this step deliberate.

**Clause 2 is waived for two named tasks, not satisfied.** This is the part
worth reading carefully, because the tempting version of this amendment is
wrong. The owner signed off the *smoke promotion clock* (P0.F4.T5) on
2026-08-19, and that is a real milestone — but it is not the same as `PR-UI-001`
reaching *Product workflow validated*. The readiness ledger states that row's
own promotion bar: "current native platform accessibility/focus evidence across
supported OSes", which does not exist yet. `PR-UI-001` therefore stays at
*Substrate validated*, and the unfreeze criterion this ADR named has **not**
been met on its own terms.

What lifts the freeze here is a direct owner decision to proceed anyway, for two
named tasks. Recorded as given:

> "5) Just turn it on now and be done with it. 6) We should just do this one
> too."

That is sufficient — the owner may amend their own freeze — but it is a
different and weaker basis than the criterion firing, and conflating the two
would leave a future reader believing the accessibility evidence exists.

**Clause 3 is amended** to undefer `PR-ENT-002`, scoped to P9.F3.T2 and
P9.F3.T3. `PR-VSC-002` and `PR-ENT-001` stay deferred; nothing here touches
them.

**Clause 1 gets one narrow exception**, for `legion-cloud` only — which in
the event went **unused**: P9.F3.T3 was built inside `legion-app` and
`legion-desktop`, where the Cloud Lane substrate already lived, so no new
workspace crate was added and clause 1 stands intact. The exception is recorded
rather than deleted because it was granted; a future reader should know it is
available and that nothing has spent it. The exception is deliberately not a general
re-opening: any *other* new crate still needs its own amendment.

**What has not changed.** Clause 4 already permitted maintenance on existing
crates and still does. The remaining frozen surfaces stay frozen. And the
readiness rows do **not** move on the strength of this amendment: promoting
`PR-ENT-001` or `PR-ENT-002` in the ledger still requires the four artifacts the
`deferred-surfaces` gate enforces — ADR, policy, tests, product evidence — for
each surface. Permission to build is not evidence of having built.

Two caveats carried forward rather than dropped:

- The four green smoke runs behind the P0.F4.T5 sign-off are thinner than the
  count suggests: three hand-dispatched, two sharing a SHA, all inside ~37
  hours. The owner accepted that knowingly. A later reader wanting a stronger
  basis should look for scheduled runs on distinct SHAs after 2026-08-24.
- The core editor is not yet demonstrated daily-drivable across three OSes,
  which was the condition this freeze existed to wait for. Building these two
  surfaces before that is a deliberate owner trade, and the risk the freeze was
  written to prevent — scope widening ahead of the core — is accepted rather
  than eliminated.

## References

- Master Plan v0.2 section 5.3 (scope freeze guidance)
- Product Readiness Ledger (PR-UI-001: still Substrate validated; PR-ENT-002:
  undeferred by Amendment 1, but not promoted — see the amendment)
- Course Correction Plan W6 finding
