# Windows UI Automation walk of a running legion-desktop window.
#
# WHY THIS EXISTS
# ---------------
# Nothing else in this repository can observe an OS accessibility tree on any
# platform. `legion-desktop --smoke` reports projection metadata only; its
# `accessibility_tree_status` (crates/legion-desktop/src/platform.rs) is
# hardcoded to emit "OS tree not observed" for every non-zero node count, so
# every document quoting the smoke evidence correctly says the OS tree was not
# observed. The macOS AX inspection recorded in
# plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md was performed by
# an external probe whose source was never committed, so it cannot be repeated.
#
# This script is the Windows leg, committed so the observation is repeatable.
#
# SCOPE: Windows only. It does NOT close the 3-OS accessibility gap for
# PR-UI-001. macOS (AXUIElement) and Linux (AT-SPI) still need equivalent
# committed probes. See
# plans/evidence/production/PR-UI-001/2026-08-16-promotion-verification.md
#
# USAGE
# -----
#   1. Start a window, e.g.
#      cargo run -p legion-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 45000
#   2. While it is up:
#      powershell -NoProfile -ExecutionPolicy Bypass -File scripts/a11y-uia-walk.ps1
#
# Exit codes: 0 walked; 3 UIA assemblies unavailable; 4 process not running;
# 5 process running but publishing no top-level UIA window.
param([string]$ProcName = "legion-desktop")

$ErrorActionPreference = "Stop"
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
Write-Output ("PROCESS_FOUND: {0} pid(s): {1}" -f $procs.Count, (($procs | ForEach-Object { $_.Id }) -join ","))

$root = [System.Windows.Automation.AutomationElement]::RootElement
$found = 0

foreach ($p in $procs) {
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $p.Id)
  $wins = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
  Write-Output ("PID {0}: top-level UIA windows = {1}" -f $p.Id, $wins.Count)
  foreach ($w in $wins) {
    $found++
    Write-Output ("WINDOW name='{0}' controlType='{1}' className='{2}' isEnabled={3} hasKeyboardFocus={4}" -f `
      $w.Current.Name, $w.Current.ControlType.ProgrammaticName, $w.Current.ClassName, `
      $w.Current.IsEnabled, $w.Current.HasKeyboardFocus)

    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $stack = New-Object System.Collections.Stack
    $stack.Push(@($w, 1))
    $count = 0
    while ($stack.Count -gt 0 -and $count -lt 400) {
      $item = $stack.Pop()
      $el = $item[0]; $depth = $item[1]
      $child = $walker.GetFirstChild($el)
      while ($child -ne $null) {
        $count++
        $pad = "  " * $depth
        Write-Output ("{0}[{1}] {2} name='{3}' automationId='{4}'" -f `
          $pad, $depth, $child.Current.ControlType.ProgrammaticName, `
          $child.Current.Name, $child.Current.AutomationId)
        if ($depth -lt 5) { $stack.Push(@($child, $depth + 1)) }
        $child = $walker.GetNextSibling($child)
      }
    }
    Write-Output ("DESCENDANTS_ENUMERATED: {0}" -f $count)
  }
}

if ($found -eq 0) { Write-Output "NO_TOPLEVEL_WINDOW_FOR_PROCESS"; exit 5 }
Write-Output "UIA_WALK_OK"
exit 0
