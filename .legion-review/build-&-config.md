# Build & Config Review

Scope reviewed:
- `Cargo.toml`
- `Cargo.lock`
- `deny.toml`
- `.github/workflows/ci.yml`

Verification performed:
- `cargo metadata --locked --format-version 1 --no-deps` passed.
- `cargo run -p xtask -- check-deps` passed.
- `cargo deny check` exited successfully, but emitted duplicate-version warnings.
- Targeted stub scan found no `TODO`, `FIXME`, `HACK`, `todo!`, or `unimplemented!` markers in the reviewed files.

## Cargo.toml

### Finding 1
- Category: failure-point
- Severity: medium
- Line numbers: 36-40, 38
- Description: The workspace opts into edition 2024 but does not declare a `rust-version` in `[workspace.package]`. Combined with CI's floating `dtolnay/rust-toolchain@stable`, the effective compiler contract is implicit: older developer/automation toolchains fail late, while CI can also start accepting or rejecting code as stable Rust changes.
- Suggested fix direction: Add an explicit workspace `rust-version` matching the intended MSRV for edition 2024 and current dependency requirements, and test that version in CI (optionally alongside current stable).

## Cargo.lock

### Finding 1
- Category: failure-point
- Severity: low
- Line numbers: examples at 44-63, 554-563, 6308-6347, 6479-6504, 7072-7121, 7925-7969, 8377-8433
- Description: The lockfile contains 55 duplicate crate names resolved at multiple versions. Examples include `accesskit_consumer` 0.35.0/0.36.0, `bitflags` 1.3.2/2.11.1, `thiserror` 1.0.69/2.0.18, `toml` 0.8.23/0.9.12, `wasmparser` 0.244.0/0.247.0/0.248.0/0.252.0, `windows-sys` 0.45.0/0.52.0/0.59.0/0.60.2/0.61.2, and `wit-parser` 0.244.0/0.247.0/0.248.0. `cargo deny check` reports these only as warnings, so the dependency gate currently allows this state.
- Suggested fix direction: Decide which duplicates are acceptable transitive baggage and document them explicitly in `deny.toml`; for the rest, align direct dependency versions or update upstream crates. Once the intentional set is small and documented, consider promoting duplicate-version enforcement from warning to deny.

## deny.toml

### Finding 1
- Category: failure-point
- Severity: high
- Line numbers: 10, 51-53
- Description: The dependency policy treats yanked crates, unknown registries, and unknown git sources as warnings (`yanked = "warn"`, `unknown-registry = "warn"`, `unknown-git = "warn"`). Because CI runs cargo-deny as a gate, these settings allow the gate to pass even when the dependency graph includes yanked packages or sources outside the expected registry/source policy.
- Suggested fix direction: Change these to deny/fail-level policy for release and protected-branch CI. If a temporary exception is needed, use a narrow documented allow/ignore entry with an owner and removal condition rather than globally warning.

### Finding 2
- Category: failure-point
- Severity: medium
- Line numbers: 14-16
- Description: `multiple-versions = "warn"` is too weak for a governance gate and there is no explicit exception list explaining which duplicate crates are acceptable. The current lockfile has 55 duplicate crate names, and `cargo deny check` still exits successfully.
- Suggested fix direction: Add explicit `skip`/exception entries for unavoidable duplicates with rationale, resolve avoidable duplicates, and move `multiple-versions` to deny when the reviewed baseline is ready.

## .github/workflows/ci.yml

### Finding 1
- Category: failure-point
- Severity: medium
- Line numbers: 1-10
- Description: The workflow does not define an explicit `permissions` block. Jobs in this workflow only need repository checkout/read access, but without a workflow-level `permissions: contents: read`, token scope depends on repository/org defaults and can be broader than necessary.
- Suggested fix direction: Add a top-level least-privilege permissions block, for example `permissions: { contents: read }`, and grant broader scopes only on jobs that truly need them.

### Finding 2
- Category: failure-point
- Severity: medium
- Line numbers: 21-25, 40-42, 70-72, 80-82, 159-164, 180-187
- Description: Third-party GitHub Actions are referenced by mutable major/channel tags (`actions/checkout@v4`, `dtolnay/rust-toolchain@stable`, `swatinem/rust-cache@v2`, `actions/upload-artifact@v4`, and `EmbarkStudios/cargo-deny-action@v2`). A compromised or unexpectedly changed tag can alter CI behavior for both validation and tagged release workflows.
- Suggested fix direction: Pin third-party actions to full commit SHAs (optionally with comments naming the upstream version), and maintain them through an automated dependency-update process.

### Finding 3
- Category: failure-point
- Severity: medium
- Line numbers: 24-25, 183-184
- Description: Both validation and release jobs install a floating stable Rust toolchain without tying it to a checked-in `rust-toolchain.toml` or the workspace MSRV. This makes build and release behavior time-dependent and can hide whether the project actually supports the intended compiler floor.
- Suggested fix direction: Add a checked-in `rust-toolchain.toml` or explicit toolchain version for release reproducibility, and add a separate CI lane for current stable if forward-compatibility coverage is desired.
