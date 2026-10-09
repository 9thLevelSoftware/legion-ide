# IDE 2026 essential migration contract

Date: 2026-10-08. Ticket: 03. Status: resolved; documentation selection only.
Independent review: PASS (Chandrasekhar), no blocking findings; coordinator
reported approval on 2026-10-08. This resolves the bounded contract deliverable,
not downstream implementation or product acceptance.
Source baseline: integration `c2a65786862e759e67ebac28c32c1cbc618047f0`.

This contract freezes the selected migration subset for implementation by ticket
22, not a claim of currently supported import. The [approved specification](../../.scratch/ide-2026-planning/spec.md)
(Q27/Q30, A11), [execution decisions](../../.scratch/ide-2026-planning/execution.md)
and [reconciliation](ide-2026-reconciliation.md) govern scope. The owner approved
the observed Windows/Rust host, Legion-on-Legion and demonstrated native
equivalents for initial pilot workflows. No renewed selection approval is needed.
Other platforms and the required extension/remote obligations remain in scope.
Canonical requirements, matrix, scenarios, dependencies and readiness are unchanged.

## Inspected support and authority

`crates/legion-ui/src/ui.rs` defines `SettingsProjection`, settings intents and
`default_keymap()`. `crates/legion-app/src/intent_routing.rs` and app composition
route settings changes; `crates/legion-app/tests/settings.rs` covers projection
updates, normalization and wrapping behavior. These tests were inspected, not run.
`crates/legion-desktop/src/view/keymap_dispatch.rs` reads the fixed default keymap,
maps action labels and preserves editor focus gates. It does not load a user keymap.
The scoped source inspection and current-state notes identify no VS Code user
settings/keybindings importer, collision preview or reversible import transaction.

ADR-0047's declarative keybinding classification in `legion-vscode-compat` is
extension metadata handling, not personal import or extension execution. Follow
[authority boundaries](../../docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md) and
[glossary](../../GLOSSARY.md): UI emits intents; app owns configuration workflow;
editor owns buffers; workspace owns guarded file mutation. Import cannot grant
execution, trust, network, provider, terminal or debugger permissions.

## Frozen settings subset for ticket 22

Input is an explicitly selected VS Code-style settings object, never automatic
profile discovery. Only these global keys are selected. All are **planned mappings**;
the current native target is available but the import path is absent.

| Source key and accepted value | Native destination and mapping |
| --- | --- |
| `editor.lineNumbers`: `on` or `off` | `SetLineNumbersVisible`: true or false; relative/interval modes unsupported. |
| `editor.minimap.enabled`: boolean | `SetMinimapVisible`, same boolean. |
| `editor.folding`: boolean | `SetCodeFoldingVisible`, same boolean. |
| `editor.stickyScroll.enabled`: boolean | `SetStickyHeadersVisible`, same boolean. |
| `editor.smoothScrolling`: boolean | `SetSmoothScrollingEnabled`, same boolean. |
| `editor.wordWrap`: `off`, `on`, `wordWrapColumn` | `SetLineWrappingPolicy`: Off, Viewport, FixedColumn respectively. `bounded` unsupported. |
| `editor.wordWrapColumn`: integer 40–240 | Paired with FixedColumn only; other modes report not applicable, never silently change wrap mode. Missing column for FixedColumn is a conflict requiring an explicit choice. |

Font size/family, zoom and theme are excluded: native fields exist, but unit,
font resolution and theme identity equivalence are not established by this ticket.
Tab/indent/newline, save/format actions, language overrides (`[rust]` etc.), workspace
scope, tasks, launch configurations, paths, shell profiles, extension configuration,
AI/provider, telemetry and security settings are outside this selected subset.
Unknown keys are reported unsupported; no speculative passthrough or execution.

## Frozen keybinding subset for ticket 22

Selected source commands map only to existing native action labels below. Accepted
keys are one Windows key press with optional Ctrl/Shift/Alt modifiers and a key
recognized by `key_label_to_egui`; source labels must be normalized explicitly
(for example `up` to `ArrowUp`). Chords, removal commands, arguments, `when`
expressions and platform-specific overrides are unsupported in this first subset.
No user-remapping path is present today; ticket 22 must add one through app-owned
configuration and retain desktop editor-focus/context gates.

| Source command | Native action label |
| --- | --- |
| `workbench.action.files.save` | `SaveActive` |
| `workbench.action.files.saveAll` | `SaveAll` |
| `actions.find` | `ToggleFindBar` |
| `editor.action.startFindReplaceAction` | `ToggleFindReplace` |
| `workbench.action.findInFiles` | `SearchWorkspace` |
| `workbench.action.gotoLine` | `GoToLine` |
| `workbench.action.quickOpen` | `OpenPalette` |
| `workbench.action.showCommands` | `OpenCommandPalette` |
| `undo` / `redo` | `Undo` / `Redo` |
| `workbench.action.closeActiveEditor` | `CloseTab` |

These are selected semantic targets, not proof of identical VS Code behavior.
In particular, `ToggleFindBar` currently opens the active-file search palette in
desktop dispatch; it does not open the in-editor find bar. Ticket 22 must expose
that semantic difference in preview rather than describe an exact UI migration.
Debug, Git and language shortcuts remain native defaults, outside the import
subset. Unsupported commands and unrecognized key names produce report rows.

## Required preview, conflict and reversal behavior

Ticket 22 owns implementation and qualification under COMP-WB-008 / S1-04 and
A11; it depends on ticket 08's restoration work. It must provide:

- A bounded, non-executing JSON/JSONC reader for an explicit settings object or
  keybinding array. Malformed input, duplicate object keys and limit violations
  reject the preview without applying anything. Comments/trailing commas may be
  parsed, never evaluated. The implementation owner must pin input limits before
  activation; this document does not invent a current parser capability.
- Per-item supported, unsupported, unchanged, invalid and conflict outcomes,
  showing source key/command, destination and proposed semantic conversion.
  Reports must exclude credentials/raw unrelated preferences.
- Explicit selection of supported entries. Existing different values, duplicate
  source destinations and shortcut collisions with defaults or existing overrides
  remain conflicts until the user chooses keep or replace for that destination.
  No last-wins behavior; unchecked/unsupported items change nothing. Preserve
  current scope precedence rather than inventing workspace overrides.
- Preview bound to the current configuration revision. Changed configuration
  requires renewed preview; cancel or failed apply leaves original preferences.
  Persist only selected changes through the proper app/workspace authority.
- Reversal of the import's exact changed entries using recorded before/after
  values. Later edits to those entries produce conflicts rather than overwrite;
  unrelated preferences survive both import and reversal. Reopened storage and
  actual native input must independently confirm persistence and shortcut effect.

No personal settings, keybindings, secrets or credentials were inspected. Only
repository interfaces and provided read-only preflight/setup notes informed scope.

## Essential initial pilot workflows and acceptance handoffs

The essential initial set is native Rust/edit/build/test/debug/Git, with **no
required VS Code extension installation**. Native service presence below identifies
reuse, not successful native acceptance. Tickets resolve through the approved
breakdown; canonical anchors retain their existing meaning.

| Workflow / disposition | Existing native service | Gap and downstream acceptance owner |
| --- | --- | --- |
| Edit/save/recover: native equivalent | `legion-editor`, bounded `legion-text`, `AppComposition::save_active_buffer`, SaveWorkflowService and `WorkspaceActor::save_file_with_proposal` | Tickets 04–08, 21 and 26; COMP-EDIT-001, COMP-PRES-008. Actual Unicode/input, undo, conflict preservation and recovery need native evidence; keep fingerprint/version/generation guards. |
| Rust language/navigation: native equivalent | App LanguageToolingWorkflow and language/toolchain settings; `legion-lsp` completion, hover, definition and diagnostics | Tickets 15–16 and 26; COMP-LANG-001 / S2-01. Use owner-selected rust-analyzer 1.98.1; qualify real responses, stale/cancel/unavailable behavior and multi-file edits. No Rust extension compatibility implied. |
| Rust build/test: native equivalent | App process/task routing, TerminalWorkflow, `refresh_test_explorer`, `run_test_explorer_item` / `run_test_explorer_group` | Ticket 17 and 26; COMP-BTD-004 / S2-04; terminal ticket 14. Real Cargo 1.98.1 passing/failing/selected tests, output, exit status, cancellation and process cleanup remain to qualify. No imported tasks or test extension promised. |
| Rust debug: native equivalent with qualification prerequisite | App DebugWorkflow and `legion-debug` DapClientRuntime / LiveDapSession, policy-gated adapter resolver | Ticket 18 and 26; COMP-BTD-007 / S2-05. CodeLLDB 1.12.3 standalone adapter is an approved extracted candidate, not a compatible installed extension. Qualify launch/attach, breakpoint, step, variables/evaluation and termination against MSVC/PDB; enum decoding limits remain explicit. |
| Git status/diff/stage/commit/conflict/history: native equivalent | App `git_inspection.rs`, `git_policy.rs`, `git_remote.rs` and Git projections/intents | Tickets 19–20 and 26; COMP-SCM-010 / S1-07. Demonstrate exact selected effects, conflict resolution and history restoration with native input and external Git oracle; no Git extension required. |

Ticket 02 owns canonical host/tool/scenario ratification. Ticket 26 owns the actual
five-working-day Manual pilot; ticket 25 proves no AI invocation. This document
does not promote any workflow from substrate to product-ready.

## Later required named extension and remote targets

| Installed identity/version from supplied inventory | Required later outcome / owners | Current disposition |
| --- | --- | --- |
| `ms-azuretools.vscode-containers` 2.5.2 (Containers) | Container management/development workflow inventory, finite execution contract and real representative extension qualification: tickets 155–168; COMP-SCOPE-FAMILY-17-01 / S5-01 and COMP-SCOPE-FAMILY-17-11 / S5-15 | Named required target; compatibility unqualified. Not an initial pilot dependency. |
| `ms-vscode-remote.remote-containers` 0.469.0 (Remote Containers) | Container workspace open/reconnect and identity/authority preservation: tickets 169–211, coordinated with 155–168; COMP-REMOTE-017 / S5-15 | Named required target; compatibility unqualified. Not an initial pilot dependency. |

Installed identity/version alone does not establish signed immutable artifact,
runtime activation, extension APIs, containment, lifecycle or reconnect support.
Ticket 155 must freeze exact supported commands/APIs and artifacts; ticket 169
must freeze actual container/remote host and authority contract. Tickets 168 and
211 own independent positive workflow evidence. Native pilot approval does not
waive these full-program obligations or promise universal VS Code parity.

## Verification and remaining prerequisites

Planned before edits: existing xtask `docs-hygiene`,
`verify-completion-register --root .` and `git diff --check`, each once.
Exact results are recorded in ticket 03. No artificial runtime tests, feature
changes, canonical register edits, provisioning or acceptance promotion occur.

The supplied setup notes supersede their older missing-resource observations:
CodeLLDB's extracted adapter hash matches the approved artifact, and the existing
native-input driver was built and attached to the interactive Default desktop.
Neither adapter availability nor driver handshake proves a real product workflow.
Remaining gates are ticket 22's importer/persistence/input behavior; real DAP/MSVC
qualification; frozen candidate/configuration and native/AT evidence; the actual
Manual observation period; and later extension artifacts, runtime contracts,
container hosts/images and independent lifecycle/reconnect qualification.
