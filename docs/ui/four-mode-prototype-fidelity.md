# Four-mode prototype fidelity ledger

Date: 2026-08-06
Native implementation: `legion-desktop`
Accepted concept source: the four local Design Compose HTML artifacts and their PNG captures

## Evidence and capture method

The accepted concept was rendered locally in Microsoft Edge because this environment had no Browser/IAB connector. Edge was used only to turn the already accepted local HTML artifact into a stable reference image; it was not used as implementation evidence. The implementation evidence is the real `legion-desktop.exe` native window, opened against this repository and `crates/legion-desktop/src/view.rs`.

Reference images inspected with `view_image`:

- `D:\tmp\legion-prototype-reference-20260806\manual-1440x900.png`
- `D:\tmp\legion-prototype-reference-20260806\assist-1440x900.png`
- `D:\tmp\legion-prototype-reference-20260806\delegate-1440x900.png`
- `D:\tmp\legion-prototype-reference-20260806\autonomous-1440x900.png`
- `D:\tmp\legion-prototype-reference-20260806\assist-960x720.png`
- `C:\Users\dasbl\.codex\generated_images\019fd524-3dce-7311-8950-d40fcccabbc4\exec-dd1f6bef-07f6-4051-b640-b45ea005aff0.png` (accepted compact visual direction)

Native images inspected in the same QA pass:

- `D:\tmp\legion-native-capture-20260806-final\manual-1440x900.png`
- `D:\tmp\legion-native-capture-20260806-final\assist-1440x900.png`
- `D:\tmp\legion-native-capture-20260806-final\delegate-1440x900.png`
- `D:\tmp\legion-native-capture-20260806-final\legion-workflows-1440x900.png`
- `D:\tmp\legion-native-capture-20260806-final\assist-960x720.png`

The Windows display reported 144 DPI (150%), but this per-monitor-aware native window's `GetClientRect` and `ClientToScreen` values were already physical pixels. The capture therefore used those values directly: exactly 1440x900 and 960x720 client pixels. Applying the DPI factor again was rejected during QA because it captured desktop pixels outside the app. `System.Drawing` dimension checks and `view_image` inspection confirmed every final PNG has the requested dimensions and contains only the Legion client.

The exact top-level commands were:

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo build -p legion-desktop
& .\.superpowers\sdd\capture-final.ps1
```

The local capture helper launched `target\debug\legion-desktop.exe` with the worktree, `crates/legion-desktop/src/view.rs`, and the deterministic session file. It then:

- polled for the visible `Legion IDE` HWND by process and title instead of trusting `WaitForInputIdle`;
- made the native window temporarily topmost so a foreground Teams/Edge window could not occlude evidence;
- resized and asserted the Win32 client at 1440x900, then 960x720;
- activated the client with an inert wordmark click, used real pointer down/up events with a 100ms dwell, and clicked the actual mode controls;
- confirmed both privilege-increasing transitions through the real `Confirm` button, while the inert wordmark click advanced the following projection frame;
- captured `ClientToScreen` origin plus `GetClientRect` width/height directly with `ffmpeg` `gdigrab`, without an additional DPI multiplier; and
- verified Manual, Assist, Delegate, Legion Workflows, and compact Assist individually with `view_image`.

The pointer sequence was wordmark → Manual capture; Assist → wordmark → Assist capture; Delegate → Confirm → wordmark → Delegate capture; Legion Workflows → Confirm → wordmark → workflow capture; Assist → wordmark → compact resize and capture. Screenshots, raw frames, the session copy, and the capture helper remain temporary local evidence and are deliberately not committed.

## Fidelity ledger

| Area | Accepted behavior | Native evidence and disposition |
| --- | --- | --- |
| Shell geometry | 42px top bar; 46px activity rail plus approximately 248px explorer; approximately 325px right rail; 192px center console; 24px full-width status | Matched deterministically at 1440x900. Physical allocation tests verify the top and status span the viewport, both side rails span from top to status, and the console is limited to the center column. A same-view Manual→Assist→Delegate→Workflows test proves long projected IDs cannot make the default right rail drift across modes; desktop resizing remains bounded to 260–325px. P1 closed. |
| Four-mode switch | Centered Manual, Assist, Delegate, and fourth-mode selector with command affordance at the right | Matched. Three disjoint top-bar regions prevent workspace text, switch, and Command from colliding. Compact labels remain readable. The fourth label intentionally reads `Legion Workflows`. P1 closed. |
| Dark token palette | Near-black editor, blue-gray panels, subdued borders, amber manual/action accents, blue Assist, violet Delegate, workflow accent | Matched through shared native theme tokens. Native anti-aliasing and system font metrics create minor tone/weight differences (P2). |
| Explorer/editor composition | Activity strip, repository tree, tabs/breadcrumb context, syntax editor as the dominant canvas | Structurally matched with live workspace projection. The native capture truthfully shows this worktree and file rather than the reference's illustrative repository. Activity controls use unwrapped `Files`/`Find`/`Sym` text with full accessible names rather than the artifact's icon set (P2; no licensed/bundled icon asset exists). |
| Manual rail | Disengaged AI state with a clear route to Assist | Matched. Manual suppresses agent/presence surfaces and states zero-egress/local-only semantics. Settings remain reachable below it. |
| Assist rail | Inline prediction, context/model controls, suggestions, explicit user acceptance | Matched to projected state. In-flight requests expose Cancel without a duplicate Predict action. The current empty provider/prediction state is intentionally truthful rather than inventing reference data. Long IDs and path chips are middle-truncated to protect the 325px rail (P2). |
| Delegate rail | Bounded task entry, sandbox/scope/budget context, staged proposal flow | Matched with a real draft control and proposal-mediated confirmation. Human Feedback has no fake Send action while no editable feedback draft exists. Empty permission/runtime projections remain explicit rather than fabricated. |
| Legion Workflows rail | Multi-task command center, budgets, risk/gate visibility | Empty-state composition is matched, with settings reachable by scrolling. It does not invent running jobs, budgets, or approvals when none are projected. This is intentionally more conservative than the reference's illustrative cards. |
| Terminal/status | Tabbed bottom console under the editor and mode/status context across the full bottom edge | Matched. Panel-order tests verify the console cannot extend under either side rail. Native content exposes current projected terminal/runtime metadata and is consequently more diagnostic than the concept (P2). |
| Compact 960x720 | Deterministic adaptation, no overlap, at least 360px of editor, secondary surfaces reachable | Matched. The workspace subtitle is suppressed, the four-mode switch stays centered and readable, the rail scrolls vertically, and rendered-frame tests verify at least 360px visible editor width and 180px height for expanded workbenches. |
| Copy | Concise mode-specific product language | Safety-critical meaning is matched. Native copy is deliberately more explicit about projection, unavailable capabilities, scope, and telemetry. Some empty-state diagnostics are visually denser (P2). |
| Focus/accessibility | Keyboard-operable named controls with visible state and usable target sizes | AccessKit tests cover names, selected/current state, dialogs, actions, and the compact right-rail scrollbar. Keyboard tests cover switch traversal, activation, confirmation cancel/restore, and modal trapping. Visible action/settings targets are at least 24px high. |

## Intentional deviations

- `Legion Workflows` replaces the artifact label `Autonomous` so the UI uses the accepted product taxonomy.
- Manual hides agent and presence surfaces; it does not merely recolor an active agent rail.
- Workflow mutations remain proposal-mediated. A presentation-level mode confirmation does not grant operation permissions.
- There is no blanket low-risk auto-approval. The reference's illustrative auto-approved job is not reproduced without authoritative projected state.
- System fonts substitute for the prototype's web fonts because those font assets and their licensing are not bundled in the repository.

## Gap disposition

No P0 or P1 fidelity gap remains after the native comparison. The comparison found and closed stale/cumulatively unbounded galley reuse (garbled native glyphs), panel ordering, content-driven right-rail width drift, compact rail reachability, top-bar region collision, duplicate Assist in-flight actions, undersized provider/BYOK and shared controls, inert settings selectors, and the untruthful Delegate feedback action.

The remaining P2 gaps are cosmetic or truthfulness-preserving: system font metrics, text activity controls in place of unbundled icons, denser diagnostic copy, compact wrapping of long IDs/paths, and absence of the reference's invented task/provider/terminal data. None obscures mode identity, overlaps a required control, makes a surface unreachable, or changes the authority model.
