# Devil IDE GUI Productization - Roadmap

## Phases

- [x] Phase 1: Baseline Reconciliation and Renderer Decision
- [x] Phase 2: Renderer-Backed Foundation Mode
- [ ] Phase 3: Daily Editing MVP
- [ ] Phase 4: Language and Terminal IDE Loop
- [ ] Phase 5: Control, Trust, and Assisted AI Surfaces
- [ ] Phase 6: Packaging, Platform Integration, and Accessibility
- [ ] Phase 7: Fully Functional Local IDE Beta
- [ ] Phase 8: Advanced Platform GUI GA

## Phase Details

### Phase 1: Baseline Reconciliation and Renderer Decision
**Goal**: Establish the exact current state, reconcile planning truth, and choose a GUI renderer path without weakening architecture gates.

**Requirements**: R-001, R-002, R-003, R-004

**Recommended Agents**: project-manager-senior, engineering-senior-developer, testing-tool-evaluator, engineering-security-engineer

**Success Criteria**:
- Phase ledger/evidence conflict is resolved or explicitly superseded for the GUI track.
- Renderer ADR records accepted stack, fallback criteria, and Windows-first evidence requirements.
- Desktop adapter boundary is specified before code is added.
- Dependency policy and `xtask` rules describe any approved renderer crate edges.
- `devil-ui` remains projection-only and no GUI dependency is introduced without policy coverage.
- Verification includes `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and targeted app/UI tests.

**Plans**: 5

### Phase 2: Renderer-Backed Foundation Mode
**Goal**: Open a real desktop window that renders current shell projections and routes commands through existing app authority.

**Requirements**: R-004, R-005, R-006, R-007, R-013

**Recommended Agents**: engineering-senior-developer, engineering-frontend-developer, design-ux-architect, testing-performance-benchmarker

**Success Criteria**:
- Renderer-backed crate or binary launches a native window.
- GUI consumes `ShellProjectionSnapshot` and renders layout, explorer, active buffer viewport, status, proposal summary, and trust summary.
- Input/key/menu/file-dialog actions become `CommandDispatchIntent` or explicit app requests.
- User can open this repository, open a file, edit a small buffer, save, see conflict/rejection state, and quit.
- UI code does not depend on editor/project/storage internals beyond approved projection/protocol contracts.
- Renderer proof records input-to-paint, frame variance, focus, clipboard, IME, and accessibility smoke results.

**Plans**: 6

### Phase 3: Daily Editing MVP
**Goal**: Make local editing usable for real files and repeated sessions.

**Requirements**: R-007, R-008, R-013

**Recommended Agents**: engineering-senior-developer, design-ui-designer, testing-qa-verification-specialist, testing-performance-benchmarker

**Success Criteria**:
- Multi-tab editor, close/reopen behavior, explorer expand/collapse/selection/reveal, cursor/selection, scrolling, undo/redo, save all, and close-dirty prompts work in the GUI.
- Search in file and search in workspace work through approved projections/services.
- Session restore recovers workspace, tabs, focus, layout, and explorer state.
- External overwrite between open and save yields a visible conflict and preserves dirty text.
- Large-file degraded mode is preserved; GUI never requires unbounded full-source projection.

**Plans**: 6

### Phase 4: Language and Terminal IDE Loop
**Goal**: Add the minimum language-tooling and terminal workflow expected from a local IDE.

**Requirements**: R-009, R-013

**Recommended Agents**: lsp-index-engineer, terminal-integration-specialist, engineering-senior-developer, testing-api-tester

**Success Criteria**:
- Problems panel, diagnostics, hover, completion, go-to-definition, references, outline, formatting, rename, organize imports, and code-action surfaces are visible.
- Edit-producing language actions become proposal previews before mutation.
- Policy-gated terminal panel supports launch, input, resize, kill, bounded output, search/scrollback status, and denial/error states.
- Terminal and LSP cannot mutate editor buffers or disk directly.
- Failures are visible, cancellable where applicable, and metadata-audited.

**Plans**: 6

### Phase 5: Control, Trust, and Assisted AI Surfaces
**Goal**: Make the control-first differentiator visible and usable in the GUI.

**Requirements**: R-010, R-011, R-013

**Recommended Agents**: engineering-ai-engineer, engineering-security-engineer, design-ux-architect, testing-qa-verification-specialist

**Success Criteria**:
- Proposal ledger, proposal details, diff/target summary, approval checklist, rollback/checkpoint, context manifest, privacy inspector, and permission/risk/cost budget panels are usable.
- Assisted AI explain/propose flows use local-first/default-deny provider routing.
- AI-generated edits are proposals only and never self-applied.
- Users can see what context was used, what was redacted or denied, and what risk labels apply.
- Approval, rejection, cancellation, stale, conflict, failed, applied, and rolled-back states are visible.

**Plans**: 5

### Phase 6: Packaging, Platform Integration, and Accessibility
**Goal**: Turn the GUI into an installable Windows desktop application with credible platform behavior.

**Requirements**: R-012, R-013

**Recommended Agents**: engineering-infrastructure-devops, design-ui-designer, testing-qa-verification-specialist, testing-performance-benchmarker

**Success Criteria**:
- Windows packaged executable or installer is produced.
- Native menus, file dialogs, clipboard, keyboard shortcuts, theme, focus traversal, IME, high-DPI behavior, and accessibility tree have smoke evidence.
- Crash-safe session restore and diagnostics export are available.
- Smoke-test scripts cover install, launch, open workspace, edit/save, terminal, LSP, proposal review, and quit.
- macOS/Linux parity plan and initial CI smoke coverage are documented.

**Plans**: 5

### Phase 7: Fully Functional Local IDE Beta
**Goal**: Reach a beta that can be used as a local IDE for normal development.

**Requirements**: R-014

**Recommended Agents**: project-management-project-shepherd, engineering-senior-developer, testing-test-results-analyzer, product-technical-writer

**Success Criteria**:
- A real Rust repository can be opened, browsed, edited, searched, saved, checked through terminal or language tooling, and reviewed through proposal surfaces.
- GUI-visible diagnostics and operational health are available.
- Privacy-safe logs, redacted diagnostics export, release-readiness checklist, launch docs, and known limitations are complete.
- Critical workflows have automated smoke coverage and manual evidence.
- The beta does not claim unsupported remote/collaboration/plugin/autonomy behavior.

**Plans**: 4

### Phase 8: Advanced Platform GUI GA
**Goal**: Expose accepted advanced runtime surfaces through production-grade GUI workflows.

**Requirements**: R-015

**Recommended Agents**: agents-orchestrator, engineering-senior-developer, lsp-index-engineer, testing-qa-verification-specialist

**Success Criteria**:
- Plugin management and contribution views preserve sandbox, capability, and metadata-only audit boundaries.
- Collaboration presence, shared proposal review, reconnect, and conflict surfaces are usable.
- Remote workspace connection manager, remote terminal/LSP/session status, reconnect/offline indicators, and remote proposal review are usable.
- Delegated task command center remains bounded and approval-gated.
- Cross-platform release, update, rollback, and incident response procedures are documented and evidenced.
- Windows, macOS, and Linux platform parity evidence exists before GA claims.

**Plans**: 5

## Progress

| Phase | Plans | Completed | Status |
|-------|-------|-----------|--------|
| 1 | 5 | 5 | Complete |
| 2 | 6 | 6 | Complete |
| 3 | 6 | 0 | Not started |
| 4 | 6 | 0 | Not started |
| 5 | 5 | 0 | Not started |
| 6 | 5 | 0 | Not started |
| 7 | 4 | 0 | Not started |
| 8 | 5 | 0 | Not started |

## Planning Notes

- Total estimated plans: 42.
- Estimates are not caps. A phase may produce as many tasks as needed to satisfy the phase goal and verification contract.
- `/legion:plan` should use `.planning/CODEBASE.md`, `.planning/codebase/index.jsonl`, and `.planning/codebase/symbols.json` before decomposing each phase.
