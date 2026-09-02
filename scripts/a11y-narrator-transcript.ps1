# Capture a Windows Narrator transcript of a live legion-desktop window.
#
# WHY THIS EXISTS
# ---------------
# `scripts/a11y-uia-walk.ps1` walks the UIA tree. That is not a screen-reader
# session (GAP-05.2 stop condition). This script starts Windows Narrator against a live
# native window, asks it to read the window, opens Narrator Speech Recap, and
# copies the spoken history Narrator itself recorded.
#
# SCOPE: Windows Narrator only. Not NVDA, VoiceOver, or Orca.
#
# USAGE
# -----
#   1. Start a live window, e.g.
#      cargo run -p legion-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 90000
#   2. While it is up:
#      powershell -NoProfile -ExecutionPolicy Bypass -File scripts/a11y-narrator-transcript.ps1
#
# Exit codes: 0 captured; 3 UIA unavailable; 4 process not running;
# 5 no top-level window; 6 Narrator did not record spoken text
# (empty, stale clipboard, or no Legion product speech);
# 7 Narrator did not start.

param(
  [string]$ProcName = "legion-desktop",
  [string]$OutFile = ""
)

$ErrorActionPreference = "Stop"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class NarratorKeys {
  public const uint KEYEVENTF_KEYUP = 0x0002;
  public const byte VK_CONTROL = 0x11;
  public const byte VK_MENU = 0x12;
  public const byte VK_CAPITAL = 0x14;
  public const byte VK_TAB = 0x09;
  public const byte VK_W = 0x57;
  public const byte VK_X = 0x58;
  [DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  public static void Down(byte vk) { keybd_event(vk, 0, 0, UIntPtr.Zero); }
  public static void Up(byte vk) { keybd_event(vk, 0, KEYEVENTF_KEYUP, UIntPtr.Zero); }
  public static void Tap(byte vk) { Down(vk); System.Threading.Thread.Sleep(40); Up(vk); }
}
"@ | Out-Null

function Send-NarratorChord([byte[]]$keys) {
  [NarratorKeys]::Down([NarratorKeys]::VK_CAPITAL)
  Start-Sleep -Milliseconds 40
  foreach ($k in $keys) { [NarratorKeys]::Down($k) }
  Start-Sleep -Milliseconds 40
  [void][array]::Reverse($keys)
  foreach ($k in $keys) { [NarratorKeys]::Up($k) }
  [NarratorKeys]::Up([NarratorKeys]::VK_CAPITAL)
  Start-Sleep -Milliseconds 250
}

try {
  Add-Type -AssemblyName UIAutomationClient
  Add-Type -AssemblyName UIAutomationTypes
} catch {
  Write-Output "UIA_LOAD_FAILED: $($_.Exception.Message)"
  exit 3
}

$procs = @(Get-Process -Name $ProcName -ErrorAction SilentlyContinue)
if ($procs.Count -eq 0) {
  Write-Output "PROCESS_NOT_FOUND: $ProcName"
  exit 4
}

$root = [System.Windows.Automation.AutomationElement]::RootElement
$legionWindow = $null
$legionHwnd = [IntPtr]::Zero
$findDeadline = (Get-Date).AddSeconds(20)
while ((Get-Date) -lt $findDeadline -and -not $legionWindow) {
  $procs = @(Get-Process -Name $ProcName -ErrorAction SilentlyContinue)
  foreach ($p in $procs) {
    $cond = New-Object System.Windows.Automation.PropertyCondition(
      [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $p.Id)
    $wins = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
    foreach ($w in $wins) {
      if ($w.Current.ControlType.ProgrammaticName -eq "ControlType.Window") {
        $legionWindow = $w
        $legionHwnd = [IntPtr]$w.Current.NativeWindowHandle
        break
      }
    }
    if ($legionWindow) { break }
  }
  if (-not $legionWindow) { Start-Sleep -Milliseconds 400 }
}
if (-not $legionWindow) {
  Write-Output "WINDOW_NOT_FOUND: $ProcName is running but published no top-level UIA window"
  exit 5
}

$windowName = $legionWindow.Current.Name
$startedNarrator = $false
if (-not (Get-Process -Name Narrator -ErrorAction SilentlyContinue)) {
  $homeKey = "HKCU:\Software\Microsoft\Narrator\NarratorHome"
  if (-not (Test-Path $homeKey)) { New-Item -Path $homeKey -Force | Out-Null }
  New-ItemProperty -Path $homeKey -Name "AutoStart" -Value 0 -PropertyType DWord -Force | Out-Null
  Start-Process -FilePath "$env:WINDIR\System32\Narrator.exe"
  $startedNarrator = $true
  $deadline = (Get-Date).AddSeconds(15)
  while (-not (Get-Process -Name Narrator -ErrorAction SilentlyContinue)) {
    if ((Get-Date) -gt $deadline) { break }
    Start-Sleep -Milliseconds 200
  }
  Start-Sleep -Seconds 2
}

if (-not (Get-Process -Name Narrator -ErrorAction SilentlyContinue)) {
  Write-Output "NARRATOR_NOT_RUNNING"
  exit 7
}

Add-Type -AssemblyName System.Windows.Forms | Out-Null

function Set-ClipboardSentinel([string]$sentinel) {
  try {
    [System.Windows.Forms.Clipboard]::SetText($sentinel)
    return $true
  } catch {
    return $false
  }
}

function Copy-LastSpokenPhrase {
  $sentinel = "LEGION_NARRATOR_SENTINEL_{0}" -f [guid]::NewGuid().ToString("N")
  if (-not (Set-ClipboardSentinel $sentinel)) {
    return ""
  }
  Send-NarratorChord @([NarratorKeys]::VK_CONTROL, [NarratorKeys]::VK_X)
  Start-Sleep -Milliseconds 400
  $text = ""
  try {
    $text = [System.Windows.Forms.Clipboard]::GetText()
  } catch {
    return ""
  }
  if (-not $text) { return "" }
  if ($text -eq $sentinel) { return "" }
  if ($text.StartsWith("LEGION_NARRATOR_SENTINEL_")) { return "" }
  return $text
}

[void][NarratorKeys]::ShowWindow($legionHwnd, 9)
[void][NarratorKeys]::SetForegroundWindow($legionHwnd)
Start-Sleep -Milliseconds 800

$spoken = New-Object "System.Collections.Generic.List[string]"
function Test-EnvironmentalUtterance([string]$phrase) {
  # CapsLock chords can make Narrator announce the system volume HUD.
  return ($phrase -match '(?i)volume level')
}

function Add-Spoken([string]$phrase) {
  if (-not $phrase) { return }
  $trimmed = $phrase.Trim()
  if ($trimmed.Length -lt 1) { return }
  if (Test-EnvironmentalUtterance $trimmed) { return }
  $spoken.Add($trimmed)
}

function Test-LegionProductSpeech([string]$phrase) {
  if ($windowName -and $phrase.ToLower().Contains($windowName.ToLower())) {
    return $true
  }
  foreach ($needle in @(
      "Manual, button",
      "Assist, button",
      "Delegate, button",
      "Legion Workflows",
      "Explorer drawer",
      "Bottom panel drawer",
      "PROBLEMS"
    )) {
    if ($phrase.ToLower().Contains($needle.ToLower())) { return $true }
  }
  return $false
}

# Narrator+W reads the current window. CapsLock is the default Narrator key.
Send-NarratorChord @([NarratorKeys]::VK_W)
Start-Sleep -Seconds 2
Add-Spoken (Copy-LastSpokenPhrase)

# Tab through the top chrome; copy each last-spoken phrase from Narrator.
for ($i = 0; $i -lt 10; $i++) {
  [void][NarratorKeys]::SetForegroundWindow($legionHwnd)
  [NarratorKeys]::Tap([NarratorKeys]::VK_TAB)
  Start-Sleep -Milliseconds 500
  Add-Spoken (Copy-LastSpokenPhrase)
}

# Narrator+Alt+X opens Speech Recap (spoken-history window).
Send-NarratorChord @([NarratorKeys]::VK_MENU, [NarratorKeys]::VK_X)
Start-Sleep -Seconds 2

$recapFound = $false
$trueCond = [System.Windows.Automation.Condition]::TrueCondition
$narratorProcs = @(Get-Process -Name Narrator -ErrorAction SilentlyContinue)
foreach ($np in $narratorProcs) {
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $np.Id)
  $wins = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
  foreach ($w in $wins) {
    $n = $w.Current.Name
    if ($n -match "Speech recap") {
      $recapFound = $true
      $desc = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, $trueCond)
      for ($i = 0; $i -lt $desc.Count; $i++) {
        $name = $desc.Item($i).Current.Name
        if ($name) { Add-Spoken $name }
      }
    }
  }
}

$unique = New-Object "System.Collections.Generic.List[string]"
$seen = @{}
foreach ($line in $spoken) {
  if (-not $seen.ContainsKey($line)) {
    $seen[$line] = $true
    $unique.Add($line)
  }
}

$os = (Get-CimInstance Win32_OperatingSystem)
$narratorFile = Get-Item "$env:WINDIR\System32\Narrator.exe"
$sha = (git rev-parse HEAD).Trim()
$when = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")

$lines = New-Object "System.Collections.Generic.List[string]"
[void]$lines.Add("AT=Windows Narrator")
[void]$lines.Add("AT_VERSION=$($narratorFile.VersionInfo.FileVersion)")
[void]$lines.Add("OS=$($os.Caption) $($os.Version)")
[void]$lines.Add("ARCH=$([System.Environment]::OSVersion.Platform) $env:PROCESSOR_ARCHITECTURE")
[void]$lines.Add("GIT_SHA=$sha")
[void]$lines.Add("CAPTURED_AT_UTC=$when")
[void]$lines.Add("WINDOW_TITLE=$windowName")
[void]$lines.Add("PROCESS=$ProcName")
[void]$lines.Add("SPEECH_RECAP_WINDOW=$recapFound")
[void]$lines.Add("PROBE=scripts/a11y-narrator-transcript.ps1")
[void]$lines.Add("UTTERANCE_COUNT=$($unique.Count)")
[void]$lines.Add("TRANSCRIPT_BEGIN")
foreach ($line in $unique) { [void]$lines.Add($line) }
[void]$lines.Add("TRANSCRIPT_END")

$text = ($lines -join "`n") + "`n"
Write-Output $text

if ($OutFile) {
  $dir = Split-Path -Parent $OutFile
  if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
  $utf8 = New-Object System.Text.UTF8Encoding $false
  [System.IO.File]::WriteAllText($OutFile, $text, $utf8)
}

if ($startedNarrator) {
  Get-Process -Name Narrator -ErrorAction SilentlyContinue | ForEach-Object {
    try { Stop-Process -Id $_.Id -Force -ErrorAction Stop } catch { }
  }
}

$legionSpoken = 0
foreach ($line in $unique) {
  if (Test-LegionProductSpeech $line) { $legionSpoken++ }
}
if ($unique.Count -lt 1 -or $legionSpoken -lt 1) {
  Write-Output "NARRATOR_NO_SPEECH"
  exit 6
}

Write-Output "NARRATOR_TRANSCRIPT_OK"
exit 0
