# Phase 6 Plan Critique -- Packaging, Platform Integration, and Accessibility

## Verdict

PASS after auto-refine.

## Rule-Chain Trace

| Plan | Verdict | Rule trace |
| --- | --- | --- |
| 06-01 | OK | Required frontmatter present; no files_modified/files_forbidden overlap; governance-first ordering prevents collision with accepted legacy collaboration Phase 6 evidence. |
| 06-02 | OK | Required frontmatter present; package work is isolated to desktop helpers, tests, script, and runbook; manifests are forbidden to prevent unapproved dependencies. |
| 06-03 | OK | Required frontmatter present; platform/accessibility smoke model is isolated to `devil-desktop` and explicitly distinguishes OS-observed facts from adapter-path/model evidence. |
| 06-04 | OK | Required frontmatter present; session and diagnostics work is metadata-only and blocks on raw-payload persistence. |
| 06-05 | OK | Required frontmatter present; smoke scripts/CI/CLI evidence work is sequenced after package and diagnostics contracts and forbids desktop implementation edits. |
| 06-06 | OK | Required frontmatter present; evidence capture is documentation-only and cannot modify code, scripts, CI, manifests, or legacy evidence. |
| 06-07 | OK | Required frontmatter present; final acceptance is blocked unless required evidence and full repository gates pass. |

## Critique Findings And Refinements

1. Legacy Phase 6 evidence collision:
   - Finding: `xtask/src/main.rs` validates accepted legacy Phase 6 collaboration evidence, while the GUI roadmap Phase 6 is packaging/platform/accessibility.
   - Refinement: Plan 06-01 adds a distinct GUI Phase 6 evidence path and forbids edits under `plans/evidence/phase-6/`.

2. Installer dependency risk:
   - Finding: "Installer" scope could lead to a new packaging dependency before policy approval.
   - Refinement: Plan 06-02 defines a packaged executable directory as the first acceptance target and blocks new dependencies or manifest edits.

3. Accessibility overclaim risk:
   - Finding: Existing smoke reports `accessibility_smoke: not observed`, so final acceptance could accidentally overstate platform proof.
   - Refinement: Plan 06-03 requires deterministic `accessibility_tree_smoke` model evidence and keeps OS accessibility observation separate.

4. Session crash-safety gap:
   - Finding: Current `DesktopSessionStore::save` writes directly to the final session path.
   - Refinement: Plan 06-04 requires write-to-temp, validate, reject raw markers, and replace semantics with tests for last-good preservation.

5. Diagnostics export privacy risk:
   - Finding: Diagnostics export could accidentally persist raw buffer text, terminal output, provider payloads, or secrets.
   - Refinement: Plan 06-04 adds raw-marker rejection and metadata-only fields, and Plan 06-06 evidence must prove the behavior.

6. CI/native-window mismatch:
   - Finding: The existing OS matrix cannot be assumed to support interactive native GUI windows.
   - Refinement: Plan 06-05 uses non-interactive dry-run CI coverage and Plan 06-06 documents manual Windows smoke separately from headless CI.

7. Final acceptance false-positive risk:
   - Finding: Phase 6 could be marked complete from generated docs alone.
   - Refinement: Plan 06-07 blocks acceptance on plan results, required artifacts, `xtask`, CLI evidence check, and full cargo/deny gates.

## Parallelization Decision

The phase remains sequential across seven waves. Governance, packaging, platform smoke, session/diagnostics, scripts/CI, evidence capture, and acceptance all build on exact outputs from the prior wave. Parallel same-wave execution would create avoidable conflicts in shared files such as `crates/devil-desktop/src/workflow.rs`, `crates/devil-desktop/src/smoke.rs`, `crates/devil-cli/src/main.rs`, `.github/workflows/ci.yml`, and the main GUI Phase 6 evidence file.

## Residual Risks For Build

- Real OS accessibility inspection may still be limited by egui/eframe and the local environment. The plan requires a deterministic accessibility tree model and honest OS-observed labels, not unsupported claims.
- The Windows package path starts as a packaged executable directory, not a signed installer. A signed installer remains a follow-up unless explicitly added with policy and evidence.
- Native window smoke can be blocked on headless hosts. Build agents must record blocked status with exact environment evidence.
- Full workspace gates can be expensive; final acceptance remains blocked unless required gates pass or the project owner changes the gate.
