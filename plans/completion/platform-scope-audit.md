# S0-01x platform quality scope audit

Date: 2026-09-05. Baseline: `f4630b4`. This inventory replaces only the
aggregate family-21 row. Product/native/external acceptance remains
`unassessed`; current source and historical platform records are implementation
traces only.

The approved family promise is Windows/macOS/Linux platform quality covering
accessibility, focus, DPI, window restore, responsiveness, startup, resource
bounds, and sustained use. The completion design's finite matrix rule requires
versioned OS, architecture, reference-hardware, display, input, and
accessibility-tool cells before dependent acceptance. XQ-02 requires packaged
OS-input/accessibility EvidenceRuns with UIA/Narrator, AX/VoiceOver, and
AT-SPI/Orca or an explicit blocked result. XQ-03 requires calibrated per-OS
renderer/product workloads, measured reference hardware, and sustained use.

## Atomic inventory

| ID | Atomic outcome | Package | Current trace and limit |
|---|---|---|---|
| `COMP-PLAT-001` | Stage 0 publishes the finite native OS/version/architecture/reference-hardware/display/input/accessibility matrix and explicit blocked cells. | S0-02 | `absent`: the approved plans define the prerequisite, but no current canonical matrix was found. |
| `COMP-PLAT-002` | Packaged native keyboard, pointer, text, clipboard, IME/CJK, and command input follows the authoritative route with focus, Unicode, dirty-state, and duplicate-action correctness. | S1-02 | `partial`: desktop adapters and input/IME tests exist; current 3-OS packaged product acceptance is not established. |
| `COMP-PLAT-003` | Packaged UI provides UIA/Narrator, AX/VoiceOver, and AT-SPI/Orca accessibility trees and keyboard-reachable focus, with truthful unavailable/blocked outcomes. | XQ-02 | `partial`: scripts, tests, and historical AccessKit/Narrator records exist; current cross-OS packaged evidence is not established. |
| `COMP-PLAT-004` | Native window lifecycle preserves workspace identity, focus, tabs, docks, layout, DPI, fonts, floating windows, and multi-monitor placement through resize, display changes, restart, and recovery. | S1-08 | `partial`: window/layout source and historical DPI smoke exist; current native cross-OS qualification is absent. |
| `COMP-PLAT-005` | Packaged startup and ordinary interaction meet declared per-platform responsiveness budgets for launch, first frame, input-to-paint, scrolling, resize, focus, and cancellation. | XQ-03 | `partial`: renderer metrics/manual performance substrate exists; calibrated packaged measurements are not complete. |
| `COMP-PLAT-006` | Product performance qualification enforces measured per-OS budgets for renderer input-to-paint, scroll, search cancellation, terminal throughput, large files/workspaces, startup, and memory plateau. | XQ-03 | `partial`: workload drivers and prior budgets exist; synthetic/headless or historical runs cannot substitute for the current candidate. |
| `COMP-PLAT-007` | Sustained native use stays within CPU, memory, descriptor, worker, and event-loop bounds with cancellation, degradation, cleanup, recovery, and user-work preservation. | S1-08 | `partial`: guardrail and operational-health tests exist; required 30-minute packaged multi-OS evidence is absent. |
| `COMP-PLAT-008` | File dialogs, clipboard, IME/CJK, keyring, PTY/ConPTY, watchers, menus, shortcuts, and display/input adapters use declared platform boundaries and report unavailable facilities without fixture substitution. | S1-08 | `partial`: platform adapter and parity tests cover substrate; native product effects across the ratified matrix remain unassessed. |
| `COMP-PLAT-009` | Packaged native qualification externally checks ordinary input, paint, focus, accessibility, layout/DPI, integrations, performance, resources, restart, cancellation, and recovery on every ratified OS cell; missing native access is blocked, never passed. | XQ-02 | `absent`: no current family-specific packaged EvidenceRun set establishes complete platform qualification. |

## Ownership and deduplication

The new rows add platform binding and qualification only. Text semantics remain
owned by `COMP-EDIT-*`; workbench layout/restart by `COMP-WB-005` and
`COMP-WB-009`; persistence by `COMP-PRES-*`; terminal parity by
`COMP-TERM-014`; and existing accessibility/performance qualification contracts
by `COMP-P8-F4-T1-1..T3-1` and `COMP-P8-F5-T1-1..T3-1`. Distribution,
signing, installation, update, crash, diagnostics, and no-egress remain family22
or XQ-owned and are not duplicated here.

Native GUI was not run. The local environment is Windows-only; macOS/Linux
native access is unavailable for this inventory. Headless tests, fixture runs,
historical parity reports, and prior GUI records remain supporting substrate and
do not establish current three-OS product proof.

All nine rows use `owner_role: luna_worker`, `acceptance: unassessed`, literal
existing source paths, and truthful `partial`/`absent` classifications. No
product code, Cargo file, GUI artifact, network action, commit, or unrelated
plan file was changed.

## Validation report

The exact UTF-8 Git blob for `plans/completion/requirements.json` at `f4630b4`
was decoded and compared object-by-object. Baseline had 401 requirement
objects; replacing family21 retains 400 objects byte-decoded equal to baseline
and adds nine rows, producing 409 total rows. No retained dependency or
protection edge referenced the removed family placeholder, so no authorized
rewiring was required.

- JSON parse: pass.
- IDs unique: pass.
- `depends_on` and `protected_product_ids`: pass, zero dangling targets.
- Combined relationship graph: pass, acyclic.
- New-row source paths: pass, all literal files exist.
- New rows: product, required, `owner_role: luna_worker`, `acceptance: unassessed`.
