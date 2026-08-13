# Native Installer Release Design

## Status

Approved design for implementation on 2026-08-12.

## Goal

Extend the existing manual release path so authorized maintainers can run one GitHub Actions workflow and receive native, testable Legion desktop installers for Windows, macOS, and Linux.

The first release tag is `v0.0.1`. Each later manual run selects the next unused numeric tag in the `v0.0.N` sequence. Releases are GitHub prereleases and remain explicitly unsigned beta artifacts until platform signing credentials are provisioned.

## Scope

The workflow will create these native package formats:

- Windows: WiX MSI.
- macOS: DMG containing an application bundle.
- Linux: Debian package and AppImage.

RPM and NSIS are deferred. Portable preview archives remain available through the existing preview workflow and are not the primary release artifacts for this path.

## Packaging architecture

`cargo-packager` will be the native packaging tool. A dedicated `Packager.toml` configuration will describe the `legion-desktop` binary, product identity, package version, description, proprietary license metadata, stable reverse-DNS identifier, package formats, and platform resources.

The repository currently has no native installer metadata or icon assets. The implementation will add a minimal packaging asset set and Linux desktop entry suitable for beta testing. Packaging metadata will not move application state, editor ownership, or save authority into the packaging layer; it only describes the already-built desktop binary.

The package version will be supplied from the workflow's computed `0.0.N` release version. This keeps installer metadata aligned with the GitHub tag without silently rewriting the workspace's existing `0.1.0` Cargo metadata or committing changes to `main`.

## Release workflow

The existing `.github/workflows/legion-release.yml` will become manual-only. It will no longer run from pushed tags, because the workflow itself creates the next tag and release.

The workflow will use a non-canceling release concurrency group so simultaneous manual dispatches are serialized and cannot select the same next version.

The preparation job will fetch tags and select the highest existing `v0.0.N` value, defaulting to `0.0.0` before incrementing to `0.0.1`. It will emit the numeric version, tag, previous tag, and source commit for downstream jobs.

Native build jobs will run on native GitHub-hosted runners:

- Ubuntu builds the `.deb` and AppImage packages.
- Windows builds the `.msi` package with WiX.
- macOS builds the `.dmg` package containing the `.app` bundle. Intel and Apple Silicon builds will be represented separately when runner availability supports both targets.

Each build job will:

1. Check out the source commit selected by the preparation job.
2. Install the Rust toolchain, Linux GUI dependencies, and packaging tool dependencies.
3. Run the existing release build and relevant workspace checks.
4. Run the native packaging command with the computed release version.
5. Validate the generated package structure and package metadata.
6. Run the existing beta smoke path where the runner supports headless GUI execution.
7. Generate SHA-256 checksums and upload platform-specific workflow artifacts.

A final publish job will download all platform artifacts, create the computed tag at the checked-out source commit, and create a GitHub prerelease with the native installers, checksums, package manifests, and release metadata. The release notes will explain how to test each package and will identify the builds as unsigned beta artifacts.

## Version and release behavior

Tags use the exact sequence `v0.0.1`, `v0.0.2`, and so on. The release title and installer version use the same numeric version without the leading `v`.

The workflow will fail if the computed tag already exists or if the tag sequence contains malformed values that would make the next version ambiguous. The publish job will have only the GitHub contents permission required to create the tag and release.

The Cargo workspace version remains independent for this first beta packaging path. Release metadata will record both the GitHub release version and the workspace package version so the distinction is visible to testers and maintainers.

## Unsigned-beta policy

No private signing material will be added to the repository or required by the first workflow implementation.

- Windows MSI packages will be unsigned and may trigger SmartScreen warnings.
- macOS DMG/app packages will be unsigned and may require the tester to use the system's explicit Open flow.
- Linux packages will be locally installable but will not claim repository or package-signature verification.

The release notes and generated metadata will use the repository's existing unsigned-beta terminology. The workflow will not produce signed or production-release claims.

## Verification

Local verification will cover:

- YAML syntax and workflow expression review.
- `cargo fmt --all --check`.
- Existing release-pipeline and workspace checks required by `AGENTS.md`, to the extent the local environment provides their dependencies.
- Packaging configuration parsing and package-script dry-run checks.

GitHub-hosted verification will cover:

- Native package generation on each supported OS.
- MSI metadata and install/uninstall smoke validation on Windows.
- DMG integrity, mount, app-bundle layout, and beta smoke validation on macOS.
- Debian metadata/extraction and AppImage execution smoke validation on Ubuntu.
- SHA-256 checksum generation for every published installer.

Failures in package generation or package validation will prevent the publish job from creating a tag or release.

## Non-goals

This change does not add platform signing, notarization, auto-updating, RPM packaging, NSIS packaging, package-manager publication, or automatic version commits to `main`.
