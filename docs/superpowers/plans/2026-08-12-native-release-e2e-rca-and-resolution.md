# Native Release E2E RCA and Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the manual native-release workflow publish only installers that have passed reproducible, platform-native checksum, metadata, install/extract, and headless-beta-smoke verification on GitHub-hosted runners.

**Architecture:** Separate package verification from the GitHub Actions YAML into two version-controlled verifier entry points: a POSIX verifier for DEB/AppImage/DMG and a PowerShell verifier for MSI. Each verifier will create a beta workspace beneath `<workspace>/target/release-smoke/`, stream a durable evidence report to the job log, validate every assigned format before returning an aggregate failure, and have the workflow upload that evidence whether it passes or fails. A `verify-only` manual dispatch mode will make E2E validation repeatable without creating a tag or release; `publish` will remain gated on all verifiers passing.

**Tech Stack:** GitHub Actions, Bash, PowerShell 7, cargo-packager 0.11.8, dpkg-deb, AppImage runtime, macOS `hdiutil`/`PlistBuddy`, Windows Installer (`msiexec` and Windows Installer Automation), Rust integration tests.

## Global Constraints

- The workflow remains manual-only (`workflow_dispatch`); no push, pull-request, schedule, or release trigger may be added.
- Publish tags remain strictly canonical and sequential: `v0.0.1`, `v0.0.2`, and so on; a verify-only run must create neither a tag nor a GitHub Release.
- Release packages are `unsigned-beta/no-os-code-signing`; do not add certificates, notarization credentials, signing keys, or secret material to the repository.
- Retain actual installers: Windows x64 MSI, Linux x64 DEB, Linux x64 AppImage, macOS x64 DMG, and macOS arm64 DMG.
- The workspace Cargo package version remains `0.1.0`; release version injection is packaging-only.
- The beta workflow deliberately rejects writable workspaces outside `<workspace>/target`; release verification must respect that contract rather than loosen it.
- Every package verifier must validate artifact existence, SHA-256, generated release metadata, install/extract structure, installer version, and the installed/extracted binary’s `--beta-smoke` exit status.
- Every verifier failure must print and upload its complete evidence before returning non-zero; no error may be visible only inside a runner temporary directory.
- Do not fabricate a Debian Maintainer identity. The public maintainer name/email must be supplied and approved before it is embedded in a release artifact.

---

## Confirmed RCA Baseline

The following facts were retrieved from manual workflow run `31661117478` for source `aa8ee06a00af6fc477f3259d97f04befc56946e0`, including the uploaded package evidence artifacts.

| Surface | What succeeded | Verified root cause / defect |
|---|---|---|
| Linux DEB | Package, SHA-256, metadata, extraction, desktop entry, icon, and executable all passed. | The beta smoke exits 1 because `.github/workflows/legion-release.yml` passes `--beta-workspace "$RUNNER_TEMP/legion-deb-smoke/workspace"`. `crates/legion-desktop/src/beta.rs` rejects it because it is outside `<workspace>/target`. The missing `Maintainer` field is also a real package-metadata defect. |
| Linux AppImage | Package and artifact upload succeeded; validation was skipped after the DEB validator failed. | It uses the same invalid `$RUNNER_TEMP` beta-workspace pattern, so it has not yet received an independent passing E2E test and is expected to fail for the same reason. |
| macOS x64 and arm64 DMG | Package, SHA-256, `hdiutil verify`, mount, bundle copy, executable, and injected `CFBundleShortVersionString` passed. | Both beta smokes exit 1 because the workflow passes `$RUNNER_TEMP/legion-dmg-smoke/workspace`, which violates the same containment invariant. The final log line is `hdiutil detach`; the actionable smoke evidence is currently only inside the uploaded artifact. |
| Windows MSI | Package, SHA-256, metadata, and administrative extraction passed. | The verifier fails before smoke at YAML lines 460–466. Its reflection-based `WindowsInstaller.Installer.OpenDatabase` call receives incompatible COM argument types and raises `DISP_E_TYPEMISMATCH`. The MSI itself has not been proven version-correct or smoke-correct yet. |

The earlier cargo-packager configuration failure was resolved by adding `name = "legion-desktop"` to `packaging/Packager.toml`; this plan does not revisit it except to keep it covered by E2E verification.

## Release Acceptance Contract

For a candidate `v0.0.N` and source SHA `S`, all five format-specific reports must contain the following successful facts before `publish` is eligible:

```text
candidate_tag=v0.0.N
source_sha=S
checksum=passed sha256=<64 lowercase hex characters>
metadata=passed
package_version=passed version=0.0.N
structure=passed
smoke_exit=0
smoke=passed
```

The DEB report additionally must show a non-empty `Maintainer:` field. A validation report that is missing, malformed, or reports a non-zero smoke status is a release gate failure.

### Task 1: Preserve the incident record and establish safe E2E execution modes

**Files:**

- Create: `docs/RELEASE_E2E_RCA.md`
- Modify: `.github/workflows/legion-release.yml:1-80,701-813`
- Test: manual GitHub Actions dispatch from the release candidate branch

**Interfaces:**

- Consumes: `workflow_dispatch.inputs.mode` with values `verify-only` and `publish`.
- Produces: `prepare.outputs.version`, `prepare.outputs.tag`, `prepare.outputs.source_sha`, `prepare.outputs.previous_tag`, and `prepare.outputs.mode`.
- Contract: `verify-only` builds and verifies all five installers but cannot create a tag, GitHub Release, or release asset; `publish` can do so only after the aggregate validation gate succeeds.

- [ ] **Step 1: Create the permanent RCA record from the preserved run evidence.**

  Include the run URL, source SHA, package artifact IDs, the four root causes in the table above, and an explicit statement that package generation is not the incident’s current failing layer. Link each finding to the exact workflow line range and application containment check.

- [ ] **Step 2: Add a manual-only mode input and propagate it as a job output.**

  Use this exact dispatch shape:

  ```yaml
  on:
    workflow_dispatch:
      inputs:
        mode:
          description: "Validate installers only, or validate and publish the next v0.0.N release"
          required: true
          default: verify-only
          type: choice
          options: [verify-only, publish]
  ```

  In `prepare`, write `mode=${{ inputs.mode }}` to `$GITHUB_OUTPUT`; make the `publish` job conditional on `inputs.mode == 'publish'` and on all package jobs succeeding.

- [ ] **Step 3: Run a verify-only dispatch from the candidate branch.**

  Expected: the version-selection job prints the candidate version, all package jobs run, and no tag or GitHub Release is created. Capture run ID, runner image details, `cargo-packager --version`, WiX version, `dpkg-deb --version`, and `hdiutil` version in the evidence.

- [ ] **Step 4: Confirm the mode boundary with GitHub CLI.**

  Run:

  ```powershell
  gh run view <verify-only-run-id> --json conclusion,url
  gh api repos/9thLevelSoftware/legion-ide/git/ref/tags/v0.0.N
  ```

  Expected: the run URL is available; the candidate tag endpoint returns 404 unless that tag existed before the run.

- [ ] **Step 5: Commit the incident record and dispatch-mode boundary.**

  ```powershell
  git add docs/RELEASE_E2E_RCA.md .github/workflows/legion-release.yml
  git commit -m "docs: record native release E2E RCA"
  ```

### Task 2: Introduce testable, evidence-first package verifiers

**Files:**

- Create: `scripts/verify-native-package.sh`
- Create: `scripts/verify-native-package.ps1`
- Create: `scripts/test-native-package-verifiers.ps1`
- Modify: `.github/workflows/legion-release.yml:248-651`
- Test: `scripts/test-native-package-verifiers.ps1`; GitHub verify-only workflow

**Interfaces:**

- POSIX command: `scripts/verify-native-package.sh --format <deb|appimage|dmg> --package-dir <dir> --release-version <0.0.N> --source-sha <sha> --workspace-root <dir> --architecture <x64|arm64>`.
- Windows command: `scripts/verify-native-package.ps1 -PackageDir <dir> -ReleaseVersion <0.0.N> -SourceSha <sha> -WorkspaceRoot <dir>`.
- Both write `PACKAGE-EVIDENCE.txt` beside the installer, print the exact same report on failure, and exit non-zero only after their assigned checks complete.

- [ ] **Step 1: Write the failing verifier-contract tests.**

  The PowerShell test script must assert command construction and evidence behavior without requiring a local macOS/Linux host:

  ```powershell
  $workspace = Join-Path $TestDrive "workspace"
  $smokeRoot = Join-Path $workspace "target/release-smoke/linux-deb"
  # Assert every generated --beta-workspace starts with "$workspace/target/".
  # Assert a verifier failure emits PACKAGE-EVIDENCE.txt to stdout before exit 1.
  # Assert the Windows version reader rejects a missing ProductVersion with an actionable error.
  ```

  Add platform-native execution tests to the workflow for each verifier; host-specific tooling is the authority for actual installation/extraction semantics.

- [ ] **Step 2: Implement the shared evidence lifecycle before any format-specific check.**

  Each verifier must create its report first, append `candidate_tag`, source SHA, format, architecture, OS/tool versions, and command path, then register an error trap/finally handler equivalent to:

  ```bash
  finish() {
    status=$?
    printf 'result=%s\n' "$([[ $status -eq 0 ]] && echo passed || echo failed)" >> "$evidence_path"
    cat "$evidence_path"
    exit "$status"
  }
  trap finish EXIT
  ```

  The PowerShell equivalent must use `try { ... } finally { Get-Content $evidencePath }`, retain the original failure status, and not suppress package or smoke errors.

- [ ] **Step 3: Implement a canonical smoke-workspace helper.**

  For each format, derive the beta workspace exactly as:

  ```text
  <workspace-root>/target/release-smoke/<platform>-<architecture>-<format>/workspace
  ```

  Keep diagnostic logs/session/evidence under the same `target/release-smoke/<stem>/` parent. The verifier must use the extracted/installed binary, pass `--workspace <workspace-root>`, and pass this beta workspace to `--beta-workspace`; it must not use `$RUNNER_TEMP` for any beta-workspace path.

- [ ] **Step 4: Move the existing checksum, metadata, extraction, and smoke code out of inline YAML.**

  Preserve the checks currently in the workflow, but normalize evidence keys to the release acceptance contract. Use `xvfb-run --auto-servernum` for Linux, `hdiutil verify` plus read-only attach/copy/detach for DMG, and `msiexec /a` for MSI. Do not downgrade smoke to best-effort: its zero exit code remains a hard gate.

- [ ] **Step 5: Run the new verifier contract tests.**

  Run:

  ```powershell
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-native-package-verifiers.ps1
  ```

  Expected: all contract tests pass, including the rule that beta workspaces are beneath the configured workspace’s `target` directory.

- [ ] **Step 6: Commit the verifier boundary.**

  ```powershell
  git add scripts/verify-native-package.sh scripts/verify-native-package.ps1 scripts/test-native-package-verifiers.ps1 .github/workflows/legion-release.yml
  git commit -m "test: add native installer verifier contract"
  ```

### Task 3: Correct Linux package metadata and validate both formats independently

**Files:**

- Modify: `packaging/Packager.toml:1-12`
- Modify: `scripts/verify-native-package.sh`
- Modify: `.github/workflows/legion-release.yml:231-371`
- Test: Linux verify-only job for DEB and AppImage

**Interfaces:**

- `Packager.toml` must provide cargo-packager’s `authors` array; cargo-packager maps this field to the Debian `Maintainer:` control field.
- DEB verifier produces `deb_maintainer=passed value=<approved maintainer>`.
- The Linux job invokes both verifiers and reports both result records before it returns its aggregate status.

- [ ] **Step 1: Obtain the release maintainer value.**

  Require an approved, monitored public identity in Debian control-file format, for example `Organization <support@example.invalid>`. Do not infer it from a GitHub username, a private address, or a placeholder. Record the approved value in the PR/release change description.

- [ ] **Step 2: Write the failing DEB metadata assertion.**

  Add the following check before declaring the DEB valid:

  ```bash
  maintainer="$(dpkg-deb -f "$deb_path" Maintainer)"
  test -n "$maintainer"
  test "$maintainer" = "$EXPECTED_DEB_MAINTAINER"
  printf 'deb_maintainer=passed value=%s\n' "$maintainer" >> "$evidence_path"
  ```

  Expected before the metadata change: failure because the control file contains no `Maintainer` field.

- [ ] **Step 3: Add the approved `authors` array to the packager template.**

  Add only the approved identity:

  ```toml
  authors = ["<approved maintainer identity>"]
  ```

  Keep `publisher` for Windows manufacturer metadata; it does not populate Debian’s maintainer field.

- [ ] **Step 4: Make Linux validation exhaustive.**

  Invoke the DEB and AppImage verifier commands from a single aggregate shell block that records their exit statuses and returns failure only after both have run:

  ```bash
  failures=0
  scripts/verify-native-package.sh --format deb ... || failures=1
  scripts/verify-native-package.sh --format appimage ... || failures=1
  exit "$failures"
  ```

  This prevents a DEB defect from hiding AppImage E2E results.

- [ ] **Step 5: Dispatch verify-only on Linux and inspect extracted evidence.**

  Expected: `dpkg-deb -f <package> Maintainer` equals the approved value; DEB and AppImage both show `smoke_exit=0`; AppImage extraction includes `AppRun`, `usr/bin/legion-desktop`, desktop file, and icon.

- [ ] **Step 6: Commit the Linux metadata and exhaustive-validation changes.**

  ```powershell
  git add packaging/Packager.toml scripts/verify-native-package.sh .github/workflows/legion-release.yml
  git commit -m "fix: verify Linux native installers end to end"
  ```

### Task 4: Replace the Windows MSI reflection failure with a proven interface

**Files:**

- Modify: `scripts/verify-native-package.ps1`
- Modify: `.github/workflows/legion-release.yml:393-546`
- Test: Windows verify-only job using the MSI built in that same job

**Interfaces:**

- Input MSI: `<package-dir>/legion-desktop-windows-x64-msi.msi`.
- Output evidence: `package_version=passed version=<0.0.N>`, `structure=passed binary=<path>`, and `smoke_exit=0`.
- Version reader must use a Windows Installer Automation invocation whose parameters are accepted by the hosted runner, not reflection with a raw `@($msiPath, 0)` argument array.

- [ ] **Step 1: Add an isolated Windows diagnostic step before replacing the implementation.**

  On a verify-only dispatch, run the freshly produced MSI through both candidate readers and append the exact result/version to evidence:

  ```powershell
  $installer = New-Object -ComObject WindowsInstaller.Installer
  $database = $installer.OpenDatabase($msiPath, 0)
  $view = $database.OpenView("SELECT `Value` FROM `Property` WHERE `Property` = 'ProductVersion'")
  $view.Execute()
  $record = $view.Fetch()
  $record.StringData(1)
  ```

  If the direct PowerShell dispatch is not accepted on `windows-latest`, run an equivalent temporary `cscript.exe`/VBScript probe, which passes Automation variants natively. Record the exact runner, Windows Installer version, method, and output. Do not merge a replacement based on local-only behavior.

- [ ] **Step 2: Write a failing version-reader test against a real MSI.**

  The test runs only on Windows in the verify-only workflow after package generation. It must reject missing `ProductVersion` and reject a version different from `$env:RELEASE_VERSION`; it must surface the query method and any Windows Installer error in `PACKAGE-EVIDENCE.txt`.

- [ ] **Step 3: Implement only the probe-proven reader.**

  Replace lines 454–517’s manual `GetType().InvokeMember(...)` sequence with the direct Automation call if the probe passed; otherwise, invoke the tested `cscript.exe` helper and parse one exact `ProductVersion=<value>` output line. Retain deterministic COM cleanup/temporary-file cleanup.

- [ ] **Step 4: Retain and complete the real installer check.**

  The verifier must run:

  ```powershell
  msiexec /a <msi> /qn TARGETDIR=<staging-directory> /norestart
  ```

  Then locate exactly one `legion-desktop.exe`, invoke that extracted executable with the canonical workspace-under-target smoke paths, require exit code zero, and append stdout/stderr plus beta evidence to the durable package evidence.

- [ ] **Step 5: Dispatch verify-only and validate the evidence artifact.**

  Expected: the report includes the Windows Installer version-reader method, `package_version=passed version=0.0.N`, exactly one extracted executable, and `smoke_exit=0`.

- [ ] **Step 6: Commit the Windows verifier repair.**

  ```powershell
  git add scripts/verify-native-package.ps1 .github/workflows/legion-release.yml
  git commit -m "fix: validate Windows MSI with supported automation"
  ```

### Task 5: Prove DMG validation on both macOS architectures and make evidence visible

**Files:**

- Modify: `scripts/verify-native-package.sh`
- Modify: `.github/workflows/legion-release.yml:563-651,689-699`
- Modify: `docs/OPERATOR_RUNBOOK.md:62-94`
- Test: macOS x64 and arm64 verify-only jobs

**Interfaces:**

- DMG verifier derives the expected stem `legion-desktop-macos-<x64|arm64>-dmg` from its `--architecture` argument.
- It reports `dmg_verify=passed`, `bundle_version=0.0.N`, `structure=passed`, and `smoke_exit=0` in package evidence.

- [ ] **Step 1: Write the DMG verifier’s failure-visible test case.**

  Make the verifier intentionally point to a non-existent DMG in a shell-level test and assert that the failure report is printed before exit. This directly prevents a repeat of the current misleading terminal line (`"diskN" ejected.`) with the actionable smoke failure hidden in the artifact.

- [ ] **Step 2: Implement guaranteed detach and evidence streaming.**

  Use a `trap` that detaches only a non-empty mount point, preserves the test failure code, and prints the completed evidence report. Do not run beta smoke from the mounted volume; copy the `.app` first, detach, then execute the copied app.

- [ ] **Step 3: Use the canonical target-contained beta workspace.**

  Run the copied `Contents/MacOS/legion-desktop` with:

  ```bash
  --workspace "$GITHUB_WORKSPACE" \
  --beta-workspace "$GITHUB_WORKSPACE/target/release-smoke/macos-$PACKAGE_ARCHITECTURE-dmg/workspace"
  ```

  Require `smoke_exit=0` for both Intel and Apple Silicon runners.

- [ ] **Step 4: Run both macOS verify-only matrix entries and inspect the artifacts.**

  Expected: checksum verification, mount/copy/detach, exact `CFBundleShortVersionString`, and beta smoke all pass independently for `macos-15-intel` and `macos-15`.

- [ ] **Step 5: Update operator guidance and commit.**

  Document the actual tested smoke procedure and that evidence is always uploaded/printed. Do not imply notarization or code signing.

  ```powershell
  git add scripts/verify-native-package.sh .github/workflows/legion-release.yml docs/OPERATOR_RUNBOOK.md
  git commit -m "fix: make DMG beta verification observable"
  ```

### Task 6: Add a release-level evidence gate and execute the first publish E2E run

**Files:**

- Modify: `.github/workflows/legion-release.yml:653-813`
- Modify: `docs/OPERATOR_RUNBOOK.md:62-94`
- Test: one successful `verify-only` run followed by one deliberate negative verifier test, then one `publish` run

**Interfaces:**

- Package jobs upload installer, checksum, `RELEASE-METADATA.toml`, `PACKAGE-EVIDENCE.txt`, and `VALIDATION-SUMMARY.toml` even after verification fails.
- `publish` downloads all five artifacts and rejects publication unless every summary has `result = "passed"`, the expected tag/version/SHA, and `smoke_exit = 0`.

- [ ] **Step 1: Add an aggregate machine-readable validation summary.**

  Each verifier writes a small TOML summary next to its evidence:

  ```toml
  schema_version = 1
  candidate_tag = "v0.0.N"
  source_sha = "<40-hex-sha>"
  format = "deb"
  architecture = "x64"
  checksum = "passed"
  metadata = "passed"
  package_version = "passed"
  structure = "passed"
  smoke_exit = 0
  result = "passed"
  ```

- [ ] **Step 2: Validate every summary in the publish job before tag creation.**

  The publish script must enumerate exactly five expected summaries, parse them with Python’s standard-library `tomllib`, verify candidate tag/source SHA/format/architecture match the prepared output, and fail before `git tag` or `gh release create` if any check is absent or non-passing.

- [ ] **Step 3: Run a negative verify-only proof.**

  On a short-lived branch, change only a verifier expectation (for example the expected package-version comparison) so a format fails. Expected: evidence is printed/uploaded, the package job fails, `publish` is skipped, and no tag/release is created. Revert the deliberate failure before the next step.

- [ ] **Step 4: Run a full successful verify-only E2E release candidate.**

  Expected: all five installer artifacts and all five summaries report pass; use `gh run view <run-id> --log-failed` and `gh run download <run-id>` to review the reports rather than relying on truncated web logs.

- [ ] **Step 5: Run the first publish-mode E2E release.**

  Expected: exactly the next canonical tag is created from the prepared SHA, the GitHub prerelease contains all five installers plus checksums, metadata, evidence, and validation summaries, and every asset’s local SHA-256 matches its companion checksum file.

- [ ] **Step 6: Run the second publish-mode E2E release.**

  Expected: tag advances by exactly one (`v0.0.N` to `v0.0.(N+1)`), no existing tag or release assets are overwritten, and all five new installer validations remain passing.

- [ ] **Step 7: Complete the release documentation and commit.**

  ```powershell
  git add .github/workflows/legion-release.yml docs/OPERATOR_RUNBOOK.md
  git commit -m "feat: gate native releases on E2E installer evidence"
  ```

## Final Verification Checklist

- [ ] `cargo fmt --all --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo test --workspace --all-targets --no-fail-fast`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo run -p xtask -- release-pipeline --dry-run`
- [ ] `cargo run -p xtask -- verify-release-pipeline`
- [ ] `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-native-package-verifiers.ps1`
- [ ] `git diff --check`
- [ ] One passing verify-only hosted run covers DEB, AppImage, MSI, macOS x64 DMG, and macOS arm64 DMG.
- [ ] One negative hosted proof confirms failed verification cannot create a tag or release.
- [ ] Two consecutive publish-mode hosted runs demonstrate canonical sequential tags and complete, verified assets.

## Plan Self-Review

- **Coverage:** The plan separates the shared beta-workspace failure, the independent Windows COM failure, the Debian maintainer defect, the skipped AppImage validation, and hidden macOS failure evidence into independently testable tasks. It also covers the manual-only/sequential-tag requirement and prohibits publish until all five actual installers pass.
- **No placeholders:** The only value intentionally left open is the Debian Maintainer identity because inventing a public support contact would create a defective artifact; Task 3 has an explicit human decision gate for it.
- **Interface consistency:** Both verifier entry points use package directory, release version, source SHA, workspace root, and target-contained smoke paths. The publish gate consumes their common `VALIDATION-SUMMARY.toml` schema.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-12-native-release-e2e-rca-and-resolution.md`. Two execution options:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, and use hosted verify-only runs as the integration gate.

2. **Inline Execution** — execute the tasks in this session using the plan’s checkpoints and wait for each hosted run before continuing.
