# WS-A-D Phase 4 D1: build portable unsigned-beta preview bundle (Windows).
#
# Produces:
#   target/preview/windows-x64/legion-desktop.exe
#   target/preview/windows-x64/UNSIGNED-BETA.toml
#   target/preview/windows-x64/package-manifest.txt
#   target/preview/legion-desktop-preview-windows-x64.zip

[CmdletBinding()]
param(
    [switch]$Release,
    [string]$OutRoot = "target/preview"
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Profile = if ($Release) { "release" } else { "debug" }
$Arch = "x64"
$BundleDir = Join-Path $RepoRoot (Join-Path $OutRoot "windows-$Arch")
$SourceExe = Join-Path (Join-Path (Join-Path $RepoRoot "target") $Profile) "legion-desktop.exe"
$DestExe = Join-Path $BundleDir "legion-desktop.exe"
$Manifest = Join-Path $BundleDir "package-manifest.txt"
$Unsigned = Join-Path $BundleDir "UNSIGNED-BETA.toml"
$ZipPath = Join-Path (Join-Path $RepoRoot $OutRoot) "legion-desktop-preview-windows-$Arch.zip"

$CargoArgs = @("build", "-p", "legion-desktop")
if ($Release) {
    $CargoArgs += "--release"
}

Write-Host "Legion preview package (unsigned-beta)"
Write-Host "Repository: $RepoRoot"
Write-Host "Profile: $Profile"
Write-Host "Bundle: $BundleDir"

Push-Location $RepoRoot
try {
    cargo @CargoArgs
} finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $SourceExe -PathType Leaf)) {
    throw "Expected desktop executable was not produced: $SourceExe"
}

New-Item -ItemType Directory -Force -Path $BundleDir | Out-Null
Copy-Item -LiteralPath $SourceExe -Destination $DestExe -Force

$GitSha = "unknown"
try {
    $GitSha = (git -C $RepoRoot rev-parse HEAD 2>$null).Trim()
} catch {}
$BuiltAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$UnsignedText = @"
schema_version = 1
package = "legion-desktop"
channel = "preview"
profile = "$Profile"
platform = "windows"
arch = "$Arch"
git_sha = "$GitSha"
built_at_utc = "$BuiltAt"
signer_status = "unsigned-beta/no-os-code-signing"
os_code_signing = false
production = false
notes = "Portable unsigned preview (WS-A-D D2: unsigned-beta retained). Not Authenticode-signed. Do not distribute as a production release."
policy_ref = "plans/evidence/production/WS-A-D/phase-4-release/D2-unsigned-beta-retained.md"
"@
Set-Content -LiteralPath $Unsigned -Value $UnsignedText -Encoding utf8

$ManifestText = @(
    "package: legion-desktop",
    "channel: preview",
    "platform: windows",
    "arch: $Arch",
    "profile: $Profile",
    "git_sha: $GitSha",
    "built_at_utc: $BuiltAt",
    "signer_status: unsigned-beta/no-os-code-signing",
    "package_executable: $DestExe"
) -join "`r`n"
Set-Content -LiteralPath $Manifest -Value $ManifestText -Encoding utf8

if (Test-Path -LiteralPath $ZipPath) {
    Remove-Item -LiteralPath $ZipPath -Force
}
Compress-Archive -Path (Join-Path $BundleDir '*') -DestinationPath $ZipPath -Force

Write-Host "Wrote $Unsigned"
Write-Host "Wrote $Manifest"
Write-Host "Wrote $ZipPath"
