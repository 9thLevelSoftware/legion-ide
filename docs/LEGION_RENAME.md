# Legion Rename

The repository now uses the canonical Legion namespace for active code, packages, scripts, and docs.

## Crate mapping

All previous `devil-*` workspace crates were renamed to matching `legion-*` crates. Rust imports now use the corresponding `legion_*` crate names.

Examples:

- `devil-protocol` -> `legion-protocol`
- `devil-app` -> `legion-app`
- `devil-ui` -> `legion-ui`
- `devil-desktop` -> `legion-desktop`
- `devil-cli` -> `legion-cli`

## Commands

Use the Legion package names for active development:

```powershell
cargo run -p legion-cli -- doctor
cargo run -p legion-cli -- evidence check --phase gui-phase8
cargo run -p legion-desktop -- --workspace .
cargo test -p legion-app --test workspace_vfs_integration
```

The Windows desktop package now produces `legion-desktop.exe`.

## Compatibility

`LEGION_*` environment variables are canonical. Runtime provider setup may still silently read old product-prefixed environment variables as a fallback for existing local machines, but active docs and help output should show only `LEGION_*` names.

New storage and retention metadata should use `legion-*` identifiers. Readers may accept old persisted identifiers when needed so existing local state remains readable after the rename.

## Historical artifacts

Archived evidence and historical phase outputs were not rewritten. Files under `plans/evidence/**`, `.planning/phases/**`, generated caches, and old command-output logs may still contain earlier command names because changing them would falsify the record of what originally ran.

Evidence validators keep accepting those historical command markers when they are checking archived evidence, but current command constants, help output, scripts, and CI should use only Legion names.
