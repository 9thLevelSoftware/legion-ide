# WS-MANUAL-02 Reference Workspaces

## Purpose

Define the reference workspaces against which all WS-MANUAL-02 scale tasks are measured.
These workspaces are generated or identified, not shipped as binary fixtures.

## Reference workspaces

### RW-1: Legion Repository (self-hosted)

- Type: Real Cargo workspace
- Approximate file count: ~1,000 files
- Approximate total size: ~20MB source
- Use: GP-1 daily-driver baseline, dogfood target
- How to obtain: the repo itself (`cargo metadata` provides the workspace root)

### RW-2: 100K-File Generated Repository

- Type: Synthetic generated workspace
- File count: 100,000 `.rs` and `.toml` files across 500 directories
- Approximate size per file: 200 bytes (stub modules)
- Total size: ~20MB
- Use: workspace tree open, watcher burst, search scalability
- Generation: `xtask generate-test-workspace --files 100000 --dirs 500 --target target/test-workspaces/rw-2`

### RW-3: 100MB Single File

- Type: Synthetic single large file
- Size: exactly 100MB (104,857,600 bytes) of repeating ASCII lines
- Line count: ~2,621,440 lines (40 bytes per line)
- Use: streaming viewport, degraded mode, memory ceiling
- Generation: programmatic in-test generation (no disk fixture needed for text model tests)

### RW-4: Large Cargo Workspace

- Type: Synthetic or real large Cargo workspace
- Package count: 50 workspace members
- Use: Cargo metadata parsing, LSP project root discovery
- Generation: `xtask generate-test-workspace --cargo-workspace --packages 50 --target target/test-workspaces/rw-4`

### RW-5: Mixed Binary/Text Workspace

- Type: Synthetic workspace with intentional binary files
- Contents: 100 `.rs` files, 10 `.png` (random bytes), 5 `.exe` (random bytes), 2 `.pdf` (random bytes), 1 `.tar.gz` (random bytes)
- Use: binary detection, preview refusal, search skip behavior
- Generation: programmatic in-test generation

## Threshold definitions

| Metric | Budget | Measured against |
| --- | --- | --- |
| 100MB file open (buffer creation) | < 5s | RW-3 |
| 100MB viewport slice (40 visible lines) | < 1ms | RW-3 |
| 100MB single keystroke edit | < 50ms | RW-3 |
| 100MB memory ceiling (buffer + index) | < 400MB | RW-3 |
| Workspace tree open (100K files) | non-blocking return | RW-2 |
| Search cancellation resource release | immediate (< 100ms) | RW-1 |
| Watcher burst (1000 events in 100ms) | debounced to < 10 notifications | RW-2 |
