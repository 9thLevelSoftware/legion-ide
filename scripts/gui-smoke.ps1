# GUI Phase 6 desktop smoke wrapper.

[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$Beta,
    [string]$Workspace = ".",
    [string]$BetaWorkspace = "target/gui-phase7-beta-workspace",
    [string]$File = "",
    [int]$DurationMs = 1500,
    [string]$Evidence = "plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md",
    [string]$SessionState = "target/gui-phase6-session.json",
    [string]$DiagnosticsExport = "target/gui-phase6-diagnostics.md"
)

$ErrorActionPreference = "Stop"

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

$ArgsList = @(
    "run", "-p", "devil-desktop", "--",
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
