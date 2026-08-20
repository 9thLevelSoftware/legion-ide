# Legion Operator Runbook

This runbook is the operational companion to `plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md`.

## Local verification gates

Run from repo root:

```sh
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

If any command fails, save exact output under `plans/evidence/legion-e2e/` before fixing. Documentation hygiene allowlists live in `docs/hygiene-allowlist.toml`; keep entries narrow and historical-only.

## Golden-path smoke promotion criteria (Tier 0)

`.github/workflows/legion-smoke.yml` runs GP-1/2/3/4 + update-drill on a weekly schedule and `workflow_dispatch`. It is intentionally **independent**: failures do **not** block PR merges via `legion-gates.yml`.

**Do not** fold smoke into the standing PR gate or add it as a required status check until all of the following hold:

1. **Stability:** at least **four consecutive** scheduled (or fully equivalent dispatch) green runs on the **3-OS matrix** (ubuntu, windows, macos) without flaky skip storms.
2. **rust-analyzer provisioning:** provision steps succeed on each OS for those runs (or documented, accepted OS-specific skip with owner sign-off).
3. **Cost accepted:** maintainers accept the multi-OS cargo + real-server cost on the PR critical path.
4. **Owner sign-off:** a short note under `plans/evidence/production/` records the four green run URLs/SHAs and the decision to promote.

Until then, local `cargo run -p xtask -- golden-path-{1,2,3,4}` and the weekly smoke remain the primary GP evidence sources. See `plans/evidence/production/WS-P0/T0-D-smoke-promotion-criteria.md`.

## Deferred surfaces and what unfreezing costs

Three readiness gates are frozen: **PR-VSC-002** (isolated extension host),
**PR-ENT-001** (remote development UX), and **PR-ENT-002** (collaboration and
admin controls). ADR-0046 keeps them deferred until PR-UI-001 reaches "product
workflow validated".

The rule (roadmap P9.F3.T4) is that each surface needs **its own ADR, policy,
tests, and product evidence** before its readiness status changes — and that the
freeze must be lifted first.

```text
cargo run -p xtask -- deferred-surfaces
```

The gate exists because the rule was otherwise enforceable only by whoever
reviewed the diff. The readiness ledger is a markdown table: a surface could be
promoted from "Deferred" to "Product workflow validated" by editing one cell,
and the four artifacts would still be missing while the row read as though they
were not.

Two properties are worth knowing:

- **The freeze is checked first, and it is what bites today.** All three
  surfaces already have their own ADRs — remote has four — so an artifacts-only
  check would wave through a promotion ADR-0046 forbids outright. The artifacts
  are necessary; the freeze being lifted is what makes them sufficient.
- **Deleting a row is not a way out.** A configured surface with no ledger row
  fails the gate, because removing the row is a louder version of the edit the
  gate exists to prevent.

To promote one of these surfaces: amend ADR-0046 (or promote PR-UI-001), produce
the four artifacts, then change the status. In that order.

## GUI packaging and support artifacts

The current package-and-support path is intentionally explicit so release notes and issue triage can point at concrete files instead of assumptions.

### Packaging commands

- Dry-run Windows package: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -DryRun`
- Live Windows package: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -Release`
- GUI smoke dry-run: `sh scripts/gui-smoke.sh --dry-run` or `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -DryRun`
- GUI beta dry-run: `sh scripts/gui-smoke.sh --beta --dry-run` or `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Beta -DryRun`
- GUI Phase 8 dry-run: `sh scripts/gui-smoke.sh --phase-8 --dry-run` or `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Phase8 -DryRun`

A live `--smoke` run exits non-zero when the report it writes says `failed` or
`blocked`, and zero only on `passed`. The evidence markdown is written first in
every case, so a non-zero exit still leaves a readable report at the `--evidence`
path. Until 2026-08-20 the run exited 0 regardless of status, which meant
`scripts/gui-smoke.sh` (run under `set -e`) and any operator reading the exit
code were told a failed smoke had passed.

### Native installer release (manual)

The installed `legion-release.yml` is a manual-only native-release workflow with a required `mode` input. An authorized maintainer starts it from the intended source branch with:

```sh
# Build and fully validate all five installers without creating a tag or release:
gh workflow run legion-release.yml -f mode=verify-only

# Validate and, only if every package verifier passes, tag and publish the next v0.0.N prerelease:
gh workflow run legion-release.yml -f mode=publish

gh run list --workflow legion-release.yml
```

A `verify-only` run exercises the complete package-and-verify pipeline but can never create a tag, GitHub Release, or release asset. In `publish` mode the workflow selects the next unused `v0.0.N` tag, beginning with `v0.0.1`; it creates the GitHub tag and prerelease and publishes the native assets. It passes the corresponding numeric version (for example, `0.0.1`) to the package scripts; the script examples use `0.0.1` only as a placeholder and the workflow substitutes its computed version. This beta release number is independent of the workspace version in `Cargo.toml`.

Package verification is performed by two version-controlled entry points rather than inline workflow YAML: `scripts/verify-native-package.sh` (DEB, AppImage, DMG) and `scripts/verify-native-package.ps1` (MSI). Each verifier checks artifact existence, SHA-256, release metadata (including `signer_status`), install/extract structure, installer version (the DEB verifier additionally requires a non-empty Debian `Maintainer:` field), and runs the extracted/installed binary headlessly with `--beta-smoke`; a non-zero smoke exit is a hard failure. The beta smoke workspace is always `target/release-smoke/<platform>-<arch>-<format>/workspace` under the checked-out workspace, because the application rejects beta workspaces outside `<workspace>/target`. Every verifier prints its complete `PACKAGE-EVIDENCE.txt` report to the job log on success and on failure, and the package jobs upload the evidence artifacts even when verification fails, so no failure detail is visible only inside a runner temporary directory. Contract tests for the verifiers run with `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-native-package-verifiers.ps1` (pwsh works too).

Each verifier also writes a machine-readable `VALIDATION-SUMMARY.toml` beside its installer. The `publish` job parses all five summaries with Python's `tomllib` before any tag or release mutation and refuses to publish unless every summary reports the expected candidate tag, source SHA, format, and architecture with `result = "passed"` and `smoke_exit = 0`; a missing or malformed summary is likewise a publication failure.

Each GitHub prerelease contains these unsigned-beta installer assets and their SHA-256 checksum files:

| Platform | Installer asset | Format |
| --- | --- | --- |
| macOS Intel | `legion-desktop-macos-x64-dmg.dmg` | DMG |
| macOS Apple Silicon | `legion-desktop-macos-arm64-dmg.dmg` | DMG |
| Windows x64 | `legion-desktop-windows-x64-msi.msi` | MSI |
| Linux x64 | `legion-desktop-linux-x64-deb.deb` | Debian package |
| Linux x64 | `legion-desktop-linux-x64-appimage.AppImage` | AppImage |

For each package, the release includes the installer, its matching `.sha256` checksum, `<package-stem>-RELEASE-METADATA.toml`, `<package-stem>-PACKAGE-EVIDENCE.txt`, and `<package-stem>-VALIDATION-SUMMARY.toml`; it does not promise a native package manifest. The evidence file records the package-structure checks, checksum verification, and beta-smoke logs produced by the package job; the validation summary is the machine-readable record the publish gate enforced. Treat all five installers as `unsigned-beta/no-os-code-signing` (no code signature, no notarization): Windows SmartScreen may warn before opening the MSI, and macOS Gatekeeper may require the tester to use Finder's explicit **Open** action for the DMG or app. Only bypass either warning after independently verifying the release source and checksum.

For DMG inspection, run `hdiutil verify <artifact>`, attach it read-only, then detach the mounted volume with `hdiutil detach <mount-point>` when inspection is complete. The hosted DMG verification follows the same tested procedure: verify, attach read-only, copy the single `.app` bundle out of the volume, detach, confirm `CFBundleShortVersionString`, then run the **copied** `Contents/MacOS/legion-desktop` with `--beta-smoke` against the `target/release-smoke/macos-<arch>-dmg/workspace` beta workspace — never from the mounted volume. The verifier guarantees the volume is detached and the full evidence report is printed even when a check fails, so a smoke failure is visible in the job log rather than hidden behind a final `hdiutil detach` line.

For local Linux testing, download the matching asset and run one of these commands:

```sh
# Debian/Ubuntu
sudo apt install ./legion-desktop-linux-x64-deb.deb
legion-desktop --beta-smoke --duration-ms 1500

# AppImage
chmod +x ./legion-desktop-linux-x64-appimage.AppImage
./legion-desktop-linux-x64-appimage.AppImage --appimage-extract-and-run --beta-smoke --duration-ms 1500
```

`cargo run -p xtask -- verify-release-pipeline` validates the descriptor metadata only. In dry-run mode it may report `dry-run/unchecked`; it does not build installers or execute their OS-specific verification commands. The workflow does not add signing credentials, certificates, notarization material, or other private keys, and it does not rewrite `Cargo.toml`; the packaging scripts receive the computed numeric release version explicitly.

### Release signer references

The release pipeline config and operator runbook must describe signer references without committing any private material. Pick exactly one source for a given release run and store only the reference string in the repo or CI configuration.

| Source | Where the material lives | What the repo records |
| --- | --- | --- |
| `env` | exported process environment | the environment variable name or alias only |
| `keyring` | OS keychain / keyring | the service/account label only |
| `kms` | deployment-owned KMS adapter | the key URI/ARN and adapter reference only |
| `ci-secret` | CI secret manager | the secret name or variable name only |

Recommended local example:

```toml
[signing]
source = "keyring"
reference = "legion-release/signing-profile"
identity = "release-signing-profile"
```

Operational notes:

- Use `env` for ephemeral local or launchd-driven runs when a shell export is the least surprising source of truth.
- Use `keyring` when the signer material should stay bound to the host user session or machine keychain.
- Use `kms` when a deployment-owned service or build adapter resolves the signer material outside the repository.
- Use `ci-secret` when CI injects a signer reference and the actual credential remains in the CI secret store.
- Never commit the private key, certificate, token value, or notarization credential itself; only commit the reference needed to look it up.

### Ed25519 signing key format (PKT-SIGN / ADR-0042)

Legion uses detached Ed25519 signatures (ADR-0042) for the auto-updater manifest. The signing key is a **base64-encoded 32-byte seed** (standard unpadded or padded base64 — the resolver accepts either). The verifying key is derived automatically from the seed and is embedded in the manifest via `signer_reference`.

Key generation (operator workstation only; never commit the output):

```sh
# Generate a 32-byte random seed and base64-encode it
openssl rand -base64 32
```

Store the resulting string as the env var or keyring secret named in `[signing].reference`. The key material must be zeroized from memory after use (the `xtask` signing module handles this automatically via the `zeroize` crate).

### Release manifest commands (PKT-SIGN)

Generate a signed or unsigned-beta release manifest after artifacts are built:

```sh
cargo run -p xtask -- release-manifest \
  --config xtask/release-pipeline.example.toml \
  --channel stable \
  --artifacts <path-to-built-artifacts> \
  --out target/release-pipeline
```

The command writes `release-manifest.v1.toml` and, when a signer is resolved, `release-manifest.v1.toml.sig` alongside it. The manifest `signer_status` field records either `signed/ed25519` or `unsigned-beta/no-signer-configured`.

### Unsigned-beta policy (WS17-T2 / P8.F1.T4 / WS-A-D D2)

If Legion ships before production signing credentials are provisioned, every release descriptor and the auto-updater manifest must carry `signer_status = "unsigned-beta/no-signer-configured"`. This is a first-class outcome — not an error — governed by the policy in `plans/product-readiness-ledger.md` (WS17-T2 entry) and explicitly **retained** for OS installers under `plans/evidence/production/WS-A-D/phase-4-release/D2-unsigned-beta-retained.md`. Portable preview bundles from `scripts/package-preview.*` / `legion-preview.yml` use `signer_status = unsigned-beta/no-os-code-signing`. The unsigned-beta status must be:

1. Visible in the release descriptor TOML written by `xtask release-pipeline --from-artifacts`.
2. Visible in the auto-updater manifest written by `xtask release-manifest`.
3. Documented in the readiness ledger before shipping.
4. Never replaced by silent “signed” claims until D2.1 OS signing secrets exist outside the repo.

An unsigned-beta release must never be silently treated as signed. The pipeline hard-rejects any attempt to run without `--dry-run` or `--from-artifacts`.

### Expected artifacts

- Windows package directory: `target/gui-phase6-package/`
- Packaged executable: `target/gui-phase6-package/legion-desktop.exe`
- Package manifest: `target/gui-phase6-package/legion-desktop-package-manifest.txt`
- GUI smoke session state and diagnostics exports: `target/gui-phase6-session.json`, `target/gui-phase6-diagnostics.md`, `target/gui-phase7-session.json`, `target/gui-phase7-diagnostics.md`, `target/gui-phase8-session.json`, `target/gui-phase8-diagnostics.md`

A release runbook is only considered closed once the packaging command, the expected artifacts, and the matching evidence files all exist for the release candidate under review.

### Supply-chain gate prerequisite

The `cargo deny check` gate above requires `cargo-deny` on the local machine. Install it with:

```sh
cargo install cargo-deny --locked
cargo deny --version
```

GitHub Actions runs `.github/workflows/legion-gates.yml` (standing gate set on ubuntu/windows/macos for every push to main and every PR; perf-harness in report-only mode, pytest excluded), `.github/workflows/legion-bench.yml` (recorded-mode legion-bench on every push to main and every PR: real fixture execution with model responses replayed from committed cassettes, gated against a committed per-task baseline; live provider runs are confined to the opt-in, scheduled, `continue-on-error` `.github/workflows/legion-bench-live.yml` and can never gate a merge), and `.github/workflows/legion-smoke.yml` (GP-1 through GP-4 golden-path smokes and the update-drill, completing the 21 standing gates on dispatch and weekly, 3-OS matrix, independent — not a PR merge blocker). The update-drill exercises deterministic update/rollback with an ephemeral Ed25519 keypair; it is zero-egress. Local developer machines must still install the CLI before running the full verification suite, which remains the primary verification source until the hosted gate history is proven stable.

## Evidence naming

Use this pattern:

- `phase-0-check-deps.txt`
- `phase-0-fmt.txt`
- `phase-1-legion-ui-tests.txt`
- `phase-4-assist-inline-prediction.txt`
- `phase-8-model-download-dry-run.txt`
- `final-workspace-test.txt`
- `final-clippy.txt`

Each evidence file should contain:

1. command;
2. working directory;
3. start/end time;
4. exit code;
5. raw output.

## Subagent execution pattern

For every implementation task:

1. dispatch one implementer subagent with exact files and commands;
2. require a failing test first when the task changes code;
3. run the task-specific gate;
4. dispatch spec-compliance reviewer;
5. dispatch quality/security reviewer;
6. fix reviewer findings before proceeding;
7. commit the task.

Do not ask the implementer subagent to read the entire planning package. Give it the one task section plus the exact source files it needs.

## Safety checks

Before any task touching AI, worker, cloud, or trace code, verify:

- Manual mode exclusion remains tested;
- proposal-only mutation remains tested;
- metadata-only default retention remains tested;
- consent-gated raw trace path remains tested;
- network routes are denied in offline/air-gap policy unless explicitly loopback and allowed.

## Phase 8 trace and model dry-runs

Run from repo root before claiming model-flywheel readiness:

```sh
bash scripts/models/download-models.sh --dry-run
bash scripts/models/start-local-workers.sh --dry-run --config config/workers.example.yaml
python3 evals/run_eval.py --dry-run
python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/legion-eval.json
python3 training/qlora_train.py --dry-run
python3 training/qlora_train.py --fixture-smoke --dataset training/fixtures/minimal_traces.jsonl --output-dir /tmp/legion-train-smoke
python3 training/convert_to_gguf.py --dry-run
python3 training/convert_to_gguf.py --fixture-smoke --model-dir /tmp/legion-train-smoke --output /tmp/legion-model.gguf --metadata-output /tmp/legion-gguf.json
python3 -m compileall training evals scripts/models
cargo test -p legion-memory --all-targets trace
cargo test -p legion-security --all-targets redaction
```

Real model download, serving, training, conversion, hosted export, or dataset construction requires explicit consented trace export plus redaction/secret-scan evidence.

## PR creation

After all phases and gates pass:

```sh
git status --short
git diff --stat origin/main...HEAD
git push -u origin HEAD
gh pr create --title "feat: implement Legion e2e product plan" --body-file /tmp/legion-pr-body.md
```

The PR body must include:

- summary by phase;
- tests/evidence paths;
- security/authority boundary notes;
- cloud/training operational notes;
- no unsupported planned features in scope.

## GP-1 smoke

The GP-1 golden-path smoke exercises the full AppComposition product surface against a throwaway fixture workspace.

Command:

```sh
cargo run -p xtask -- golden-path-1
```

Evidence report: `target/golden-path/gp1_report.toml` (written after every run, overwritten on re-run).

To record a copy under the evidence tree (operator-only; CI uploads the `target/` artifact instead):

```sh
cargo run -p xtask -- golden-path-1 --record-evidence plans/evidence/production/M8/
```

### Step overview and skip semantics

| Step | What it verifies | Skip condition |
|------|-----------------|----------------|
| s1 | Fixture copy to temp dir + workspace open as Trusted | None (always runs) |
| s2 | rust-analyzer session init (real server, product path) | **Skipped** (not failed) if `rust-analyzer` absent from PATH |
| s3 | Diagnostic cycle: introduce error → detect → fix → clear | Skipped when s2 is skipped |
| s4 | Workspace search for known literal + case-sensitive variant | None |
| s5 | Terminal: `cargo test` via product gate, poll for exit-0 | Skipped gracefully if PTY unavailable (reason logged) |
| s6 | Git: edit via app save path → dirty-file check → stage + commit | None |
| s7 | Evidence TOML written to `target/golden-path/gp1_report.toml` | None |

A step-level `skipped` status is not a failure. The overall run exits 0 when all non-skipped steps pass. The CI workflow (`.github/workflows/legion-smoke.yml`) is independent and a red run there does not block PR merges.

The smoke never writes inside the repo checkout (except `target/` and the optional `--record-evidence` path). Fixture copies live in the OS temp directory; they are cleaned on success and left for inspection on failure (path printed to stderr).

## Git remote auth paths

Legion's git remote actions shell out to the user's installed `git` binary without scrubbing the process environment. That means SSH-based remotes continue to use the caller's `SSH_AUTH_SOCK`/agent setup, and HTTPS remotes continue to use whatever credential helper is already configured for the host (for example the macOS keychain helper, Git Credential Manager, or a custom helper on `$PATH`).
