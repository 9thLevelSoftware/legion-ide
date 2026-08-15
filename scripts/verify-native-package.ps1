# Verify the packaged Legion Windows MSI end to end.
#
# Covers: artifact existence, SHA-256 checksum, generated release metadata,
# MSI ProductVersion (Windows Installer Automation, direct COM dispatch — not
# reflection), administrative extraction via msiexec /a, staged-binary
# structure, and the extracted binary's headless --beta-smoke exit status.
#
# Evidence-first contract:
#   * PACKAGE-EVIDENCE.txt in the package directory receives every check
#     result plus the complete smoke logs.
#   * The complete evidence report is printed to the terminal on success and
#     on failure, so no defect is visible only inside an uploaded artifact.
#   * VALIDATION-SUMMARY.toml is written beside the installer on every exit;
#     the publish job refuses to release unless every summary reports
#     result = "passed" and smoke_exit = 0.
#
# The beta smoke workspace is always derived as
#   <WorkspaceRoot>/target/release-smoke/windows-x64-msi/workspace
# because the application rejects beta workspaces outside <workspace>/target.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$PackageDir,
    [Parameter(Mandatory = $true)]
    [string]$ReleaseVersion,
    [Parameter(Mandatory = $true)]
    [string]$SourceSha,
    [Parameter(Mandatory = $true)]
    [string]$WorkspaceRoot,
    [switch]$PrintSmokePlan
)

$ErrorActionPreference = "Stop"

if ($ReleaseVersion -notmatch '^0\.0\.[1-9][0-9]*$') {
    throw "-ReleaseVersion must be canonical 0.0.N with N at least 1 and no zero padding"
}
if ($SourceSha -notmatch '^[0-9a-fA-F]{40}$') {
    throw "-SourceSha must be a 40-hex commit SHA"
}

$PackageDir = [System.IO.Path]::GetFullPath($PackageDir)
$WorkspaceRoot = [System.IO.Path]::GetFullPath($WorkspaceRoot)
$stem = "legion-desktop-windows-x64-msi"
$msiPath = Join-Path $PackageDir "$stem.msi"
$checksumPath = "$msiPath.sha256"
$metadataPath = Join-Path $PackageDir "RELEASE-METADATA.toml"
$evidencePath = Join-Path $PackageDir "PACKAGE-EVIDENCE.txt"
$summaryPath = Join-Path $PackageDir "VALIDATION-SUMMARY.toml"
$smokeRoot = Join-Path $WorkspaceRoot "target/release-smoke/windows-x64-msi"
$betaWorkspace = Join-Path $smokeRoot "workspace"
$smokeDir = Join-Path $smokeRoot "smoke"
$stagingDir = Join-Path $smokeRoot "staging"
$candidateTag = "v$ReleaseVersion"

if ($PrintSmokePlan) {
    Write-Output "artifact=$msiPath"
    Write-Output "beta_workspace=$betaWorkspace"
    Write-Output "smoke_dir=$smokeDir"
    Write-Output "staging_dir=$stagingDir"
    exit 0
}

# Per-check status tracked for VALIDATION-SUMMARY.toml. "not-run" means the
# verifier failed before reaching that check; the publish gate accepts only
# "passed".
$checksumStatus = "not-run"
$metadataStatus = "not-run"
$packageVersionStatus = "not-run"
$structureStatus = "not-run"
$smokeExit = -1
$result = "failed"
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Fail([string]$Message) {
    Add-Content -LiteralPath $evidencePath -Value "error=$Message"
    throw $Message
}

function Read-MsiProductVersion([string]$Path) {
    $windowsInstaller = $null
    $database = $null
    $view = $null
    $record = $null
    try {
        $windowsInstaller = New-Object -ComObject WindowsInstaller.Installer
        try {
            $database = $windowsInstaller.OpenDatabase($Path, 0)
            $query = 'SELECT `Value` FROM `Property` WHERE `Property` = ''ProductVersion'''
            $view = $database.OpenView($query)
            $view.Execute()
            $record = $view.Fetch()
        } catch {
            Fail "Unable to query MSI Property table for ProductVersion via WindowsInstaller.Installer: $($_.Exception.Message)"
        }
        if ($null -eq $record) {
            Fail "MSI Property table does not contain ProductVersion"
        }
        return $record.StringData(1)
    } finally {
        if ($null -ne $view) {
            try { $view.Close() } catch {}
        }
        foreach ($comObject in @($record, $view, $database, $windowsInstaller)) {
            if ($null -ne $comObject -and [System.Runtime.InteropServices.Marshal]::IsComObject($comObject)) {
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($comObject)
            }
        }
    }
}

# Create the evidence report before any check so that every failure path has a
# durable, printable record.
New-Item -ItemType Directory -Force -Path $PackageDir, $smokeRoot, $smokeDir, $stagingDir | Out-Null
@(
    "verifier=scripts/verify-native-package.ps1"
    "candidate_tag=$candidateTag"
    "source_sha=$SourceSha"
    "format=wix"
    "architecture=x64"
    "verifier_os=$([System.Environment]::OSVersion.VersionString)"
    "version_reader=WindowsInstaller.Installer direct COM dispatch"
) | Add-Content -LiteralPath $evidencePath

try {
    foreach ($required in @($msiPath, $checksumPath, $metadataPath)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            Fail "Missing required package file: $required"
        }
    }

    $checksumLine = (Get-Content -LiteralPath $checksumPath -Raw).Replace("`r", "").Trim()
    if ($checksumLine -notmatch '^([0-9a-f]{64}) \*(.+)$') {
        Fail "Malformed checksum file: $checksumPath"
    }
    if ($Matches[2] -ne (Split-Path -Leaf $msiPath)) {
        Fail "Checksum names unexpected installer: $($Matches[2])"
    }
    $expectedHash = $Matches[1]
    $actualHash = (Get-FileHash -LiteralPath $msiPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $expectedHash) {
        Fail "MSI checksum mismatch: expected $expectedHash, computed $actualHash"
    }
    $checksumStatus = "passed"
    Add-Content -LiteralPath $evidencePath -Value "checksum=passed sha256=$actualHash"

    $metadata = Get-Content -LiteralPath $metadataPath
    foreach ($line in @(
        "release_version = `"$ReleaseVersion`""
        "git_sha = `"$SourceSha`""
        'platform = "windows"'
        'architecture = "x64"'
        'format = "wix"'
        'signer_status = "unsigned-beta/no-os-code-signing"'
    )) {
        if ($metadata -cnotcontains $line) {
            Fail "Missing metadata line: $line"
        }
    }
    $metadataStatus = "passed"
    Add-Content -LiteralPath $evidencePath -Value "metadata=passed"

    # Read ProductVersion before the (slower) administrative extraction so a
    # version defect is reported cheaply and deterministically.
    $productVersion = Read-MsiProductVersion $msiPath
    if ($productVersion -cne $ReleaseVersion) {
        Fail "MSI ProductVersion mismatch: expected $ReleaseVersion, found $productVersion"
    }
    $packageVersionStatus = "passed"
    Add-Content -LiteralPath $evidencePath -Value "package_version=passed version=$productVersion"

    $arguments = @('/a', "`"$msiPath`"", '/qn', "TARGETDIR=`"$stagingDir`"", '/norestart')
    $process = Start-Process msiexec.exe -ArgumentList $arguments -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        Fail "msiexec administrative install failed with exit code $($process.ExitCode)"
    }
    $stagedBinaries = @(
        Get-ChildItem -LiteralPath $stagingDir -Recurse -File -Filter "legion-desktop.exe"
    )
    if ($stagedBinaries.Count -ne 1) {
        $found = ($stagedBinaries.FullName -join ", ")
        Fail "Expected exactly one staged legion-desktop.exe; found $($stagedBinaries.Count): $found"
    }
    $stagedBinary = $stagedBinaries[0].FullName
    $structureStatus = "passed"
    Add-Content -LiteralPath $evidencePath -Value "structure=passed binary=$stagedBinary"

    $smokeLog = Join-Path $smokeDir "stdout-stderr.log"
    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $smokeArguments = @(
        "--beta-smoke"
        "--duration-ms", "1500"
        "--workspace", $WorkspaceRoot
        "--beta-workspace", $betaWorkspace
        "--evidence", (Join-Path $smokeDir "beta-smoke.md")
        "--session-state", (Join-Path $smokeDir "session.json")
        "--diagnostics-export", (Join-Path $smokeDir "diagnostics.md")
    )
    & $stagedBinary @smokeArguments *> $smokeLog
    $smokeStatus = $LASTEXITCODE
    $ErrorActionPreference = $oldPreference
    $smokeExit = $smokeStatus
    Add-Content -LiteralPath $evidencePath -Value "smoke_exit=$smokeStatus policy=hard-fail-beta-workflow-is-headless"
    if (Test-Path -LiteralPath $smokeLog) {
        Get-Content -LiteralPath $smokeLog | Add-Content -LiteralPath $evidencePath
    }
    $betaEvidence = Join-Path $smokeDir "beta-smoke.md"
    if (Test-Path -LiteralPath $betaEvidence) {
        Get-Content -LiteralPath $betaEvidence | Add-Content -LiteralPath $evidencePath
    }
    if ($smokeStatus -ne 0) {
        Fail "Installed MSI beta smoke failed with exit code $smokeStatus"
    }
    Add-Content -LiteralPath $evidencePath -Value "smoke=passed"
    $result = "passed"
} finally {
    $summaryLines = @(
        "schema_version = 1"
        "candidate_tag = `"$candidateTag`""
        "source_sha = `"$SourceSha`""
        'format = "wix"'
        'architecture = "x64"'
        "checksum = `"$checksumStatus`""
        "metadata = `"$metadataStatus`""
        "package_version = `"$packageVersionStatus`""
        "structure = `"$structureStatus`""
        "smoke_exit = $smokeExit"
        "result = `"$result`""
    )
    [System.IO.File]::WriteAllText($summaryPath, (($summaryLines -join "`n") + "`n"), $Utf8NoBom)
    Add-Content -LiteralPath $evidencePath -Value "result=$result"
    Write-Host "==== PACKAGE-EVIDENCE ($stem) ===="
    Get-Content -LiteralPath $evidencePath | ForEach-Object { Write-Host $_ }
}
