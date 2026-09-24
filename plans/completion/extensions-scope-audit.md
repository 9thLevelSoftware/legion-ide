# S0-01t extension scope audit

The baseline was revision `ddfb166` with 354 requirement objects. This slice
removes only `COMP-SCOPE-FAMILY-17` and adds eleven atomic product outcomes
(`COMP-SCOPE-FAMILY-17-01..11`), for 364 total rows and 353 retained baseline
objects. Retained objects were compared after decoding the Git blob as UTF-8;
their fields are unchanged. Every new row has `acceptance: unassessed`.

## Source and authority basis

The approved extension promise is in
`docs/superpowers/specs/2026-09-04-product-completion-design.md`. The detailed
Stage 5 contract and qualification plan is
`docs/superpowers/plans/2026-09-04-ai-team-completion.md` (S5-01 through S5-04
and S5-15). The master-plan workstream is WS-EXT-01 in
`plans/legion-production-master-plan-v0.2.md`. Runtime and distribution
boundaries are defined by `plans/adrs/ADR-0019-wasm-plugin-runtime.md`,
`plans/adrs/ADR-0047-extension-distribution.md`, and
`plans/adrs/ADR-0050-wasmtime-runtime-ratification.md`.

The approved design supersedes the historical metadata-only restriction for
the required webview, notebook, custom-editor, and storage capabilities. The
current ADRs still limit Open VSX to declarative metadata and do not activate a
Node host. Accordingly, metadata/classification traces are implementation
substrate only; they do not satisfy runtime or product acceptance.

## Atomic inventory

| Row | Outcome | Package | Implementation | Evidence boundary |
| --- | --- | --- | --- | --- |
| 17-01 | Full host/API/contribution/version, WIT, isolation, Tier 3, storage, lifecycle, and envelope contract | S5-01 | absent | Proposed contract types and matrix are not present |
| 17-02 | Signed Wasmtime/WIT execution with bounded host calls and proposal-mediated effects | S5-02 | partial | `WasmPluginHost`, WIT files, hostile probes, and quotas are substrate only |
| 17-03 | Versioned extension services, activation, registration, cancellation, and safe host-call envelopes | S5-01 | partial | Manifest/host/session DTOs exist; full service contract is not ratified |
| 17-04 | Install/enable/disable/update/remove/restart/quarantine and interrupted-update rollback | S5-02 | partial | Signed registry and install evidence exist; complete product lifecycle is unassessed |
| 17-05 | Signatures, permissions, quotas, isolation, tamper rejection, crash quarantine, and no ambient authority | S5-02 | partial | Runtime denial and permission review exist; packaged product security is unassessed |
| 17-06 | Workspace/global/secret storage scopes, quotas, migration, deletion, retention, and restart persistence | S5-02 | absent | The planned storage service/file is not present |
| 17-07 | Supported VS Code Tier 1/2 Node and web-worker API subset with activation, IPC, storage, diagnostics, and recovery | S5-03 | absent | `legion-vscode-compat` classifies manifests but no host runtime exists |
| 17-08 | Secure webview UI channel with origin/resource/CSP/message controls and recovery | S5-04 | absent | Tier 3 classification exists; webview runtime/view is absent |
| 17-09 | Notebook identity, open/edit/run/interrupt/save/restart, bounded kernels, and proposal mediation | S5-04 | absent | Notebook runtime/view and product tests are absent |
| 17-10 | Custom-editor identity, open/edit/save/revert/backup, conflict-safe proposals, and recovery | S5-04 | absent | Custom-editor runtime/view and product tests are absent |
| 17-11 | Packaged Stage 5 qualification of named representative extension workflows | S5-15 | absent | No required immutable real-extension matrix result exists |

## Exact deduplication and protected owners

The retained `COMP-P7-F2-T1-1`, `COMP-P7-F2-T2-1`, `COMP-P7-F2-T3-1`, and
`COMP-P7-F2-T4-1` rows remain canonical for the narrower bundled-install,
permission-prompt, tamper-refusal, and metadata-only VSIX outcomes. Family 17
does not duplicate them. `COMP-TRUST-001`, `COMP-TRUST-003`,
`COMP-TRUST-004`, and `COMP-TRUST-005` remain canonical for generalized
proposal lifecycle, batch/rollback, audit, and trust/egress controls.
`COMP-PRES-008` remains canonical for save-conflict preservation. Family 15
owns the Stage 0 compatibility matrix; family 17 consumes it. The new rows
use `protected_product_ids` and dependencies for these relationships rather
than copying their outcomes.

No legacy ID is duplicated. Source paths in the replacement objects are
literal existing repository paths without `#fragment` suffixes. Implementation
labels are conservative current-source classifications; no row claims native,
external, packaged, or accepted evidence.
