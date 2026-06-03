# GUI desktop smoke wrapper.

[CmdletBinding()]
param(
    [switch]$Help,
    [switch]$DryRun,
    [switch]$Beta,
    [switch]$Phase8,
    [string]$Workspace = ".",
    [string]$BetaWorkspace = "target/gui-phase7-beta-workspace",
    [string]$File = "",
    [int]$DurationMs = 1500,
    [string]$Evidence = "plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md",
    [string]$SessionState = "target/gui-phase6-session.json",
    [string]$DiagnosticsExport = "target/gui-phase6-diagnostics.md"
)

$ErrorActionPreference = "Stop"

function Write-GuiSmokeHelp {
    Write-Host "GUI smoke wrapper"
    Write-Host ""
    Write-Host "Usage:"
    Write-Host "  powershell -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 [-DryRun] [-Beta|-Phase8] [-Workspace <path>] [-File <path>]"
    Write-Host ""
    Write-Host "Modes:"
    Write-Host "  default  GUI Phase 6 desktop smoke evidence"
    Write-Host "  -Beta    GUI Phase 7 local beta smoke evidence"
    Write-Host "  -Phase8  GUI phase-8 advanced surface smoke evidence"
    Write-Host ""
    Write-Host "Phase 8 defaults:"
    Write-Host "  Evidence: plans/evidence/gui-productization/phase-8-advanced-surface-smoke.md"
    Write-Host "  Session state: target/gui-phase8-session.json"
    Write-Host "  Diagnostics export: target/gui-phase8-diagnostics.md"
}

if ($Help) {
    Write-GuiSmokeHelp
    exit 0
}

if ($Beta -and $Phase8) {
    throw "Choose either -Beta or -Phase8, not both."
}

if ($Beta) {
    if ($PSBoundParameters.ContainsKey("Evidence") -eq $false) {
        $Evidence = "plans/evidence/gui-productization/phase-7-local-workflow-smoke.md"
    }
    if ($PSBoundParameters.ContainsKey("SessionState") -eq $false) {
        $SessionState = "target/gui-phase7-session.json"
    }
    if ($PSBoundParameters.ContainsKey("DiagnosticsExport") -eq $false) {
        $DiagnosticsExport = "target/gui-phase7-diagnostics.md"
    }
}

if ($Phase8) {
    if ($PSBoundParameters.ContainsKey("Evidence") -eq $false) {
        $Evidence = "plans/evidence/gui-productization/phase-8-advanced-surface-smoke.md"
    }
    if ($PSBoundParameters.ContainsKey("SessionState") -eq $false) {
        $SessionState = "target/gui-phase8-session.json"
    }
    if ($PSBoundParameters.ContainsKey("DiagnosticsExport") -eq $false) {
        $DiagnosticsExport = "target/gui-phase8-diagnostics.md"
    }
}

$ArgsList = @(
    "run", "-p", "legion-desktop", "--",
    "--workspace", $Workspace,
    "--evidence", $Evidence,
    "--session-state", $SessionState,
    "--diagnostics-export", $DiagnosticsExport
)

if ($Beta) {
    $ArgsList += @("--beta-smoke", "--beta-workspace", $BetaWorkspace)
} else {
    $ArgsList += @("--smoke", "--duration-ms", "$DurationMs")
}

if (-not [string]::IsNullOrWhiteSpace($File)) {
    $ArgsList += @("--file", $File)
}

if ($Beta) {
    Write-Host "GUI Phase 7 beta smoke plan"
    Write-Host "Beta workspace: $BetaWorkspace"
} elseif ($Phase8) {
    Write-Host "GUI Phase 8 advanced surface smoke plan"
} else {
    Write-Host "GUI Phase 6 smoke plan"
}
Write-Host "Cargo command: cargo $($ArgsList -join ' ')"
Write-Host "Evidence: $Evidence"
Write-Host "Session state: $SessionState"
Write-Host "Diagnostics export: $DiagnosticsExport"

if ($DryRun) {
    Write-Host "Dry run: smoke command was not executed."
    exit 0
}

cargo @ArgsList
