# GUI Phase 6 package runbook

## Scope

This runbook covers the deterministic Windows desktop packaging path for `legion-desktop`. It packages the existing executable and metadata only; it does not create an installer, introduce new dependencies, or change runtime ownership boundaries.

## Dry Run

```powershell
scripts/package-windows.ps1 -DryRun
```

Expected behavior:

- Prints the repository root, cargo command, source executable path, package output path, and manifest path.
- Does not run `cargo build`.
- Does not create package directories or copy executables.

## Debug Package

```powershell
scripts/package-windows.ps1
```

Expected behavior:

- Runs `cargo build -p legion-desktop`.
- Copies `target/debug/legion-desktop.exe` into `target/gui-phase6-package/legion-desktop.exe`.
- Writes `target/gui-phase6-package/legion-desktop-package-manifest.txt`.

## Release Package

```powershell
scripts/package-windows.ps1 -Release
```

Expected behavior:

- Runs `cargo build -p legion-desktop --release`.
- Copies `target/release/legion-desktop.exe` into `target/gui-phase6-package/legion-desktop.exe`.
- Writes `target/gui-phase6-package/legion-desktop-package-manifest.txt`.

## Boundary Notes

- Packaging metadata is path and command metadata only.
- Package manifests must not include editor text, file previews, source bodies, secrets, or workspace mutation payloads.
- Runtime state remains owned by `legion-app`; `legion-desktop` remains an adapter.
