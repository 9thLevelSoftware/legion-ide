# Unsigned local package for the native input acceptance harness — 2026-09-08

## What this record is

**Classification: preparation for an unsigned local package build. Not a
package build, not release qualification, and not product acceptance.**

Round `r07`, packet `s1-02e`, worktree `D:/legion-ide-completion`, branch
`codex/full-product-resume`, `HEAD` `21dfc0be05c48a78c7146bfaca1efb00108fbd9e`.

The owner authorised building an unsigned local package so that
`xtask native-product-acceptance` has a packaged product to launch instead of a
development build. This record states exactly what that authorisation produced
in this packet and what it did not.

**No MSI was built in this packet, and nothing was staged.** The staging entry
point the harness needs was written and its refusals were tested against
synthetic fixtures; the package build itself was not run, for the reason
recorded under [Why no package exists yet](#why-no-package-exists-yet).

## What was produced

- [`scripts/stage-native-acceptance-package.ps1`](../../../scripts/stage-native-acceptance-package.ps1) —
  new. It copies the product payload that `msiexec /a` extracted from a verified
  MSI into `target/native-input-acceptance/package/` and writes
  `STAGING-EVIDENCE.toml` beside it. It builds nothing, signs nothing, publishes
  nothing, and runs no acceptance harness.
- Eight new contract tests in
  [`scripts/test-native-package-verifiers.ps1`](../../../scripts/test-native-package-verifiers.ps1),
  run against synthetic fixtures with no real installer, no `msiexec` and no
  product build.
- The staging entry point documented in
  [`docs/OPERATOR_RUNBOOK.md`](../../../docs/OPERATOR_RUNBOOK.md), in the
  paragraph that already documents the two package verifiers.

### Contract test run

```
pwsh -NoProfile -File scripts/test-native-package-verifiers.ps1
```

Result: `passed=21 failed=0 skipped=0`. Thirteen of those tests are the existing
verifier tests, unchanged and still passing with their assertions unweakened;
eight are new and cover the staging step:

| Test | What it asserts |
| --- | --- |
| `stage script rejects a source under target/debug` | non-zero exit and a refusal naming the development-build directories, for `target/debug` and, in a run without `-DryRun`, for `target/native-package/cargo-target/debug`, which creates no destination directory |
| `stage script rejects a source under target/release` | the same refusal for `target/release`, for the packager's own `target/native-package/cargo-target/release`, and for an `out/renamed/release` directory carrying a cargo `.fingerprint` marker — each of the last two without `-DryRun`, so the "no destination created" assertion is a real one — plus that a `target/release-smoke/...` source is still accepted, because the name rule matches whole path segments rather than substrings |
| `stage script rejects an MSI whose sha256 does not match its sidecar` | non-zero exit naming the expected hash, on a fixture whose payload tree is otherwise valid |
| `stage script rejects an extraction tree with no legion-desktop.exe` | non-zero exit reporting a candidate count of 0 |
| `stage script rejects an extraction tree with more than one legion-desktop.exe` | non-zero exit reporting a count of 2 and naming every candidate |
| `stage script stages the whole payload directory and writes STAGING-EVIDENCE.toml` | the nested payload files are copied, the staged executable is byte-identical to the extracted one, and the evidence carries both recomputed hashes |
| `stage evidence records signed = false and the exact signing prerequisite` | `signed = false`, `signer_status` verbatim from `RELEASE-METADATA.toml`, and the exact signing and clean-machine prerequisite strings |
| `stage script is idempotent across two runs` | a second run replaces the destination rather than merging into it, and every evidence value except `staged_utc` is identical |

## Why no package exists yet

Building the package means running the repository's own entry point:

```
pwsh -NoProfile -File scripts/package-native.ps1 -Version 0.0.1 -Format wix -OutDir target/native-package/output
```

That script runs `cargo build --release -p legion-desktop` and `cargo packager`,
so it is a cargo command. This packet's role is `implementer (terra)`, not the
round's cargo lane, and the round rule forbids running cargo, rustc or xtask
from any other role. At the time of this packet the lane was occupied: two
`cargo test --workspace --locked` processes were running on this host
(`Get-CimInstance Win32_Process -Filter "Name='cargo.exe'"`), and
`.superpowers/sdd/2026-09-04-full-product-completion/cargo.lock-owner` did not
exist. The rule for a busy lane is to stop and report blocked rather than wait,
so the build was not run and no MSI was produced.

`0.0.1` is the version to use: this worktree carries no git tags, and
`target/native-package/output/` does not exist, so no `0.0.N` value has been
consumed. It is a local build version, not a release; the workspace version in
`Cargo.toml` is `0.1.0` and is a separate axis.

Host prerequisites were checked and are present, so the build is expected to be
runnable rather than blocked on tooling: `cargo-packager.exe` is installed in
`~/.cargo/bin`, the WiX toolset is already cached at
`%LOCALAPPDATA%\.cargo-packager\WixTools` (so the packager needs no network
fetch for it), and `msiexec.exe` exists for the verifier's administrative
extraction. That is an inspection of the host, not a build result; whether the
package builds is decided by running it.

### The remaining sequence

Once the cargo lane is free, in this order, from `D:/legion-ide-completion`:

```
pwsh -NoProfile -File scripts/package-native.ps1 -Version 0.0.1 -Format wix -OutDir target/native-package/output
pwsh -NoProfile -File scripts/verify-native-package.ps1 -PackageDir D:/legion-ide-completion/target/native-package/output -ReleaseVersion 0.0.1 -SourceSha 21dfc0be05c48a78c7146bfaca1efb00108fbd9e -WorkspaceRoot D:/legion-ide-completion
pwsh -NoProfile -File scripts/stage-native-acceptance-package.ps1 -PackageDir D:/legion-ide-completion/target/native-package/output -StagingSource D:/legion-ide-completion/target/release-smoke/windows-x64-msi/staging -DestinationDir D:/legion-ide-completion/target/native-input-acceptance/package
```

Only the first is a cargo command. The verifier must report `result = "passed"`
and `smoke_exit = 0` in `VALIDATION-SUMMARY.toml` before the staging step runs;
`result = "not-run"` for any check means the verifier stopped before reaching it
and is not a pass. Nothing may be staged from an unverified MSI.

## Signing: nothing here is signed, and nothing here changes that

`scripts/package-native.ps1` writes `RELEASE-METADATA.toml` with a machine-written
line reading:

```
signer_status = "unsigned-beta/no-os-code-signing"
```

When a package exists, that line — read from the generated file, not asserted
here — is the evidence that the artifact carries no code signature. The staging
script copies that value verbatim into `STAGING-EVIDENCE.toml` and records
`signed = false` beside it, so an unsigned artifact cannot be staged as anything
else.

No signing credential, certificate, key, notarization tool, provider or feed
exists on this host or in the retained facts. Every signing, notarisation, trust,
channel and release row — `COMP-DIST-003`, `COMP-DIST-005`, `COMP-DIST-007`,
`COMP-DIST-010` and the rest — stays blocked on:

> Owner-supplied signing, notarization and update-feed infrastructure; no
> signing credential, certificate, key, notarization tool, provider or feed is
> available in the retained facts.

This packet moved none of those rows and produced no signature, no notarisation
and no publisher identity. Nothing described here could be distributed.

## Prerequisites that remain outstanding

- **Clean machine.** Any package produced by this sequence is built and
  extracted on a developer host and is never installed on a clean virtual
  machine. *"A clean virtual machine for each supported OS with no prior Legion
  installation."* remains outstanding, and an `msiexec /a` administrative
  extraction is not an installation.
- **macOS and Linux.** Both remain blocked on `BLK-2026-09-08-02`. A Windows
  package is not a substitute for either row, and this packet produced no macOS
  or Linux artifact of any kind.
- **`BLK-2026-09-08-04`.** Still standing. Its prerequisite is a packaged native
  product present in the harness package directory; that directory does not
  exist in this worktree and `target/native-input-acceptance/package/legion-desktop.exe`
  is still absent. The staging script exists to satisfy that prerequisite once
  a verified MSI exists, and satisfying it would not by itself produce any
  acceptance evidence.

## What did not happen

- `xtask native-product-acceptance` was not run, in this packet or by this
  packet's instruction. No product window was opened, no OS-level input was
  injected, and no input class was observed.
- No cargo, rustc or xtask command was run by this packet.
- No file under `plans/completion/` was read for modification or changed.
  `COMP-PLAT-002` stays `implementation: partial` / `acceptance: unassessed`,
  and no `acceptance` or `implementation` value anywhere moved. The
  classification discipline this directory follows is the one stated in
  [the full-product-resume evidence README](../full-product-resume-2026-09-08/README.md).
- No `.rs` file was changed. The harness, the driver crate and
  [ADR-0056](../../adrs/ADR-0056-native-input-acceptance-harness.md) belong to a
  different packet this round.
- Nothing under `target/` was created or committed by this packet.
