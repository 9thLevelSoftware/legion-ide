# Native release E2E RCA — August 13, 2026

This document preserves the first full native-release incident record for the manual GitHub Actions workflow.

Run URL: <https://github.com/9thLevelSoftware/legion-ide/actions/runs/31661117478>
Repository: `9thLevelSoftware/legion-ide`
Branch: `main`
Source SHA: `aa8ee06a00af6fc477f3259d97f04befc56946e0`

Current failing layer: package validation and release orchestration. Package generation is not the incident's current failing layer.

Evidence for that conclusion:

- `Package Linux installers` succeeded before Linux validation failed.
- `Package Windows MSI` succeeded before Windows validation failed.
- `Package macOS DMG` succeeded on both `macos-15-intel` and `macos-15` before smoke validation failed.
- Five package artifacts were uploaded from the failing run, which proves installer generation completed even though the overall workflow failed.
- `Publish unsigned beta prerelease` was skipped because the validation gate failed before any tag or GitHub Release mutation could run.

## Uploaded artifacts from the preserved failed run

| Artifact | GitHub artifact ID | Digest |
| --- | ---: | --- |
| `legion-desktop-linux-x64-deb` | `9166528770` | `sha256:6ab8a8930287276c28da61d61d8e30cf6ce5ae360a869d83a2cc8fac8765b3a6` |
| `legion-desktop-linux-x64-appimage` | `9166529472` | `sha256:a1e6ce066ef2860ae074de1c5977f810a934c2d59b55a0fcea4f19a9a6f69fa9` |
| `legion-desktop-windows-x64-msi` | `9166695110` | `sha256:6b3828bc88b840e4ff495c095052604e69e571db86f27f82933c853f452ae750` |
| `legion-desktop-macos-arm64-dmg` | `9166479530` | `sha256:71c270b0a9e8ae901304ae4cb81765b64c13c25419c7008186a7b05c804f205d` |
| `legion-desktop-macos-x64-dmg` | `9166805449` | `sha256:9eaac71b225576632f2bc3baad5d24109deb33c0c659c0b56354fc3c969169dc` |

## Root causes captured from run 31661117478

| Finding | Preserved evidence | Workflow line range | Application containment check |
| --- | --- | --- | --- |
| Linux DEB validation completes metadata and structure checks, then hard-fails in smoke because the beta workspace is outside the trusted workspace `target/` tree. | Downloaded evidence records `package_version=passed`, `structure=passed`, `smoke_exit=1`, and `beta workspace '/home/runner/work/_temp/legion-deb-smoke/workspace' must resolve inside '/home/runner/work/legion-ide/legion-ide/target'`. The same evidence still preserves the Debian metadata defect: `dpkg-deb --info` warns `missing 'Maintainer' field`. | [`.github/workflows/legion-release.yml` lines 248-308](https://github.com/9thLevelSoftware/legion-ide/blob/aa8ee06a00af6fc477f3259d97f04befc56946e0/.github/workflows/legion-release.yml#L248-L308) | Triggered by the beta-workspace containment guard in [`crates/legion-desktop/src/beta.rs` lines 442-488](https://github.com/9thLevelSoftware/legion-ide/blob/aa8ee06a00af6fc477f3259d97f04befc56946e0/crates/legion-desktop/src/beta.rs#L442-L488), specifically the requirement that the resolved beta workspace remain under `<workspace-root>/target`. |
| Windows MSI validation fails inside the PowerShell COM reflection reader, before smoke or publish logic. | `InvokeMember("OpenDatabase", ...)` throws `Type mismatch. (0x80020005 (DISP_E_TYPEMISMATCH))`. | [`.github/workflows/legion-release.yml` lines 393-546](https://github.com/9thLevelSoftware/legion-ide/blob/aa8ee06a00af6fc477f3259d97f04befc56946e0/.github/workflows/legion-release.yml#L393-L546) | Not reached. The validator exits before the MSI smoke command can hit the beta-workspace containment guard in [`crates/legion-desktop/src/beta.rs` lines 442-488](https://github.com/9thLevelSoftware/legion-ide/blob/aa8ee06a00af6fc477f3259d97f04befc56946e0/crates/legion-desktop/src/beta.rs#L442-L488). |
| macOS x64 DMG validation succeeds on checksum and structure, then fails because the smoke workspace is outside the trusted workspace `target/` tree. | Uploaded evidence records `smoke_exit=1` and `beta workspace '/Users/runner/work/_temp/legion-dmg-smoke/workspace' must resolve inside '/Users/runner/work/legion-ide/legion-ide/target'`. | [`.github/workflows/legion-release.yml` lines 563-651](https://github.com/9thLevelSoftware/legion-ide/blob/aa8ee06a00af6fc477f3259d97f04befc56946e0/.github/workflows/legion-release.yml#L563-L651) | Triggered by the beta-workspace containment guard in [`crates/legion-desktop/src/beta.rs` lines 442-488](https://github.com/9thLevelSoftware/legion-ide/blob/aa8ee06a00af6fc477f3259d97f04befc56946e0/crates/legion-desktop/src/beta.rs#L442-L488), specifically the requirement that the resolved beta workspace remain under `<workspace-root>/target`. |
| macOS arm64 DMG validation fails for the same reason as macOS x64; the architecture changes, but the containment break is identical. | Uploaded evidence records `smoke_exit=1` and the same `must resolve inside ... /target` error for `/Users/runner/work/_temp/legion-dmg-smoke/workspace`. | [`.github/workflows/legion-release.yml` lines 563-651](https://github.com/9thLevelSoftware/legion-ide/blob/aa8ee06a00af6fc477f3259d97f04befc56946e0/.github/workflows/legion-release.yml#L563-L651) | Triggered by the same beta-workspace containment guard in [`crates/legion-desktop/src/beta.rs` lines 442-488](https://github.com/9thLevelSoftware/legion-ide/blob/aa8ee06a00af6fc477f3259d97f04befc56946e0/crates/legion-desktop/src/beta.rs#L442-L488). |

## Additional incident notes

- The Debian package has two distinct issues in the preserved run: a metadata defect (`Maintainer` missing) and the actual hard job failure, which is the out-of-`target/` beta-workspace smoke path.
- The AppImage installer was built and uploaded, but its smoke validation did not run in this incident because the shared Linux job stopped after the DEB smoke failure.
- The AppImage smoke block in the preserved workflow still constructs `--beta-workspace` under `$RUNNER_TEMP`, so it carries the same out-of-`target/` containment risk as the macOS DMG validators even though that defect was not executed in run `31661117478`.
- The preserved run already demonstrates the publish safety boundary that existed accidentally through failure: the publish job never ran because the validation layer failed first. The repaired workflow should make that boundary explicit through a manual `mode` input rather than relying on downstream failure.
