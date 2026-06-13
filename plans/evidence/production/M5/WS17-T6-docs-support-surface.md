# M5 — WS17.T6 Docs & Support Surface Evidence

## Status

Verified.

## Acceptance target

- User-facing docs cover the four GP paths currently exposed by the product surface: Manual, Assist, Delegate, and Legion Workflows.
- The keyboard reference, troubleshooting guide, and operator runbook are linked from the canonical documentation index.
- The bug-report template requires the diagnostics bundle fields needed for support triage.
- The product-readiness ledger still keeps PR-REL-001 in progress until signed-installer or explicit unsigned-beta evidence exists.

## What was verified

- `docs/INDEX.md`
  - Acts as the canonical docs map.
  - Points end users, operators, keyboard readers, and support responders at the right document.
- `docs/USER_GUIDE.md`
  - Covers Manual, Assist, Delegate, and Legion Workflows as the four product paths.
  - Routes packaging/release readers to the operator runbook and support readers to troubleshooting.
- `docs/KEYBOARD_REFERENCE.md`
  - Lists the current projected shortcut labels and mode controls surfaced by the product UI.
- `docs/TROUBLESHOOTING.md`
  - Documents the standard support artifacts and explicitly points bug reporters at `.github/ISSUE_TEMPLATE/bug_report.md`.
- `.github/ISSUE_TEMPLATE/bug_report.md`
  - Requires the diagnostics bundle fields: evidence file, session-state file, diagnostics export, and package manifest when relevant.
- `docs/OPERATOR_RUNBOOK.md`
  - Lists the package/build commands and the expected support artifacts that close the GA release runbook.
- `plans/product-readiness-ledger.md`
  - PR-REL-001 remains `In progress`.
  - The release posture remains gated on signed-installer or explicitly unsigned-beta evidence; this docs task does not move that gate.
- `cargo run -p xtask -- docs-hygiene`
  - Passed.

## Notes

- This task is documentation and support-surface only.
- No release posture or product-readiness status was changed.
- The support bundle path is now documented end-to-end without relying on raw logs or uncaptured screenshots.
