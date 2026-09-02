[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [Parameter(Mandatory = $true)]
    [ValidateSet("wix", "dmg", "deb", "appimage")]
    [string]$Format,
    [ValidateSet("default", "manual")]
    [string]$Sku = "default",
    [Alias("OutputDir", "Out")]
    [string]$OutDir = "target/native-package/output",
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

if ($Version -notmatch '^0\.0\.[1-9][0-9]*$') {
    throw "-Version must be canonical 0.0.N with N at least 1 and no zero padding"
}
if ($Format -ne "wix") {
    throw "Windows native packaging supports only -Format wix"
}

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$NativeDir = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot "target/native-package"))
$OutputDir = if ([System.IO.Path]::IsPathRooted($OutDir)) {
    [System.IO.Path]::GetFullPath($OutDir)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $RepoRoot $OutDir))
}
$BinariesDir = Join-Path $NativeDir "cargo-target/release"
$PackagingDir = Join-Path $RepoRoot "packaging"
$ConfigPath = Join-Path $NativeDir "Packager.toml"
$Platform = "windows"
$Architecture = "x64"
$StagingDir = Join-Path $NativeDir ("packager-" + [Guid]::NewGuid().ToString("N"))
$PackageFormat = "msi"
$Extension = "msi"
$PackageName = "legion-desktop-$Platform-$Architecture-$PackageFormat.$Extension"
$PackagePath = Join-Path $OutputDir $PackageName
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function ConvertTo-TomlString([string]$Value) {
    return $Value.Replace("\", "\\").Replace('"', '\"')
}

function Assert-X64PackageHost {
    $osArchitecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    if ($osArchitecture -ne [System.Runtime.InteropServices.Architecture]::X64) {
        throw "Windows native packaging requires an x64 host; detected $osArchitecture."
    }
}

function Assert-X64Executable([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $reader = New-Object System.IO.BinaryReader($stream)
        try {
            if ($reader.ReadUInt16() -ne 0x5A4D) {
                throw "Expected a PE executable: $Path"
            }
            $stream.Position = 0x3C
            $peOffset = $reader.ReadUInt32()
            $stream.Position = $peOffset + 4
            if ($reader.ReadUInt16() -ne 0x8664) {
                throw "Expected an x64 executable: $Path"
            }
        } finally {
            $reader.Dispose()
        }
    } finally {
        $stream.Dispose()
    }
}

function Render-PackagerConfig {
    New-Item -ItemType Directory -Force -Path $NativeDir | Out-Null
    $template = Get-Content -LiteralPath (Join-Path $PackagingDir "Packager.toml") -Raw
    $config = $template.Replace('version = "0.0.0"', ('version = "{0}"' -f (ConvertTo-TomlString $Version)))
    $config = $config.Replace("__FORMAT__", (ConvertTo-TomlString $Format))
    $config = $config.Replace("__BINARIES_DIR__", (ConvertTo-TomlString $BinariesDir))
    $config = $config.Replace("__OUT_DIR__", (ConvertTo-TomlString $StagingDir))
    $config = $config.Replace("__PACKAGING_DIR__", (ConvertTo-TomlString $PackagingDir))
    $config = $config.Replace("__REPO_ROOT__", (ConvertTo-TomlString $RepoRoot))
    [System.IO.File]::WriteAllText($ConfigPath, $config, $Utf8NoBom)
}

Assert-X64PackageHost
Render-PackagerConfig
if ($DryRun) {
    Write-Host "Planned package: $PackagePath"
    exit 0
}

if (Test-Path -LiteralPath $PackagePath) {
    throw "Refusing to overwrite existing package: $PackagePath"
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
New-Item -ItemType Directory -Path $StagingDir | Out-Null
$originalTargetDir = $env:CARGO_TARGET_DIR
try {
    $env:CARGO_TARGET_DIR = Join-Path $NativeDir "cargo-target"
    Push-Location $RepoRoot
    try {
        if ($Sku -eq "manual") {
            cargo build --release -p legion-desktop --no-default-features --features offline
        } else {
            cargo build --release -p legion-desktop
        }
        Assert-X64Executable (Join-Path $BinariesDir "legion-desktop.exe")
        New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null
        Copy-Item -LiteralPath (Join-Path $RepoRoot "LICENSE") -Destination (Join-Path $BinariesDir "LICENSE") -Force
        Copy-Item -LiteralPath (Join-Path $RepoRoot "docs\PRIVACY.md") -Destination (Join-Path $BinariesDir "PRIVACY.md") -Force
        Copy-Item -LiteralPath (Join-Path $RepoRoot "THIRD_PARTY_NOTICES.md") -Destination (Join-Path $BinariesDir "THIRD_PARTY_NOTICES.md") -Force
        cargo packager --release --config $ConfigPath
    } finally {
        Pop-Location
    }

    $candidates = @(Get-ChildItem -LiteralPath $StagingDir -Recurse -File -Filter "*.$Extension")
    if ($candidates.Count -ne 1) {
        throw "Expected exactly one .$Extension package in $StagingDir; found $($candidates.Count)"
    }
    Move-Item -LiteralPath $candidates[0].FullName -Destination $PackagePath

    $workspaceVersion = (Select-String -LiteralPath (Join-Path $RepoRoot "Cargo.toml") -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value
    $gitSha = "unknown"
    try {
        $gitSha = (git -C $RepoRoot rev-parse HEAD 2>$null).Trim()
    } catch {}
    $metadataLines = @(
        "release_version = `"$Version`""
        "workspace_version = `"$workspaceVersion`""
        "git_sha = `"$gitSha`""
        "platform = `"$Platform`""
        "architecture = `"$Architecture`""
        "format = `"$Format`""
        "sku = `"$Sku`""
        'signer_status = "unsigned-beta/no-os-code-signing"'
    )
    $metadata = ($metadataLines -join "`n") + "`n"
    [System.IO.File]::WriteAllText((Join-Path $OutputDir "RELEASE-METADATA.toml"), $metadata, $Utf8NoBom)

    $hash = (Get-FileHash -LiteralPath $PackagePath -Algorithm SHA256).Hash.ToLowerInvariant()
    [System.IO.File]::WriteAllText(
        "$PackagePath.sha256",
        "$hash *$PackageName",
        [System.Text.Encoding]::ASCII
    )

    Write-Host "Wrote $PackagePath"
    Write-Host "Wrote $(Join-Path $OutputDir 'RELEASE-METADATA.toml')"
    Write-Host "Wrote $PackagePath.sha256"
} finally {
    $env:CARGO_TARGET_DIR = $originalTargetDir
    if (Test-Path -LiteralPath $StagingDir) {
        [System.IO.Directory]::Delete($StagingDir, $true)
    }
}
