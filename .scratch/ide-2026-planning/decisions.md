# Legion 2026 program design decisions

Status: interview complete; scope confirmed; planning only. This record feeds the
program specification and dependency-linked implementation tickets. It is not
a replacement for the canonical completion records or product-readiness ledger.

## Inputs and preserved decisions

- [Research](../../plans/evidence/2026-ide-demand-and-technology-research.md)
- [Frontier plan and confirmed Q1–Q18](../../.omp/plans/FRONTIER_INTEGRATION_PLAN.md)
- [Domain glossary](../../GLOSSARY.md)
- [Product-readiness ledger](../../plans/product-readiness-ledger.md)

Q1–Q18 remain settled and are incorporated by reference. Newly researched
candidates are not automatically shipping commitments. Preserve existing work
and verify implementation before creating duplicate work packages.

## Confirmed decisions

| Decision | Resolution |
| --- | --- |
| Q19 — Program scope | Produce one comprehensive program plan combining the existing frontier work and new research candidates. Assign every candidate an explicit required-delivery, bounded-experiment, or deferred disposition with rationale. |
| Q20 — First release target | First target an internal daily-driver pilot demonstrating real edit/test/debug/Git/change-review workflows. External releases have separate gates; existing enterprise obligations remain, and pilot success does not imply production readiness. |
| Q21 — Sequencing | Prioritize obstacles to using existing projects, including debugger and required-extension compatibility, over advanced AI features. Keep all approved frontier work in the program. |
| Q22 — Planning constraints | Plan evidence-based milestones without an assumed deadline or fixed staffing. Establish the dependency graph and external prerequisites before estimating schedule and staffing. |
| Q23 — Pilot coverage | First acceptance slice is Windows and Rust using Legion's own repository. Expand through required languages and operating systems without dropping full-program obligations. |
| Q24 — ACP interoperability | Genuine ACP interoperability is required program work, extending the existing command bridge. Qualify named agents and capabilities; preserve proposal review and workspace authority, and reject unsupported authority mappings. |
| Q25 — Prediction delivery | Useful real-provider next-edit assistance is required program work. Reuse existing UI/lifecycle and qualify an existing provider/model against latency, retained edits, rejection and undo. Custom model training remains a bounded experiment. |
| Q26 — Session recovery | Recoverable agent work after restart is required. Interrupted execution remains stopped until explicitly resumed; retained context respects consent. Continuing execution after IDE exit is a separate deferred capability. |
| Q27 — Migration scope | Support a documented subset of settings/keybindings and a named essential-extension set with demonstrated compatibility. Report unsupported items explicitly; do not promise universal VS Code parity or waive runtime-extension gates. |
| Q28 — Initial agents | Target Codex and Claude Code for independent ACP qualification of permissions, cancellation, context and proposal review. These are qualification targets, not compatibility promises; failed integrations remain visibly unsupported. |
| Q29 — Prediction activation | Require an explicitly selected provider profile for next-edit assistance. No silent local/hosted/deterministic switching; Manual remains AI-free. Show unavailable providers honestly and measure useful edits against ordinary language completion before qualification. |
| Q30 — Native equivalents | A demonstrated native equivalent may satisfy an essential workflow's pilot migration requirement. Clearly distinguish native capability from extension compatibility; runtime-extension compatibility remains a separate program obligation. |
| Q31 — Interrupted actions | Inspect resulting state and reconcile uncertain command outcomes before retrying actions that could duplicate external effects. Recovery never blindly replays pushes, publication, migrations or other consequential actions. |
| Q32 — Remaining candidates | Source-linked failures/browser traces and portable instructions/skills are required program work, introduced in relevant milestones with provenance/trust controls. Remote environments/team review remain required existing workstreams after the local pilot. Interactive agent stepping/trajectory editing is a bounded experiment. Replacing Git/PR collaboration and autonomous deployment are deferred. |
| Q33 — Pilot acceptance | Require five working days of real Legion development covering declared edit/test/debug/Git/review workflows, with no unresolved data-loss, unauthorized-write or workflow-blocking defects. Record attempts and fallback to other IDEs. The period is observation, not a delivery deadline or sufficient evidence by itself. |
| Q34 — Agent containment | External agents' own filesystem/terminal tools operate within an enforced isolated working copy. Main-workspace changes enter only through Legion proposals; a cooperative protocol adapter alone is insufficient. Unsupported enforcement means an unsupported integration. |
| Q35 — Skill trust | Discovery of instructions/skills is distinct from trust in executable content. Show loaded source/version; new or expanded script/tool/network permissions require explicit authorization. Opening a repository or updating a skill cannot silently authorize execution or expand grants. |
| Q36 — Acceptance milestones | Separate manual daily-driver acceptance from assisted-workflow acceptance. Missing provider credentials/availability must not block evaluating ordinary editing. Both milestones remain required; assisted acceptance demonstrates qualified agents, real predictions, proposal review, cancellation and restart recovery. |

## Remaining specification work

- Reconcile the current code and authoritative completion records with the older frontier plan.
- Convert confirmed decisions into user stories, capability contracts, acceptance scenarios,
  dependency edges and explicit external prerequisites.
- Inventory and name the essential migration workflows/extensions and qualified agent/provider
  configurations; make any unsupported prerequisites visible before acceptance.
- Determine feasible implementation details through source inspection and bounded technical
  investigations. Bring back decisions only if they change confirmed scope or authority.
- Review the proposed behavioral test boundaries during specification synthesis.

## Interview close-out

Q19–Q36 are confirmed, in addition to preserved Q1–Q18. The user confirmed the
shared understanding and authorized specification synthesis. The resulting
[specification](spec.md) preserves these decisions. No implementation tickets or
product changes have been created by this interview.

## Comments

The user accepted Q19–Q22 together with “yes to all.” Implementation, provisioning,
publication and shipping are not authorized by this planning interview.

The user also accepted Q23–Q27 together with “yes to all.” Current-state findings
are recorded separately in [inspection notes](current-state-notes.md); inspection
is not runtime qualification.

The user accepted Q28–Q32 together with “yes to all.” The candidate dispositions
above refine program scope without promoting any capability to product-ready.

The user accepted Q33–Q36 together with “yes to all.”

The user answered “yes” to the shared-understanding summary and proceeding with
`to-spec`. The user then confirmed AppComposition integration tests, native desktop
acceptance and focused process/storage tests. The specification is published in
the local tracker as `ready-for-agent`.
