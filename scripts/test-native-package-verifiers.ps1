# Contract tests for scripts/verify-native-package.sh and
# scripts/verify-native-package.ps1.
#
# These tests exercise the verifier boundaries against synthetic fixtures and
# require neither real installers nor a macOS/Linux host:
#   * every generated --beta-workspace is contained under
#     <workspace-root>/target/release-smoke/<platform>-<arch>-<format>/workspace;
#   * a verifier failure prints the PACKAGE-EVIDENCE.txt report to stdout
#     before exiting non-zero (missing-file and bad-checksum rejection paths);
#   * every exit writes VALIDATION-SUMMARY.toml with result = "failed" on
#     failure so the publish gate can never mistake a crash for a pass;
#   * the Windows version reader rejects a missing MSI ProductVersion with an
#     actionable error (synthetic MSI built via Windows Installer Automation).
#
# Host-specific tooling (dpkg-deb, AppImage runtime, hdiutil, msiexec) remains
# the authority for actual installation/extraction semantics; those paths run
# on the platform-native jobs in .github/workflows/legion-release.yml.

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$shVerifier = Join-Path $repoRoot "scripts/verify-native-package.sh"
$psVerifier = Join-Path $repoRoot "scripts/verify-native-package.ps1"
$testSha = "0123456789abcdef0123456789abcdef01234567"
$isWindowsHost = ($env:OS -eq "Windows_NT")

function Resolve-GitBash {
    if (-not $script:isWindowsHost) {
        return "bash"
    }
    $candidates = @(
        (Join-Path $env:ProgramFiles "Git/bin/bash.exe"),
        (Join-Path $env:ProgramFiles "Git/usr/bin/bash.exe")
    )
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return $candidate
        }
    }
    # Fall back to PATH, but never WSL's System32 bash shim: it cannot resolve
    # Windows-style C:/ paths the tests pass.
    $fromPath = @(Get-Command bash -ErrorAction SilentlyContinue -All) |
        Where-Object { $_.Source -notmatch '(?i)system32' } |
        Select-Object -First 1
    if ($null -ne $fromPath) {
        return $fromPath.Source
    }
    return $null
}

function Resolve-PowerShellExe {
    $pwsh = Get-Command pwsh -ErrorAction SilentlyContinue
    if ($null -ne $pwsh) {
        return $pwsh.Source
    }
    return (Get-Command powershell).Source
}

$gitBash = Resolve-GitBash
$psExe = Resolve-PowerShellExe
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("legion-verifier-tests-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $testRoot | Out-Null

$script:passed = 0
$script:failed = 0
$script:skipped = 0

function Invoke-Test([string]$Name, [scriptblock]$Body) {
    try {
        & $Body
        $script:passed++
        Write-Host "PASS  $Name"
    } catch {
        $message = $_.Exception.Message
        if ($message -like "SKIP:*") {
            $script:skipped++
            Write-Host "SKIP  $Name -- $message"
        } else {
            $script:failed++
            Write-Host "FAIL  $Name -- $message"
        }
    }
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) {
        throw $Message
    }
}

function ConvertTo-BashPath([string]$Path) {
    return ($Path -replace '\\', '/')
}

function New-FixtureDir([string]$Name) {
    $dir = Join-Path $testRoot $Name
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    return $dir
}

function Write-MetadataFile([string]$Dir, [string]$Platform, [string]$Architecture, [string]$Format) {
    $lines = @(
        'release_version = "0.0.1"'
        'workspace_version = "0.1.0"'
        "git_sha = `"$testSha`""
        "platform = `"$Platform`""
        "architecture = `"$Architecture`""
        "format = `"$Format`""
        'signer_status = "unsigned-beta/no-os-code-signing"'
    )
    [System.IO.File]::WriteAllText(
        (Join-Path $Dir "RELEASE-METADATA.toml"),
        (($lines -join "`n") + "`n"),
        (New-Object System.Text.UTF8Encoding($false))
    )
}

function Invoke-ShVerifier([string[]]$Arguments) {
    if ($null -eq $script:gitBash) {
        throw "SKIP: no usable bash (Git Bash) found on this host"
    }
    # Relax the preference around the child call: under Windows PowerShell 5.1
    # a `2>&1` redirection of native stderr raises a terminating error when
    # $ErrorActionPreference is Stop.
    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $stdout = & $script:gitBash (ConvertTo-BashPath $script:shVerifier) @Arguments 2>&1 | ForEach-Object { "$_" }
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $oldPreference
    }
    return [pscustomobject]@{
        ExitCode = $exitCode
        Output   = ($stdout -join "`n")
    }
}

function Invoke-PsVerifier([string[]]$Arguments) {
    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $stdout = & $script:psExe -NoProfile -ExecutionPolicy Bypass -File $script:psVerifier @Arguments 2>&1 | ForEach-Object { "$_" }
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $oldPreference
    }
    return [pscustomobject]@{
        ExitCode = $exitCode
        Output   = ($stdout -join "`n")
    }
}

function Get-OutputValue([string]$Output, [string]$Key) {
    foreach ($line in ($Output -split "`n")) {
        if ($line.StartsWith("$Key=")) {
            return $line.Substring($Key.Length + 1)
        }
    }
    return $null
}

function New-EmptyPropertyTableMsi([string]$Path) {
    # Build a real (but empty) Windows Installer database whose Property table
    # exists and deliberately lacks ProductVersion.
    $installer = $null
    $database = $null
    $view = $null
    try {
        try {
            $installer = New-Object -ComObject WindowsInstaller.Installer
        } catch {
            throw "SKIP: WindowsInstaller.Installer COM is unavailable: $($_.Exception.Message)"
        }
        $database = $installer.OpenDatabase($Path, 3) # msiOpenDatabaseModeCreateDirect
        $sql = 'CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(0) NOT NULL LOCALIZABLE PRIMARY KEY `Property`)'
        $view = $database.OpenView($sql)
        $view.Execute()
        $view.Close()
        $database.Commit()
    } finally {
        foreach ($comObject in @($view, $database, $installer)) {
            if ($null -ne $comObject -and [System.Runtime.InteropServices.Marshal]::IsComObject($comObject)) {
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($comObject)
            }
        }
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
    }
}

function New-ProductVersionMsi([string]$Path, [string]$Value) {
    # A real Windows Installer database whose Property table carries
    # ProductVersion with the exact bytes given -- including padding, which is
    # the point of the regression this builds a fixture for.
    $installer = $null
    $database = $null
    $view = $null
    # Declared out here so the finally block can release it. Leaving the insert
    # view out of that list held the database file open, and the next read of
    # the .msi failed with "being used by another process" -- which looks like
    # a flaky test rather than a leaked COM handle.
    $insert = $null
    try {
        try {
            $installer = New-Object -ComObject WindowsInstaller.Installer
        } catch {
            throw "SKIP: WindowsInstaller.Installer COM is unavailable: $($_.Exception.Message)"
        }
        $database = $installer.OpenDatabase($Path, 3) # msiOpenDatabaseModeCreateDirect
        $sql = 'CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(0) NOT NULL LOCALIZABLE PRIMARY KEY `Property`)'
        $view = $database.OpenView($sql)
        [void]$view.Execute()
        [void]$view.Close()
        $insert = $database.OpenView("INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductVersion', '$Value')")
        [void]$insert.Execute()
        [void]$insert.Close()
        $database.Commit()
    } finally {
        foreach ($comObject in @($insert, $view, $database, $installer)) {
            if ($null -ne $comObject -and [System.Runtime.InteropServices.Marshal]::IsComObject($comObject)) {
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($comObject)
            }
        }
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
    }
}

function Write-RealChecksumFile([string]$ArtifactPath) {
    $hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
    [System.IO.File]::WriteAllText(
        "$ArtifactPath.sha256",
        "$hash *$(Split-Path -Leaf $ArtifactPath)`n",
        [System.Text.Encoding]::ASCII
    )
}

$zeroHash = "0" * 64

Write-Host "== native package verifier contract tests =="
Write-Host "bash: $(if ($gitBash) { $gitBash } else { '<none>' })"
Write-Host "powershell: $psExe"
Write-Host "fixtures: $testRoot"
Write-Host ""

# --- Containment: every generated --beta-workspace lives under <workspace>/target ---

$workspace = New-FixtureDir "workspace"
$workspaceBash = ConvertTo-BashPath $workspace

foreach ($case in @(
    @{ Format = "deb"; Architecture = "x64"; Stem = "linux-x64-deb" },
    @{ Format = "appimage"; Architecture = "x64"; Stem = "linux-x64-appimage" },
    @{ Format = "dmg"; Architecture = "x64"; Stem = "macos-x64-dmg" },
    @{ Format = "dmg"; Architecture = "arm64"; Stem = "macos-arm64-dmg" }
)) {
    $format = $case.Format
    $architecture = $case.Architecture
    $stem = $case.Stem
    Invoke-Test "sh verifier plans $stem beta workspace under <workspace>/target" {
        $packageDir = ConvertTo-BashPath (New-FixtureDir "plan-$stem")
        $run = Invoke-ShVerifier @(
            "--format", $format,
            "--package-dir", $packageDir,
            "--release-version", "0.0.1",
            "--source-sha", $testSha,
            "--workspace-root", $workspaceBash,
            "--architecture", $architecture,
            "--print-smoke-plan"
        )
        Assert-True ($run.ExitCode -eq 0) "print-smoke-plan exited $($run.ExitCode): $($run.Output)"
        $betaWorkspace = Get-OutputValue $run.Output "beta_workspace"
        Assert-True ($null -ne $betaWorkspace) "no beta_workspace line in: $($run.Output)"
        $expectedPrefix = "$workspaceBash/target/release-smoke/$stem"
        Assert-True ($betaWorkspace.StartsWith($expectedPrefix, [System.StringComparison]::Ordinal)) `
            "beta workspace '$betaWorkspace' is not under '$expectedPrefix'"
        Assert-True ($betaWorkspace.EndsWith("/workspace")) "beta workspace '$betaWorkspace' does not end with /workspace"
        $smokeDir = Get-OutputValue $run.Output "smoke_dir"
        Assert-True ($null -ne $smokeDir -and $smokeDir.StartsWith($expectedPrefix, [System.StringComparison]::Ordinal)) `
            "smoke dir '$smokeDir' is not under '$expectedPrefix'"
    }
}

Invoke-Test "ps verifier plans windows-x64-msi beta workspace under <workspace>/target" {
    $packageDir = New-FixtureDir "plan-windows-x64-msi"
    $run = Invoke-PsVerifier @(
        "-PackageDir", $packageDir,
        "-ReleaseVersion", "0.0.1",
        "-SourceSha", $testSha,
        "-WorkspaceRoot", $workspace,
        "-PrintSmokePlan"
    )
    Assert-True ($run.ExitCode -eq 0) "-PrintSmokePlan exited $($run.ExitCode): $($run.Output)"
    $betaWorkspace = Get-OutputValue $run.Output "beta_workspace"
    Assert-True ($null -ne $betaWorkspace) "no beta_workspace line in: $($run.Output)"
    $expectedPrefix = Join-Path $workspace "target/release-smoke/windows-x64-msi"
    Assert-True ($betaWorkspace.StartsWith($expectedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) `
        "beta workspace '$betaWorkspace' is not under '$expectedPrefix'"
    Assert-True ($betaWorkspace.EndsWith("workspace")) "beta workspace '$betaWorkspace' does not end with workspace"
}

# --- POSIX verifier: missing-file rejection with evidence streamed to stdout ---

Invoke-Test "sh verifier rejects a missing DEB and prints evidence before exit" {
    $packageDir = New-FixtureDir "deb-missing"
    $run = Invoke-ShVerifier @(
        "--format", "deb",
        "--package-dir", (ConvertTo-BashPath $packageDir),
        "--release-version", "0.0.1",
        "--source-sha", $testSha,
        "--workspace-root", $workspaceBash,
        "--architecture", "x64"
    )
    Assert-True ($run.ExitCode -ne 0) "verifier unexpectedly passed: $($run.Output)"
    Assert-True ($run.Output -match 'error=missing required file:') "no missing-file error in output: $($run.Output)"
    Assert-True ($run.Output -match 'result=failed') "evidence report was not printed before exit: $($run.Output)"
    Assert-True ($run.Output -match 'candidate_tag=v0\.0\.1') "evidence header missing from stdout: $($run.Output)"
    $evidence = Join-Path $packageDir "PACKAGE-EVIDENCE.txt"
    Assert-True (Test-Path -LiteralPath $evidence) "PACKAGE-EVIDENCE.txt was not written"
    Assert-True ((Get-Content -LiteralPath $evidence -Raw) -match 'result=failed') "evidence file lacks result=failed"
    $summary = Join-Path $packageDir "VALIDATION-SUMMARY.toml"
    Assert-True (Test-Path -LiteralPath $summary) "VALIDATION-SUMMARY.toml was not written"
    $summaryText = Get-Content -LiteralPath $summary -Raw
    Assert-True ($summaryText -match 'result = "failed"') "summary lacks result = `"failed`""
    Assert-True ($summaryText -match 'smoke_exit = -1') "summary lacks smoke_exit = -1 for a pre-smoke failure"
}

# --- POSIX verifier: bad-checksum rejection ---

Invoke-Test "sh verifier rejects a DEB whose sha256 does not match" {
    $packageDir = New-FixtureDir "deb-bad-checksum"
    $debPath = Join-Path $packageDir "legion-desktop-linux-x64-deb.deb"
    [System.IO.File]::WriteAllText($debPath, "not a real debian package", [System.Text.Encoding]::ASCII)
    [System.IO.File]::WriteAllText("$debPath.sha256", "$zeroHash *legion-desktop-linux-x64-deb.deb`n", [System.Text.Encoding]::ASCII)
    Write-MetadataFile $packageDir "linux" "x64" "deb"
    $run = Invoke-ShVerifier @(
        "--format", "deb",
        "--package-dir", (ConvertTo-BashPath $packageDir),
        "--release-version", "0.0.1",
        "--source-sha", $testSha,
        "--workspace-root", $workspaceBash,
        "--architecture", "x64"
    )
    Assert-True ($run.ExitCode -ne 0) "verifier unexpectedly passed: $($run.Output)"
    Assert-True ($run.Output -match 'error=sha256 mismatch') "no sha256 mismatch error in output: $($run.Output)"
    Assert-True ($run.Output -match 'result=failed') "evidence report was not printed before exit: $($run.Output)"
    $summaryText = Get-Content -LiteralPath (Join-Path $packageDir "VALIDATION-SUMMARY.toml") -Raw
    Assert-True ($summaryText -match 'checksum = "not-run"') "summary must not report checksum as passed"
    Assert-True ($summaryText -match 'result = "failed"') "summary lacks result = `"failed`""
}

# --- DMG failure visibility (Task 5): missing DMG prints the report, not a bare eject line ---

Invoke-Test "sh verifier prints the DMG failure report for a non-existent DMG" {
    $packageDir = New-FixtureDir "dmg-missing"
    $run = Invoke-ShVerifier @(
        "--format", "dmg",
        "--package-dir", (ConvertTo-BashPath $packageDir),
        "--release-version", "0.0.1",
        "--source-sha", $testSha,
        "--workspace-root", $workspaceBash,
        "--architecture", "arm64"
    )
    Assert-True ($run.ExitCode -ne 0) "verifier unexpectedly passed: $($run.Output)"
    Assert-True ($run.Output -match 'error=missing required file:.*legion-desktop-macos-arm64-dmg\.dmg') `
        "no actionable DMG failure in output: $($run.Output)"
    Assert-True ($run.Output -match '==== PACKAGE-EVIDENCE \(legion-desktop-macos-arm64-dmg\) ====') `
        "failure report banner missing from terminal output: $($run.Output)"
    Assert-True ($run.Output -match 'result=failed') "evidence report was not printed before exit: $($run.Output)"
}

# --- Windows verifier: missing-file rejection with evidence streamed to stdout ---

Invoke-Test "ps verifier rejects a missing MSI and prints evidence before exit" {
    $packageDir = New-FixtureDir "msi-missing"
    $run = Invoke-PsVerifier @(
        "-PackageDir", $packageDir,
        "-ReleaseVersion", "0.0.1",
        "-SourceSha", $testSha,
        "-WorkspaceRoot", $workspace
    )
    Assert-True ($run.ExitCode -ne 0) "verifier unexpectedly passed: $($run.Output)"
    Assert-True ($run.Output -match 'Missing required package file') "no missing-file error in output: $($run.Output)"
    Assert-True ($run.Output -match 'result=failed') "evidence report was not printed before exit: $($run.Output)"
    $summary = Join-Path $packageDir "VALIDATION-SUMMARY.toml"
    Assert-True (Test-Path -LiteralPath $summary) "VALIDATION-SUMMARY.toml was not written"
    $summaryText = Get-Content -LiteralPath $summary -Raw
    Assert-True ($summaryText -match 'result = "failed"') "summary lacks result = `"failed`""
    Assert-True ($summaryText -match 'smoke_exit = -1') "summary lacks smoke_exit = -1 for a pre-smoke failure"
}

# --- Windows verifier: bad-checksum rejection (fails before msiexec or COM) ---

Invoke-Test "ps verifier rejects an MSI whose sha256 does not match" {
    $packageDir = New-FixtureDir "msi-bad-checksum"
    $msiPath = Join-Path $packageDir "legion-desktop-windows-x64-msi.msi"
    [System.IO.File]::WriteAllText($msiPath, "not a real msi", [System.Text.Encoding]::ASCII)
    [System.IO.File]::WriteAllText("$msiPath.sha256", "$zeroHash *legion-desktop-windows-x64-msi.msi`n", [System.Text.Encoding]::ASCII)
    Write-MetadataFile $packageDir "windows" "x64" "wix"
    $run = Invoke-PsVerifier @(
        "-PackageDir", $packageDir,
        "-ReleaseVersion", "0.0.1",
        "-SourceSha", $testSha,
        "-WorkspaceRoot", $workspace
    )
    Assert-True ($run.ExitCode -ne 0) "verifier unexpectedly passed: $($run.Output)"
    Assert-True ($run.Output -match 'MSI checksum mismatch') "no checksum mismatch error in output: $($run.Output)"
    Assert-True ($run.Output -match 'result=failed') "evidence report was not printed before exit: $($run.Output)"
    $summaryText = Get-Content -LiteralPath (Join-Path $packageDir "VALIDATION-SUMMARY.toml") -Raw
    Assert-True ($summaryText -match 'checksum = "not-run"') "summary must not report checksum as passed"
}

# --- Windows version reader: padded ProductVersion is not a mismatch ---

Invoke-Test "ps verifier accepts a ProductVersion carrying surrounding whitespace" {
    if (-not $isWindowsHost) {
        throw "SKIP: Windows Installer Automation requires a Windows host"
    }
    # Regression: the COM StringData accessor returns the Property value with
    # padding, and the comparison is ordinal, so every release failed with
    # "expected 0.0.2, found  0.0.2" -- two identical versions and a verifier
    # insisting they differed. The whole release pipeline was blocked by it.
    $packageDir = New-FixtureDir "msi-padded-product-version"
    $msiPath = Join-Path $packageDir "legion-desktop-windows-x64-msi.msi"
    New-ProductVersionMsi $msiPath " 0.0.1 "
    Write-RealChecksumFile $msiPath
    Write-MetadataFile $packageDir "windows" "x64" "wix"
    $run = Invoke-PsVerifier @(
        "-PackageDir", $packageDir,
        "-ReleaseVersion", "0.0.1",
        "-SourceSha", $testSha,
        "-WorkspaceRoot", $workspace
    )
    # The fixture is not a real installable package, so the verifier still
    # fails later at extraction. What must not appear is the version mismatch.
    Assert-True ($run.Output -notmatch 'ProductVersion mismatch') `
        "padded ProductVersion was reported as a mismatch: $($run.Output)"
}

Invoke-Test "ps verifier still rejects a genuinely different ProductVersion" {
    if (-not $isWindowsHost) {
        throw "SKIP: Windows Installer Automation requires a Windows host"
    }
    # The other half of the trim: narrowing the comparison must not stop it
    # catching a real mismatch.
    $packageDir = New-FixtureDir "msi-wrong-product-version"
    $msiPath = Join-Path $packageDir "legion-desktop-windows-x64-msi.msi"
    New-ProductVersionMsi $msiPath "0.0.9"
    Write-RealChecksumFile $msiPath
    Write-MetadataFile $packageDir "windows" "x64" "wix"
    $run = Invoke-PsVerifier @(
        "-PackageDir", $packageDir,
        "-ReleaseVersion", "0.0.1",
        "-SourceSha", $testSha,
        "-WorkspaceRoot", $workspace
    )
    Assert-True ($run.ExitCode -ne 0) "verifier unexpectedly passed: $($run.Output)"
    Assert-True ($run.Output -match 'ProductVersion mismatch') `
        "a real version mismatch was not reported: $($run.Output)"
}

# --- Windows version reader: missing ProductVersion is an actionable error ---

Invoke-Test "ps verifier rejects an MSI without ProductVersion with an actionable error" {
    if (-not $isWindowsHost) {
        throw "SKIP: Windows Installer Automation requires a Windows host"
    }
    $packageDir = New-FixtureDir "msi-no-product-version"
    $msiPath = Join-Path $packageDir "legion-desktop-windows-x64-msi.msi"
    New-EmptyPropertyTableMsi $msiPath
    Write-RealChecksumFile $msiPath
    Write-MetadataFile $packageDir "windows" "x64" "wix"
    $run = Invoke-PsVerifier @(
        "-PackageDir", $packageDir,
        "-ReleaseVersion", "0.0.1",
        "-SourceSha", $testSha,
        "-WorkspaceRoot", $workspace
    )
    Assert-True ($run.ExitCode -ne 0) "verifier unexpectedly passed: $($run.Output)"
    Assert-True ($run.Output -match 'ProductVersion') "error does not mention ProductVersion: $($run.Output)"
    Assert-True ($run.Output -match 'result=failed') "evidence report was not printed before exit: $($run.Output)"
    $summaryText = Get-Content -LiteralPath (Join-Path $packageDir "VALIDATION-SUMMARY.toml") -Raw
    Assert-True ($summaryText -match 'checksum = "passed"') "checksum should pass before the version reader runs"
    Assert-True ($summaryText -match 'package_version = "not-run"') "package_version must not be reported as passed"
    Assert-True ($summaryText -match 'result = "failed"') "summary lacks result = `"failed`""
}

Write-Host ""
Write-Host "passed=$($script:passed) failed=$($script:failed) skipped=$($script:skipped)"

try {
    Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction Stop
} catch {
    Write-Host "warning: could not remove fixture directory $testRoot"
}

if ($script:failed -gt 0) {
    exit 1
}
exit 0
