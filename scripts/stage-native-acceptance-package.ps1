# Stage the packaged Legion product for the native input acceptance harness.
#
# This is a test-instrument staging step. It is not part of the release path,
# no workflow references it, and it never builds, signs or publishes anything.
# Its only job is to put the product payload that `msiexec /a` extracted from a
# verified MSI where `xtask native-product-acceptance` looks for it, and to
# leave behind a machine-readable record of exactly which artifact was staged.
#
# Refusals (each exits non-zero with an `error=` line and copies nothing):
#   * the MSI's recomputed SHA-256 does not match its `.sha256` sidecar;
#   * the resolved source path lies under a cargo build directory, recognised
#     two ways: a `debug` or `release` path segment whose parent segment name
#     ends in `target` (`<...>/target/debug/...`, `<...>/target/release/...`,
#     and the packager's own `<...>/target/native-package/cargo-target/release`,
#     which is where `scripts/package-native.ps1` points `CARGO_TARGET_DIR`), or
#     a `debug`/`release` ancestor directory carrying cargo's `.fingerprint`
#     marker, which catches a build directory under any other name. This is a
#     name-and-marker guard over the resolved path, not a proof of provenance:
#     it stops the convenient mistakes -- handing the harness the development
#     build or the packager's own release build -- but a payload deliberately
#     copied somewhere else first would still pass it. What actually ties the
#     staged payload to a specific artifact is the MSI hash check above and the
#     hashes recorded in `STAGING-EVIDENCE.toml`;
#   * the SHA-256 of the extracted `legion-desktop.exe` does not match the
#     `legion-desktop.exe.sha256` sidecar beside the MSI, so a payload that
#     did not come from that installer cannot be staged beside its hash;
#   * `-DestinationDir` is the same as, contains, or is nested inside
#     `-PackageDir`, `-StagingSource`, or the payload directory;
#   * the extraction tree does not contain exactly one `legion-desktop.exe`.
#
# `target/release-smoke/...` is deliberately not a development build directory:
# the refusal matches whole path segments, so the verifier's own extraction
# directory stages normally while `target/release` and the packager's
# `cargo-target/release` never do.
#
# The staged artifact is unsigned. `STAGING-EVIDENCE.toml` records
# `signed = false` together with the signer status copied verbatim from
# `RELEASE-METADATA.toml` and the exact prerequisite a signed artifact needs.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$PackageDir,
    [Parameter(Mandatory = $true)]
    [string]$StagingSource,
    [string]$DestinationDir = "target/native-input-acceptance/package",
    [Alias("WhatIf")]
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Stem = "legion-desktop-windows-x64-msi"
$ExecutableName = "legion-desktop.exe"
$EvidenceName = "STAGING-EVIDENCE.toml"
$SigningPrerequisite = "Owner-supplied signing, notarization and update-feed infrastructure; no signing credential, certificate, key, notarization tool, provider or feed is available in the retained facts."
$CleanMachinePrerequisite = "A clean virtual machine for each supported OS with no prior Legion installation."
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Fail([string]$Message) {
    Write-Host "error=$Message"
    exit 1
}

function Resolve-FullPath([string]$Path) {
    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $RepoRoot $Path))
}

function ConvertTo-TomlString([string]$Value) {
    return $Value.Replace("\", "\\").Replace('"', '\"')
}

# True when the path contains a `debug` or `release` whole segment whose parent
# segment name ends in `target`. That covers `target/debug` and `target/release`
# and also `target/native-package/cargo-target/release`, the directory
# `scripts/package-native.ps1` actually builds into after it redirects
# `CARGO_TARGET_DIR`. Segment matching, not substring matching, is what keeps
# `target/release-smoke` -- the verifier's own extraction root -- stageable.
function Test-DevelopmentBuildPath([string]$Path) {
    $normalized = ($Path -replace '\\', '/').TrimEnd('/')
    $segments = $normalized.Split('/')
    for ($index = 0; $index -lt $segments.Length - 1; $index++) {
        if (-not $segments[$index].ToLowerInvariant().EndsWith("target")) {
            continue
        }
        $next = $segments[$index + 1].ToLowerInvariant()
        if ($next -eq "debug" -or $next -eq "release") {
            return $true
        }
    }
    return $false
}

# The path-name rule above only knows the names cargo target directories usually
# carry. This one asks the filesystem instead: a `debug` or `release` directory
# holding cargo's `.fingerprint` bookkeeping directory is a cargo build
# directory whatever its parent is called. Returns the offending ancestor, or
# $null.
function Get-CargoBuildTreeAncestor([string]$Path) {
    $current = $Path
    while (-not [string]::IsNullOrEmpty($current)) {
        $leaf = (Split-Path -Leaf $current).ToLowerInvariant()
        if (($leaf -eq "debug" -or $leaf -eq "release") -and
            (Test-Path -LiteralPath (Join-Path $current ".fingerprint") -PathType Container)) {
            return $current
        }
        $parent = Split-Path -Parent $current
        if ([string]::IsNullOrEmpty($parent) -or $parent -eq $current) {
            break
        }
        $current = $parent
    }
    return $null
}

function Assert-NotDevelopmentBuild([string]$Label, [string]$Path) {
    if (Test-DevelopmentBuildPath $Path) {
        Fail ("Refusing to stage a development build: $Label $Path lies under a cargo build directory -- " +
            "a debug or release directory whose parent name ends in 'target', which covers " +
            "target/debug or target/release and also the packager's own " +
            "target/native-package/cargo-target/release. " +
            "Stage only a payload extracted from a verified installer.")
    }
    $cargoTree = Get-CargoBuildTreeAncestor $Path
    if ($null -ne $cargoTree) {
        Fail ("Refusing to stage a development build: $Label $Path lies under $cargoTree, " +
            "a cargo build directory carrying a .fingerprint marker. " +
            "Stage only a payload extracted from a verified installer.")
    }
}

function Test-PathIsAtOrAbove([string]$Ancestor, [string]$Descendant) {
    $a = ($Ancestor -replace '\\', '/').TrimEnd('/').ToLowerInvariant()
    $d = ($Descendant -replace '\\', '/').TrimEnd('/').ToLowerInvariant()
    if ($a -eq $d) {
        return $true
    }
    return $d.StartsWith("$a/", [System.StringComparison]::Ordinal)
}

function Get-MetadataValue([string[]]$Lines, [string]$Key) {
    foreach ($line in $Lines) {
        if ($line -match "^\s*$([regex]::Escape($Key))\s*=\s*`"(.*)`"\s*$") {
            return $Matches[1]
        }
    }
    return $null
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

$PackageDir = Resolve-FullPath $PackageDir
$StagingSource = Resolve-FullPath $StagingSource
$DestinationDir = Resolve-FullPath $DestinationDir

$msiPath = Join-Path $PackageDir "$Stem.msi"
$checksumPath = "$msiPath.sha256"
$metadataPath = Join-Path $PackageDir "RELEASE-METADATA.toml"

foreach ($required in @($msiPath, $checksumPath, $metadataPath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        Fail "Missing required package file: $required"
    }
}

# The hash comes first: nothing is inspected, resolved or copied until the MSI
# in hand is provably the MSI its sidecar names.
$checksumLine = (Get-Content -LiteralPath $checksumPath -Raw).Replace("`r", "").Trim()
if ($checksumLine -notmatch '^([0-9a-f]{64}) \*(.+)$') {
    Fail "Malformed checksum file: $checksumPath"
}
$expectedHash = $Matches[1]
$checksumName = $Matches[2]
if ($checksumName -ne (Split-Path -Leaf $msiPath)) {
    Fail "Checksum names unexpected installer: expected $(Split-Path -Leaf $msiPath), found $checksumName"
}
$actualHash = Get-Sha256 $msiPath
if ($actualHash -ne $expectedHash) {
    Fail "MSI checksum mismatch: expected $expectedHash, computed $actualHash for $msiPath"
}

if (-not (Test-Path -LiteralPath $StagingSource -PathType Container)) {
    Fail "Staging source directory does not exist: $StagingSource"
}

Assert-NotDevelopmentBuild "staging source" $StagingSource

$metadataLines = @(Get-Content -LiteralPath $metadataPath)
$releaseVersion = Get-MetadataValue $metadataLines "release_version"
$signerStatus = Get-MetadataValue $metadataLines "signer_status"
$gitSha = Get-MetadataValue $metadataLines "git_sha"
foreach ($pair in @(
    @{ Key = "release_version"; Value = $releaseVersion },
    @{ Key = "signer_status"; Value = $signerStatus },
    @{ Key = "git_sha"; Value = $gitSha }
)) {
    if ([string]::IsNullOrEmpty($pair.Value)) {
        Fail "RELEASE-METADATA.toml is missing $($pair.Key): $metadataPath"
    }
}

$candidates = @(
    Get-ChildItem -LiteralPath $StagingSource -Recurse -File -Filter $ExecutableName |
        Sort-Object FullName
)
if ($candidates.Count -ne 1) {
    $found = if ($candidates.Count -eq 0) { "<none>" } else { ($candidates.FullName -join ", ") }
    Fail "Expected exactly one $ExecutableName under $StagingSource; found $($candidates.Count): $found"
}
$sourceExecutable = $candidates[0].FullName
$payloadDir = (Split-Path -Parent $sourceExecutable)

Assert-NotDevelopmentBuild "payload directory" $payloadDir

$payloadSidecar = Join-Path $PackageDir "$ExecutableName.sha256"
if (-not (Test-Path -LiteralPath $payloadSidecar -PathType Leaf)) {
    Fail "Missing payload checksum sidecar: $payloadSidecar"
}
$payloadChecksumLine = (Get-Content -LiteralPath $payloadSidecar -Raw).Replace("`r", "").Trim()
if ($payloadChecksumLine -notmatch '^([0-9a-f]{64}) \*(.+)$') {
    Fail "Malformed payload checksum file: $payloadSidecar"
}
$expectedPayloadHash = $Matches[1]
$payloadChecksumName = $Matches[2]
if ($payloadChecksumName -ne $ExecutableName) {
    Fail "Payload checksum names unexpected executable: expected $ExecutableName, found $payloadChecksumName"
}
$sourceExecutableHash = Get-Sha256 $sourceExecutable
if ($sourceExecutableHash -ne $expectedPayloadHash) {
    Fail "Staged payload is not the verified MSI content: expected $expectedPayloadHash, computed $sourceExecutableHash for $sourceExecutable"
}

if ($null -eq (Split-Path -Parent $DestinationDir) -or (Split-Path -Parent $DestinationDir) -eq "") {
    Fail "Refusing to stage into a filesystem root: $DestinationDir"
}
foreach ($guarded in @($PackageDir, $StagingSource, $payloadDir)) {
    if (Test-PathIsAtOrAbove $DestinationDir $guarded) {
        Fail "Refusing to stage into $DestinationDir because it contains the source path $guarded"
    }
    if (Test-PathIsAtOrAbove $guarded $DestinationDir) {
        Fail "Refusing to stage into $DestinationDir because it is nested inside the source path $guarded"
    }
}

$payloadFiles = @(Get-ChildItem -LiteralPath $payloadDir -Recurse -File | Sort-Object FullName)
$stagedExecutable = Join-Path $DestinationDir $ExecutableName
$sourceExecutableHash = Get-Sha256 $sourceExecutable
$msiBytes = (Get-Item -LiteralPath $msiPath).Length

if ($DryRun) {
    Write-Host "plan=stage-native-acceptance-package"
    Write-Host "source_msi=$msiPath"
    Write-Host "source_msi_sha256=$actualHash"
    Write-Host "payload_source_dir=$payloadDir"
    Write-Host "payload_file_count=$($payloadFiles.Count)"
    Write-Host "destination_dir=$DestinationDir"
    Write-Host "staged_executable=$stagedExecutable"
    Write-Host "signed=false"
    exit 0
}

# A whole-directory replace, so a second run over the same inputs produces the
# same destination contents rather than accumulating stale files beside them.
if (Test-Path -LiteralPath $DestinationDir) {
    Remove-Item -LiteralPath $DestinationDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $DestinationDir | Out-Null

foreach ($file in $payloadFiles) {
    $relative = $file.FullName.Substring($payloadDir.Length).TrimStart([char]'\', [char]'/')
    $target = Join-Path $DestinationDir $relative
    $targetParent = Split-Path -Parent $target
    if (-not (Test-Path -LiteralPath $targetParent -PathType Container)) {
        New-Item -ItemType Directory -Force -Path $targetParent | Out-Null
    }
    Copy-Item -LiteralPath $file.FullName -Destination $target -Force
}

if (-not (Test-Path -LiteralPath $stagedExecutable -PathType Leaf)) {
    Fail "Staging completed without producing $stagedExecutable"
}
$stagedExecutableHash = Get-Sha256 $stagedExecutable
if ($stagedExecutableHash -ne $sourceExecutableHash) {
    Fail "Staged executable hash $stagedExecutableHash does not match extracted executable hash $sourceExecutableHash"
}

$evidenceLines = @(
    "schema_version = 1"
    'staged_by = "scripts/stage-native-acceptance-package.ps1"'
    'classification = "unsigned local package staged for the native input acceptance harness; not release qualification and not product acceptance"'
    "staged_utc = `"$([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))`""
    "source_msi = `"$(ConvertTo-TomlString $msiPath)`""
    "source_msi_sha256 = `"$actualHash`""
    "source_msi_bytes = $msiBytes"
    "release_version = `"$(ConvertTo-TomlString $releaseVersion)`""
    "signer_status = `"$(ConvertTo-TomlString $signerStatus)`""
    "git_sha = `"$(ConvertTo-TomlString $gitSha)`""
    "payload_source_dir = `"$(ConvertTo-TomlString $payloadDir)`""
    "payload_file_count = $($payloadFiles.Count)"
    "destination_dir = `"$(ConvertTo-TomlString $DestinationDir)`""
    "staged_executable = `"$(ConvertTo-TomlString $stagedExecutable)`""
    "staged_executable_sha256 = `"$stagedExecutableHash`""
    "signed = false"
    "signing_prerequisite = `"$(ConvertTo-TomlString $SigningPrerequisite)`""
    "clean_machine_prerequisite = `"$(ConvertTo-TomlString $CleanMachinePrerequisite)`""
    'acceptance_effect = "none; this script runs no acceptance harness and moves no acceptance value"'
)
[System.IO.File]::WriteAllText(
    (Join-Path $DestinationDir $EvidenceName),
    (($evidenceLines -join "`n") + "`n"),
    $Utf8NoBom
)

Write-Host "staged=$DestinationDir"
Write-Host "staged_executable=$stagedExecutable"
Write-Host "staged_executable_sha256=$stagedExecutableHash"
Write-Host "source_msi_sha256=$actualHash"
Write-Host "payload_file_count=$($payloadFiles.Count)"
Write-Host "signed=false"
exit 0
