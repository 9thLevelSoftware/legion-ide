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

The Windows display reported 144 DPI (150%). The native client was therefore resized to 2160x1350 physical pixels for a 1440x900 logical capture and to 1440x1080 physical pixels for a 960x720 logical capture. Win32 `GetClientRect` and `ClientToScreen` supplied the exact client origin; `GetDpiForWindow` verified the scale. The captured physical client was then downsampled with Lanczos. No browser, mocked DOM, or synthetic renderer was used for the native images.

The exact launch and capture commands were:

```powershell
cargo build -p legion-desktop
$captureDir = 'D:\tmp\legion-native-capture-20260806-final'
$app = Start-Process .\target\debug\legion-desktop.exe -ArgumentList @(
  '--workspace', 'D:\legion-ide\.worktrees\ui-prototype-polish',
  '--file', 'crates/legion-desktop/src/view.rs',
  '--session-state', "$captureDir\session-final-accepted.json"
) -WindowStyle Normal -PassThru

Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class NativeCapture {
  public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr state);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr state);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref POINT point);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after,
    int x, int y, int width, int height, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx,
    uint dy, uint data, UIntPtr extraInfo);
  public static IntPtr FindTitledWindow(uint processId, string title) {
    IntPtr result = IntPtr.Zero;
    EnumWindows((hwnd, state) => {
      GetWindowThreadProcessId(hwnd, out uint owner);
      var text = new StringBuilder(256);
      GetWindowText(hwnd, text, text.Capacity);
      if (owner == processId && IsWindowVisible(hwnd) && text.ToString() == title) {
        result = hwnd;
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return result;
  }
}
'@

try { $app.WaitForInputIdle() | Out-Null } catch { }
$hwnd = [NativeCapture]::FindTitledWindow([uint32]$app.Id, 'Legion IDE')
if ($hwnd -eq [IntPtr]::Zero) { throw 'visible Legion IDE HWND was not found' }
$client = [NativeCapture+RECT]::new()
$window = [NativeCapture+RECT]::new()
[NativeCapture]::GetClientRect($hwnd, [ref]$client) | Out-Null
[NativeCapture]::GetWindowRect($hwnd, [ref]$window) | Out-Null
$chromeWidth = ($window.Right - $window.Left) - ($client.Right - $client.Left)
$chromeHeight = ($window.Bottom - $window.Top) - ($client.Bottom - $client.Top)
$dpi = [NativeCapture]::GetDpiForWindow($hwnd) # 144
$scale = $dpi / 96.0 # 1.5

# This caller receives logical Win32 coordinates. A 1440x900 logical client is
# 2160x1350 physical at 150%; gdigrab requires the converted physical values.
[NativeCapture]::SetWindowPos($hwnd, [IntPtr]::Zero, 180, 20,
  1440 + $chromeWidth, 900 + $chromeHeight, 0x0040) | Out-Null
Start-Sleep -Milliseconds 500
$clientOrigin = [NativeCapture+POINT]::new()
[NativeCapture]::ClientToScreen($hwnd, [ref]$clientOrigin) | Out-Null
[NativeCapture]::GetClientRect($hwnd, [ref]$client) | Out-Null
if ($client.Right -ne 1440 -or $client.Bottom -ne 900) { throw 'desktop client resize failed' }
$physicalX = [math]::Floor($clientOrigin.X * $scale) # 280
$physicalY = [math]::Floor($clientOrigin.Y * $scale) # 75
$physicalWidth = [Convert]::ToInt32($client.Right * $scale) # 2160
$physicalHeight = [Convert]::ToInt32($client.Bottom * $scale) # 1350
$rawDesktop = "$captureDir\manual-final-2160x1350-raw.png"
$desktopCapture = "$captureDir\manual-1440x900.png"
ffmpeg -hide_banner -loglevel error -f gdigrab -draw_mouse 0 -framerate 30 `
  -offset_x $physicalX -offset_y $physicalY -video_size "${physicalWidth}x${physicalHeight}" `
  -i desktop -frames:v 1 -update 1 -y $rawDesktop
ffmpeg -hide_banner -loglevel error -i $rawDesktop `
  -vf 'scale=1440:900:flags=lanczos' -frames:v 1 -update 1 -y $desktopCapture

# The same two ffmpeg commands were repeated after native pointer actions.
# Coordinates are logical client coordinates. Escalation confirmation is a
# real two-step interaction; the inert wordmark click advances the subsequent
# projection frame without invoking an application action.
# Assist: click 660,26.
# Delegate: click 784,26; click Confirm at 567,455; click 100,22.
# Legion Workflows: click 911,26; click Confirm at 567,455; click 100,22.
# Return to Assist: click 660,26.

# 960x720 logical at 150%: 1440x1080 physical.
[NativeCapture]::SetWindowPos($hwnd, [IntPtr]::Zero, 500, 200,
  960 + $chromeWidth, 720 + $chromeHeight, 0x0040) | Out-Null
Start-Sleep -Milliseconds 500
$clientOrigin = [NativeCapture+POINT]::new()
[NativeCapture]::ClientToScreen($hwnd, [ref]$clientOrigin) | Out-Null
[NativeCapture]::GetClientRect($hwnd, [ref]$client) | Out-Null
if ($client.Right -ne 960 -or $client.Bottom -ne 720) { throw 'compact client resize failed' }
$physicalX = [math]::Floor($clientOrigin.X * $scale)
$physicalY = [math]::Floor($clientOrigin.Y * $scale)
$physicalWidth = [Convert]::ToInt32($client.Right * $scale) # 1440
$physicalHeight = [Convert]::ToInt32($client.Bottom * $scale) # 1080
$rawCompact = "$captureDir\assist-final-1440x1080-raw.png"
$compactCapture = "$captureDir\assist-960x720.png"
ffmpeg -hide_banner -loglevel error -f gdigrab -draw_mouse 0 -framerate 30 `
  -offset_x $physicalX -offset_y $physicalY -video_size "${physicalWidth}x${physicalHeight}" `
  -i desktop -frames:v 1 -update 1 -y $rawCompact
ffmpeg -hide_banner -loglevel error -i $rawCompact `
  -vf 'scale=960:720:flags=lanczos' -frames:v 1 -update 1 -y $compactCapture
```

Mode changes and onboarding dismissal were performed through real pointer clicks in the native window. The screenshots and session file remain temporary evidence and are deliberately not committed.

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
