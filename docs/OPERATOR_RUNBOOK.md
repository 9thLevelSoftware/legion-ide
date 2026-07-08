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

## GUI packaging and support artifacts

The current package-and-support path is intentionally explicit so release notes and issue triage can point at concrete files instead of assumptions.

### Packaging commands

- Dry-run Windows package: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -DryRun`
- Live Windows package: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -Release`
- GUI smoke dry-run: `sh scripts/gui-smoke.sh --dry-run` or `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -DryRun`
- GUI beta dry-run: `sh scripts/gui-smoke.sh --beta --dry-run` or `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Beta -DryRun`
- GUI Phase 8 dry-run: `sh scripts/gui-smoke.sh --phase-8 --dry-run` or `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1 -Phase8 -DryRun`

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

### Unsigned-beta policy (WS17-T2 / P8.F1.T4)

If Legion ships before production signing credentials are provisioned, every release descriptor and the auto-updater manifest must carry `signer_status = "unsigned-beta/no-signer-configured"`. This is a first-class outcome — not an error — governed by the policy in `plans/product-readiness-ledger.md` (WS17-T2 entry). The unsigned-beta status must be:

1. Visible in the release descriptor TOML written by `xtask release-pipeline --from-artifacts`.
2. Visible in the auto-updater manifest written by `xtask release-manifest`.
3. Documented in the readiness ledger before shipping.

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

GitHub Actions runs `.github/workflows/legion-gates.yml` (standing gate set on ubuntu/windows/macos for every push to main and every PR; perf-harness in report-only mode, pytest excluded), `.github/workflows/legion-bench.yml` (weekly recorded-mode legion-bench fixture scoring; live provider calls are a future M13 scope), and `.github/workflows/legion-smoke.yml` (GP-1 through GP-4 golden-path smokes and the update-drill, completing the 20 standing gates on dispatch and weekly, 3-OS matrix, independent — not a PR merge blocker). The update-drill exercises deterministic update/rollback with an ephemeral Ed25519 keypair; it is zero-egress. Local developer machines must still install the CLI before running the full verification suite, which remains the primary verification source until the hosted gate history is proven stable.

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
