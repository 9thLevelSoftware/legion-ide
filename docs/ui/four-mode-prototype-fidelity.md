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

- `D:\tmp\legion-native-capture-20260806-task6\manual-1440x900.png`
- `D:\tmp\legion-native-capture-20260806-task6\assist-1440x900.png`
- `D:\tmp\legion-native-capture-20260806-task6\delegate-1440x900.png`
- `D:\tmp\legion-native-capture-20260806-task6\legion-workflows-1440x900.png`
- `D:\tmp\legion-native-capture-20260806-task6\assist-960x720.png`

The Windows display reported 144 DPI (150%). The native client was therefore resized to 2160x1350 physical pixels for a 1440x900 logical capture and to 1440x1080 physical pixels for a 960x720 logical capture. Win32 `GetClientRect` and `ClientToScreen` supplied the exact client origin; `GetDpiForWindow` verified the scale. The captured physical client was then downsampled with Lanczos. No browser, mocked DOM, or synthetic renderer was used for the native images.

The exact launch and capture commands were:

```powershell
$env:CARGO_BUILD_JOBS='1'
cargo build -p legion-desktop
$app = Start-Process .\target\debug\legion-desktop.exe -ArgumentList @(
  '--workspace', 'D:\legion-ide\.worktrees\ui-prototype-polish',
  '--file', 'crates/legion-desktop/src/view.rs',
  '--session-state', 'D:\tmp\legion-native-capture-20260806-task6\session-final.json'
) -PassThru

# After a DPI-aware Win32 resize/query, these values are the physical client origin.
ffmpeg -hide_banner -loglevel error -f gdigrab -draw_mouse 0 -framerate 30 `
  -offset_x $clientOrigin.X -offset_y $clientOrigin.Y -video_size 2160x1350 `
  -i desktop -frames:v 1 -update 1 -y $rawDesktop
ffmpeg -hide_banner -loglevel error -i $rawDesktop `
  -vf 'scale=1440:900:flags=lanczos' -frames:v 1 -update 1 -y $desktopCapture

ffmpeg -hide_banner -loglevel error -f gdigrab -draw_mouse 0 -framerate 30 `
  -offset_x 511 -offset_y 225 -video_size 1440x1080 `
  -i desktop -frames:v 1 -update 1 -y assist-final-1440x1080-raw.png
ffmpeg -hide_banner -loglevel error -i assist-final-1440x1080-raw.png `
  -vf 'scale=960:720:flags=lanczos' -frames:v 1 -update 1 -y assist-960x720.png
```

Mode changes and onboarding dismissal were performed through real pointer clicks in the native window. The screenshots and session file remain temporary evidence and are deliberately not committed.

## Fidelity ledger

| Area | Accepted behavior | Native evidence and disposition |
| --- | --- | --- |
| Shell geometry | 42px top bar; 46px activity rail plus approximately 248px explorer; approximately 325px right rail; 192px center console; 24px full-width status | Matched deterministically at 1440x900. Physical allocation tests verify the top and status span the viewport, both side rails span from top to status, and the console is limited to the center column. P1 closed. |
| Four-mode switch | Centered Manual, Assist, Delegate, and fourth-mode selector with command affordance at the right | Matched. Three disjoint top-bar regions prevent workspace text, switch, and Command from colliding. Compact labels remain readable. The fourth label intentionally reads `Legion Workflows`. P1 closed. |
| Dark token palette | Near-black editor, blue-gray panels, subdued borders, amber manual/action accents, blue Assist, violet Delegate, workflow accent | Matched through shared native theme tokens. Native anti-aliasing and system font metrics create minor tone/weight differences (P2). |
| Explorer/editor composition | Activity strip, repository tree, tabs/breadcrumb context, syntax editor as the dominant canvas | Structurally matched with live workspace projection. The native capture truthfully shows this worktree and file rather than the reference's illustrative repository. Activity controls use compact text rather than the artifact's icon set (P2; no licensed/bundled icon asset exists). |
| Manual rail | Disengaged AI state with a clear route to Assist | Matched. Manual suppresses agent/presence surfaces and states zero-egress/local-only semantics. Settings remain reachable below it. |
| Assist rail | Inline prediction, context/model controls, suggestions, explicit user acceptance | Matched to projected state. In-flight requests expose Cancel without a duplicate Predict action. The current empty provider/prediction state is intentionally truthful rather than inventing reference data. Long IDs and path chips are denser than the concept (P2). |
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

No P0 or P1 fidelity gap remains after the native comparison. The comparison found and closed stale cross-render-pass galley reuse (garbled native glyphs), panel ordering, compact rail reachability, top-bar region collision, duplicate Assist in-flight actions, undersized controls, inert settings selectors, and the untruthful Delegate feedback action.

The remaining P2 gaps are cosmetic or truthfulness-preserving: system font metrics, text activity controls in place of unbundled icons, denser diagnostic copy, compact wrapping of long IDs/paths, and absence of the reference's invented task/provider/terminal data. None obscures mode identity, overlaps a required control, makes a surface unreachable, or changes the authority model.
