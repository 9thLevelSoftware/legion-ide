# GUI Phase 6 desktop packaging wrapper.

[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$Release,
    [string]$OutDir = "target/gui-phase6-package"
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Profile = if ($Release) { "release" } else { "debug" }
$PackageDir = Join-Path $RepoRoot $OutDir
$SourceExe = Join-Path (Join-Path (Join-Path $RepoRoot "target") $Profile) "legion-desktop.exe"
$DestExe = Join-Path $PackageDir "legion-desktop.exe"
$Manifest = Join-Path $PackageDir "legion-desktop-package-manifest.txt"
$CargoArgs = @("build", "-p", "legion-desktop")
if ($Release) {
    $CargoArgs += "--release"
}

Write-Host "GUI Phase 6 Windows package plan"
Write-Host "Repository: $RepoRoot"
Write-Host "Profile: $Profile"
Write-Host "Output: $PackageDir"
Write-Host "Executable source: $SourceExe"
Write-Host "Executable destination: $DestExe"
Write-Host "Cargo command: cargo $($CargoArgs -join ' ')"

if ($DryRun) {
    Write-Host "Dry run: no build, copy, or package output was written."
    exit 0
}

cargo @CargoArgs

if (-not (Test-Path -LiteralPath $SourceExe -PathType Leaf)) {
    throw "Expected desktop executable was not produced: $SourceExe"
}

New-Item -ItemType Directory -Force -Path $PackageDir | Out-Null
Copy-Item -LiteralPath $SourceExe -Destination $DestExe -Force

$ManifestText = @(
    "package: legion-desktop",
    "platform: windows",
    "profile: $Profile",
    "dry_run: false",
    "cargo_command: cargo $($CargoArgs -join ' ')",
    "source_executable: $SourceExe",
    "package_directory: $PackageDir",
    "package_executable: $DestExe"
) -join "`r`n"

Set-Content -LiteralPath $Manifest -Value $ManifestText -Encoding UTF8
Write-Host "Package manifest: $Manifest"
