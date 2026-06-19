# Legion Operator Runbook

This runbook is the operational companion to `plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md`.

## Local verification gates

Run from repo root:

```sh
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
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

No GitHub Actions CI workflow is currently configured, so local developer machines must install the CLI before running the full verification suite.

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

## Git remote auth paths

Legion's git remote actions shell out to the user's installed `git` binary without scrubbing the process environment. That means SSH-based remotes continue to use the caller's `SSH_AUTH_SOCK`/agent setup, and HTTPS remotes continue to use whatever credential helper is already configured for the host (for example the macOS keychain helper, Git Credential Manager, or a custom helper on `$PATH`).
