# GAP scope audit and refinement

Date: 2026-09-05  
Baseline: `65381ed`  

The stable `COMP-SCOPE-GAP-01..10` identifiers are retained and rewritten as
explicit cross-cutting product outcomes. Their source bindings now point to the
P0 sequence, the approved completion/qualification plans, and bounded current
source or evidence. Dependencies name finite product prerequisites; they do not
replace the protected-product evidence obligations and do not point through
protected edges.

`COMP-GAP-001` is the only new row. It closes the missing finite outcome for a
packaged native GUI install/edit/save journey, real window input, external disk
effects, the four-green promotion clock, blocked native configurations, and the
immutable installed-preview journal. It is `absent`, `owner_role: luna_worker`,
and `acceptance: unassessed`.

The ten retained outcomes are:

- GAP-01: packaged installed-product GUI and promotion, depending on the
  platform matrix/input, clean install, and existing dispatched-run prerequisite.
- GAP-02: release signing/trust, depending on the distribution matrix/artifact,
  signer, and clean-machine outcomes.
- GAP-03: signed update/rollback, depending on channel, update, and P8-F2 rows.
- GAP-04: packaged work preservation/recovery, depending on PRES recovery and
  native journey rows plus diagnostics.
- GAP-05: native accessibility, depending on the platform matrix, native
  accessibility/qualification, and P8-F5 evidence rows.
- GAP-06: Manual/offline zero-egress, depending on the distribution artifact,
  training Manual rule, existing Manual controls, and trust qualification.
- GAP-07: internal repository governance, retaining required/unassessed status
  and protecting installed-product truth and signing/trust (GAP-01/GAP-02).
- GAP-08: internal documentation and claim truth, retaining required/unassessed
  status and protecting distribution support/docs, installed-product truth, and
  signing/trust (DIST-009/GAP-01/GAP-02).
- GAP-09: measured product performance, depending on the platform matrix,
  calibrated platform budgets, and P8-F4 workload gates.
- GAP-10: support/privacy/legal distribution, depending on diagnostics,
  support documentation, license/notices, and P8-F3 consent/export rows.

No dependency is added from a row to an internal row that already protects it.
GAP-01 remains protected by `COMP-P0-F4-T2-1`, GAP-02 remains protected by the
P0/P8 signing rows and `COMP-SCOPE-FAMILY-17-04`, and internal GAP-07/GAP-08
protect the concrete product outcomes named above. The retained
`COMP-P0-F4-T1-1` protection was explicitly rewired from internal GAP-08 to
`COMP-DIST-009`, GAP-01, and GAP-02.
Replacing any of those GAP IDs later requires an explicit source-mapped
protected-field rewire.

Legal/support coverage is bounded to `docs/PRIVACY.md`,
`THIRD_PARTY_NOTICES.md`, `COMP-P0-F5-T2-1`, `COMP-P0-F5-T3-1`, diagnostics,
and the distribution support row. The P0 Help/About requirement remains part of
GAP-10's acceptance boundary and is now source-bound to
`crates/legion-desktop/src/view.rs` as the desktop support-bundle export
entrypoint; this source binding is not acceptance proof.
