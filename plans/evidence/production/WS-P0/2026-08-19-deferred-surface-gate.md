# A gate for surfaces that were frozen by agreement only

Date: 2026-08-19
Task: P9.F3.T4 — keep remote, collaboration and extension-host surfaces out of GA
unless explicitly pulled forward with evidence
Gate: `cargo run -p xtask -- deferred-surfaces`
Config: `xtask/deferred-surfaces.toml`

## The rule, and why it needed teeth

ADR-0046 freezes surface expansion until PR-UI-001 reaches "product workflow
validated", and keeps three gates deferred by name: PR-VSC-002 (isolated
extension host), PR-ENT-001 (remote development UX), PR-ENT-002 (collaboration
and admin controls). P9.F3.T4 states the accompanying rule — each surface needs
its own ADR, policy, tests and product evidence before its readiness status
changes.

Nothing enforced it. The readiness ledger is a markdown table. Promoting a
frozen surface from "Deferred" to "Product workflow validated" was a one-cell
edit in a documentation file, after which the row would read as though four
artifacts existed that did not. The rule was real; the enforcement was whoever
happened to review the diff.

## What the gate checks

For each frozen surface: if the ledger row still says Deferred / Not started /
Blocked, nothing is required — that is the honest resting state. The obligation
attaches the moment the row claims anything else, and then two conditions apply.

**The freeze must be lifted.** PR-UI-001 must read "Product workflow validated".

**All four artifacts must exist**, each named explicitly in the config: the
surface's own ADR, a policy, a test target, and product evidence.

A configured surface with no ledger row is an error rather than a pass. Deleting
the row would otherwise be an escape from the gate — a louder version of exactly
the edit it exists to prevent.

## The first version of this gate was vacuous, and the mutation caught it

The initial config pointed every surface's `adr` at ADR-0046 itself and every
`policy` at the shared `legion-security/src/policy.rs`. Both exist. So did the
test directories and, for remote, the evidence directory — with the result that
promoting PR-ENT-001 **passed**.

That is the failure this repository keeps producing and keeps catching: a check
that reports success because it is asking a question whose answer is already yes.
The mutation is what exposed it. Promoting the row and watching the gate wave it
through is a five-second experiment that turns a plausible gate into a measured
one.

Two things were wrong. The rule says each surface needs **its own** ADR, and
ADR-0046 is the freeze shared by all three — so the config now names
ADR-0025 (remote transport), ADR-0045 (collaboration operation layer) and
ADR-0050 (extension host, which does not exist yet, which is the point).

The number needed care. Review caught the first attempt naming
`ADR-0047-isolated-extension-host.md`, and ADR-0047 is already taken by
extension *distribution* — a different topic. Pointing at a path that does not
exist is deliberate here, since the artifact is exactly what the freeze is
waiting for; pointing at a number already spoken for would have misdirected
whoever writes the real one. 0050 is the next free slot.

More importantly, an artifacts-only check was the wrong shape. Every frozen
surface already has its own ADR — remote has four — so artifacts alone would
have permitted a promotion ADR-0046 forbids outright. **The freeze is checked
first**, and it is the condition that actually bites today.

## Verification

```
cargo run -p xtask -- deferred-surfaces
deferred-surfaces: 3 frozen surface(s) deferred or fully evidenced
```

Against a ledger with PR-ENT-001 promoted:

```
deferred-surfaces: 1 surface(s) claim readiness their artifacts do not support:
  PR-ENT-001 is "Product workflow validated" but is missing:
      the ADR-0046 freeze is still in force: PR-UI-001 is not "Product workflow validated"
exit 1
```

Three unit tests cover the ledger reader, which is the part that could silently
weaken the gate. The important one is that the **status cell** is read rather
than the whole line: PR-VSC-002's acceptance text contains the word "validated"
while the row is deferred, so a line-wide search would promote it by accident —
the precise opposite of the gate's purpose.

Wired into `legion-gates.yml` beside `intent-reachability`, and documented in
`docs/OPERATOR_RUNBOOK.md` under "Deferred surfaces and what unfreezing costs".

## What this means for the rest of the roadmap

The freeze is active: PR-UI-001 reads "Substrate validated" today. That makes
three roadmap tasks unreachable without amending an accepted ADR, and this gate
now says so mechanically rather than relying on someone remembering:

| Task | Why it is blocked |
| --- | --- |
| P9.F3.T2 activate LAN/remote transport | ADR-0046 clause 2 names `legion-remote-transport` as activation-gated |
| P9.F3.T3 productize Cloud Lane | needs a `legion-cloud` crate; clause 1 forbids new workspace crates |
| P7.F1.\*, P7.F2.\* extension host | clause 3 keeps PR-VSC-002 deferred; undeferring requires an ADR amendment |

Unfreezing is a decision, not an implementation task. It belongs to the owner.
