# Native Installer Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a manual GitHub Actions release workflow that computes `v0.0.N` tags and publishes unsigned beta Windows MSI, macOS DMG, and Linux DEB/AppImage installers for `legion-desktop`.

**Architecture:** Keep application code and workspace ownership unchanged. Add a dedicated `cargo-packager` configuration plus thin platform scripts that build the existing `legion-desktop` binary, produce one native package per target, normalize package names, emit metadata/checksums, and let a final workflow job create the tag and GitHub prerelease only after every platform job passes.

**Tech Stack:** Rust/Cargo workspace, `cargo-packager` 0.11.8, WiX MSI, macOS `hdiutil`, Debian/AppImage tooling, Bash, PowerShell, GitHub Actions, GitHub CLI.

## Global Constraints

- Releases are manual-only through `workflow_dispatch`; pushed tags must not trigger the release workflow.
- Release tags are exactly `v0.0.1`, `v0.0.2`, and so on; the first run defaults to `v0.0.1`.
- The first native package set is Windows WiX MSI, macOS DMG/app, and Linux Debian/AppImage; RPM and NSIS are out of scope.
- Package metadata uses the computed numeric version `0.0.N`; the workspace Cargo version remains `0.1.0` and is not silently committed or rewritten by CI.
- Release outputs are unsigned beta artifacts and must not claim code signing, notarization, or production status.
- No private signing keys, certificates, tokens, or notarization credentials may be committed.
- The workflow must serialize manual runs with a non-canceling concurrency group so two runs cannot select the same next tag.
- Existing UI projection, editor-state ownership, proposal-mediated saves, and observability contracts are outside this change and must not be modified.
- Required local verification remains the phase-gate set in `AGENTS.md`; missing local tools must be reported rather than hidden.

## File map

- Create `packaging/Packager.toml` as the shared native package configuration template.
- Create `packaging/icons/legion.svg` and generated platform icon files required by AppImage, MSI, and macOS packaging.
- Create `packaging/linux/legion.desktop` for the Linux application entry.
- Create `scripts/package-native.sh` for Linux/macOS packaging and `scripts/package-native.ps1` for Windows packaging.
- Modify `xtask/release-pipeline.example.toml` so descriptor names and artifact extensions match native package outputs.
- Modify `.github/workflows/legion-release.yml` to implement preparation, native package matrix, validation, and publish jobs.
- Modify `docs/OPERATOR_RUNBOOK.md` with manual dispatch, package locations, test commands, and unsigned-beta warnings.
- Do not modify Rust application source or the workspace version.

---

### Task 1: Add native packaging configuration and deterministic package scripts

**Files:**
- Create: `packaging/Packager.toml`
- Create: `packaging/icons/legion.svg`
- Create: `packaging/icons/legion.ico`
- Create: `packaging/icons/legion.icns`
- Create: `packaging/icons/legion-512.png`
- Create: `packaging/icons/legion-256.png`
- Create: `packaging/icons/legion-128.png`
- Create: `packaging/icons/legion-64.png`
- Create: `packaging/icons/legion-32.png`
- Create: `packaging/icons/legion-16.png`
- Create: `packaging/linux/legion.desktop`
- Create: `scripts/package-native.sh`
- Create: `scripts/package-native.ps1`

**Interfaces:**
- Both scripts accept `--version 0.0.N`, one platform `--format` (`wix`, `dmg`, `deb`, or `appimage`), an output directory, and a dry-run option.
- Both scripts build `legion-desktop` in release mode and invoke `cargo packager` with a generated config whose `version` is the supplied numeric release version.
- Both scripts emit exactly one normalized package per invocation:
  - `legion-desktop-windows-x64-msi.msi`
  - `legion-desktop-macos-x64-dmg.dmg` or `legion-desktop-macos-arm64-dmg.dmg`
  - `legion-desktop-linux-x64-deb.deb`
  - `legion-desktop-linux-x64-appimage.AppImage`
- Both scripts write `RELEASE-METADATA.toml` beside the package with `release_version`, `workspace_version`, `git_sha`, `platform`, `architecture`, `format`, and `signer_status = "unsigned-beta/no-os-code-signing"`.

- [ ] **Step 1: Add the shared packager configuration.**

Use `cargo-packager` fields `product-name`, `version`, `binaries`, `binaries-dir`, `out-dir`, `identifier`, `description`, `publisher`, `copyright`, `category`, `icons`, and platform-specific package settings. Use `binaries = [{ path = "legion-desktop", main = true }]` with a release `binaries-dir`, and leave signing fields absent. Keep the template version at `0.0.0`; the scripts must write the actual `0.0.N` into a generated file under `target/native-package/` before invoking the packager.

- [ ] **Step 2: Add the minimal icon source and generate platform assets.**

Create a square, simple Legion mark in `packaging/icons/legion.svg`, then generate the checked-in ICO, ICNS, and PNG sizes with the pinned `svg-to-icons` tool:

```text
cargo install svg-to-icons --version 0.3.0 --locked
New-Item -ItemType Directory packaging/icon-source -Force
Copy-Item packaging/icons/legion.svg packaging/icon-source/icon.svg
Push-Location packaging/icon-source
cargo-svgtoicons --all
Pop-Location
Move-Item packaging/icon-source/icons/icon.ico packaging/icons/legion.ico
Move-Item packaging/icon-source/icons/icon.icns packaging/icons/legion.icns
Move-Item packaging/icon-source/icons/icon-512.png packaging/icons/legion-512.png
Move-Item packaging/icon-source/icons/icon_256x256.png packaging/icons/legion-256.png
Move-Item packaging/icon-source/icons/icon_128x128.png packaging/icons/legion-128.png
Move-Item packaging/icon-source/icons/icon_64x64.png packaging/icons/legion-64.png
Move-Item packaging/icon-source/icons/icon_32x32.png packaging/icons/legion-32.png
Move-Item packaging/icon-source/icons/icon_16x16.png packaging/icons/legion-16.png
```

Keep the generated files named `legion.ico`, `legion.icns`, `legion-512.png`, `legion-256.png`, `legion-128.png`, `legion-64.png`, `legion-32.png`, and `legion-16.png`. The AppImage configuration must point at `legion-512.png`, and the Windows/macOS configurations must point at the matching ICO/ICNS assets.

- [ ] **Step 3: Add the Linux desktop entry.**

Create `packaging/linux/legion.desktop` with these concrete keys:

```ini
[Desktop Entry]
Type=Application
Name=Legion
Comment=Legion IDE desktop application
Exec=legion-desktop
Icon=legion
Terminal=false
Categories=Development;IDE;
```

Wire the desktop entry and icon into the Linux package configuration so the `.deb` installs them under the standard applications and icon locations.

- [ ] **Step 4: Implement the Unix packaging script.**

`scripts/package-native.sh` must use `set -euo pipefail`, reject versions that do not match `^0\\.0\\.[0-9]+$`, reject formats outside `dmg|deb|appimage`, and never write outside the requested output directory plus `target/native-package/`. It must:

```text
cargo build --release -p legion-desktop
mkdir -p target/native-package
# render Packager.toml with the requested version, target format, release binary path, and output directory
cargo packager --release --config target/native-package/Packager.toml
# locate the single generated package for the requested format
# rename it to the exact legion-desktop-${PLATFORM}-${ARCH}-${FORMAT}.${EXTENSION} contract
# write RELEASE-METADATA.toml and a .sha256 file
```

For `--dry-run`, validate the arguments, render the generated config, print the planned package path, and exit without compiling or writing package output.

- [ ] **Step 5: Implement the Windows packaging script.**

`scripts/package-native.ps1` must use `$ErrorActionPreference = "Stop"`, validate the same version and format contract, build `cargo build --release -p legion-desktop`, render the same config shape with Windows paths, run `cargo packager --release --config target/native-package/Packager.toml`, normalize the MSI to `legion-desktop-windows-x64-msi.msi`, and write the same metadata/checksum files. Its `-DryRun` path must not build or write package output.

- [ ] **Step 6: Run script-level checks.**

Run:

```text
bash -n scripts/package-native.sh
bash scripts/package-native.sh --version 0.0.1 --format appimage --dry-run
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-native.ps1 -Version 0.0.1 -Format wix -DryRun
```

Expected: syntax succeeds, both dry-runs print the computed package contract, and neither dry-run creates an installer or invokes Cargo.

### Task 2: Align release descriptors and operator documentation

**Files:**
- Modify: `xtask/release-pipeline.example.toml`
- Modify: `docs/OPERATOR_RUNBOOK.md`

**Interfaces:**
- Descriptor names and artifact suffixes match the files produced by Task 1.
- Operators can start a release with `gh workflow run legion-release.yml` and can identify the package format, unsigned status, and local smoke command from the runbook.

- [ ] **Step 1: Replace placeholder target descriptors.**

Set `dist_tool = "cargo-packager"` and declare these installer targets with matching names, platforms, targets, and artifact extensions:

```text
legion-desktop-macos-x64-dmg       -> dmg
legion-desktop-macos-arm64-dmg     -> dmg
legion-desktop-windows-x64-msi     -> msi
legion-desktop-linux-x64-deb       -> deb
legion-desktop-linux-x64-appimage  -> AppImage
```

Use the Task 1 scripts in each `build_command` and use concrete verification commands such as `hdiutil verify`, `msiexec` metadata/install checks, `dpkg-deb --info`, and AppImage extraction/launch checks. Keep signer references descriptive only and preserve the existing unsigned-beta status.

- [ ] **Step 2: Document the manual release procedure.**

Add a native release section to `docs/OPERATOR_RUNBOOK.md` covering:

```text
gh workflow run legion-release.yml
gh run list --workflow legion-release.yml
```

Document the generated GitHub assets, the `v0.0.N` numbering rule, the Windows SmartScreen/macOS Gatekeeper warnings, Linux local installation commands, and the fact that the workflow does not add signing credentials or rewrite `Cargo.toml`.

- [ ] **Step 3: Verify the descriptor contract.**

Run:

```text
cargo test -p xtask --test release_pipeline
cargo run -p xtask -- release-pipeline --dry-run --channel preview
cargo run -p xtask -- verify-release-pipeline
```

Expected: descriptor filenames are collision-free, the dry-run writes the version stamp and five target descriptors, and verification reports no failed descriptors.

### Task 3: Replace the release workflow with manual native packaging and publish

**Files:**
- Modify: `.github/workflows/legion-release.yml`

**Interfaces:**
- `prepare.outputs.version`, `prepare.outputs.tag`, `prepare.outputs.source_sha`, and `prepare.outputs.previous_tag` are consumed by every package and publish job.
- Each package matrix entry uploads a named artifact containing one normalized installer, its checksum, metadata, and package evidence.
- The publish job creates the tag and GitHub prerelease only after all package jobs pass.

- [ ] **Step 1: Change triggers, permissions, and concurrency.**

Use this workflow shell:

```yaml
on:
  workflow_dispatch:

permissions:
  contents: write

concurrency:
  group: legion-release-serial
  cancel-in-progress: false
```

Do not retain `push.tags` in this workflow. Keep the release job independent from the standing PR gates.

- [ ] **Step 2: Add the preparation job.**

On `ubuntu-latest`, check out with `fetch-depth: 0`, fetch tags, inspect only tags matching `v0.0.*`, reject malformed matching tags, calculate the maximum numeric suffix plus one, and write:

```text
version=0.0.N
tag=v0.0.N
previous_tag=v0.0.(N-1) or empty
source_sha=${{ github.sha }}
```

Before publishing, the final job must re-check that the computed tag does not exist.

- [ ] **Step 3: Add the validation job.**

Run the existing release checks once on `ubuntu-latest` after installing the Linux GUI dependencies and Rust stable:

```text
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
```

This job must depend on `prepare` and must complete before packaging jobs start.

- [ ] **Step 4: Add the native package matrix.**

Use these matrix entries:

```yaml
include:
  - os: ubuntu-latest
    platform: linux
    formats: deb,appimage
  - os: windows-latest
    platform: windows
    formats: wix
  - os: macos-13
    platform: macos
    architecture: x64
    formats: dmg
  - os: macos-14
    platform: macos
    architecture: arm64
    formats: dmg
```

Install `cargo-packager --locked --version 0.11.8`; install WiX on Windows if `wix --version` is not already available; install the existing Linux GUI build dependencies plus AppImage runtime/build dependencies on Ubuntu. Invoke Task 1 once for each required format and upload the exact normalized outputs with `actions/upload-artifact@v4`.

- [ ] **Step 5: Add package-specific smoke validation.**

Use these checks before uploading each package:

```text
Windows: set `MSI_PATH` and `STAGING_DIR`, run `msiexec /a "$MSI_PATH" /qn TARGETDIR="$STAGING_DIR"`, then run `"$STAGING_DIR\legion-desktop.exe" --beta-smoke --duration-ms 1500`.
macOS: set `DMG_PATH`, run `hdiutil verify "$DMG_PATH"`, attach it, copy the `.app` to a temporary directory, and run `Contents/MacOS/legion-desktop --beta-smoke --duration-ms 1500`.
Linux DEB: set `DEB_PATH` and `STAGING_DIR`, run `dpkg-deb --info "$DEB_PATH"`, extract with `dpkg-deb -x "$DEB_PATH" "$STAGING_DIR"`, and run the staged binary under `xvfb-run --auto-servernum` with `--beta-smoke --duration-ms 1500`.
Linux AppImage: set `APPIMAGE_PATH`, run `chmod +x "$APPIMAGE_PATH"`, and launch `"$APPIMAGE_PATH" --appimage-extract-and-run --beta-smoke --duration-ms 1500` under `xvfb-run`.
```

The smoke command may be marked best-effort only when GUI initialization is the sole failure; package structure, metadata, and checksum checks must remain hard failures.

- [ ] **Step 6: Add the publish job.**

Download all package artifacts, write a release notes file that includes the tag, source SHA, previous tag, unsigned-beta warning, and per-platform install/test instructions, then create the release with the repository token:

```text
gh release create "$TAG" --target "$SOURCE_SHA" --title "Legion $TAG" --prerelease --notes-file release-notes.md
gh release upload "$TAG" dist/*
```

Set `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` on the publish job. The job must fail before `gh release create` if the tag exists, if any expected installer is missing, or if a checksum does not match its installer.

### Task 4: Run release automation verification and handoff checks

**Files:**
- Verify: `.github/workflows/legion-release.yml`
- Verify: `packaging/Packager.toml`
- Verify: `scripts/package-native.sh`
- Verify: `scripts/package-native.ps1`
- Verify: `xtask/release-pipeline.example.toml`
- Verify: `docs/OPERATOR_RUNBOOK.md`

- [ ] **Step 1: Run static workflow checks.**

Run the strongest available local workflow validator, preferring `actionlint`:

```text
actionlint .github/workflows/legion-release.yml
```

If `actionlint` is unavailable, parse the YAML with the installed YAML parser and manually verify all `${{ needs.prepare.outputs.* }}` references, job dependencies, matrix keys, artifact paths, and permissions. Do not claim expression validation passed when only YAML parsing ran.

- [ ] **Step 2: Run repository gates affected by the change.**

Run:

```text
git diff --check
cargo fmt --all --check
cargo run -p xtask -- release-pipeline --dry-run --channel preview
cargo run -p xtask -- verify-release-pipeline
cargo test -p xtask --test release_pipeline
```

Then run the full workspace checks from `AGENTS.md` if the local toolchain has all dependencies. Record any unavailable `cargo-deny`, GUI, or packaging tool separately.

- [ ] **Step 3: Inspect the final diff and working tree.**

Run:

```text
git diff --stat
git diff -- .github/workflows/legion-release.yml packaging scripts/package-native.sh scripts/package-native.ps1 xtask/release-pipeline.example.toml docs/OPERATOR_RUNBOOK.md
git status --short
```

Confirm that no private signing material, generated `target/` output, or unrelated application changes are present. Do not create a commit unless the user explicitly authorizes it.
