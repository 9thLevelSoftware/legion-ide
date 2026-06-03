# Resolve Legion E2E Audit Findings Implementation Plan

> **For Hermes:** Use the `gpt55-kimi-engineering-workflow` and `subagent-driven-development` skills to implement this plan task-by-task. GPT-5.5 owns architecture, sequencing, reviews, git, CI, and final acceptance. Kimi 2.6 subagents receive only one bounded task packet at a time with exact files, tests, acceptance criteria, and stop conditions.

**Goal:** Resolve every issue, stub, deferral, and audit finding from `ENGINEERING_AUDIT.yaml` by completing the code paths and evidence coverage, not by deleting planned features or weakening requirements.

**Architecture:** Keep Legion local-first, proposal-gated, projection-only, and default-deny. Complete missing/deferred surfaces in layered order: test/evidence traceability, product-readiness reconciliation, unified beta e2e coverage, real cloud lane transport, real training/eval harnesses, CI coverage, and stale repo guidance. Do not bypass proposal-mediated mutation, do not make cloud/training required for Manual/local operation, and do not rename internal `devil-*` crates in this plan.

**Tech Stack:** Rust 2024 workspace, cargo tests, GitHub Actions, Python 3, shell/PowerShell dry-run scripts, `devil-cli` evidence gates, optional Python training dependencies, optional local HTTP/mock server for cloud contract tests.

---

## Current Context / Assumptions

Current audit artifacts:

- `ENGINEERING_AUDIT.yaml`
- `ENGINEERING_AUDIT.html`
- `ENGINEERING_STATUS.md`
- `.hermes/audit-packets/*.md`
- `audit-reports/manual-ui-e2e-audit-2026-06-02.md`

Audit summary:

- Features inspected: 10
- Implemented: 7
- Partially implemented / deferred: 3
- Findings: 6
- Verified local gates: workspace tests, fmt, check, clippy, xtask dependency policy, evidence checks, model/training/eval dry-runs
- Local non-parity: `cargo-deny` was not installed locally, though CI runs `EmbarkStudios/cargo-deny-action@v2`

Findings to resolve:

1. `finding-product-readiness-ledger-stale` — `plans/product-readiness-ledger.md` marks product gates stale / Not started despite validated surfaces.
2. `finding-unified-beta-e2e-test-missing` — beta acceptance scenario lacks one unified e2e scenario test or accepted matrix.
3. `finding-cloud-lane-production-transport-missing` — cloud lane has policy/client contracts and fixtures but no production HTTP/gRPC transport path.
4. `finding-training-eval-real-execution-deferred` — training/eval scripts are dry-run scaffolds rather than real optional training/inference harnesses.
5. `finding-agents-placeholder-note-stale` — `AGENTS.md` still labels active agent/tracker/memory crates as placeholders.
6. `finding-training-dry-runs-not-in-ci` — model/training/eval dry-runs are not directly run in CI.

Non-negotiables:

- Resolution means completing code, tests, docs, and evidence. Do not “fix” findings by removing requirements, deleting features, or lowering acceptance criteria.
- Manual mode must remain AI/network/cloud/worker/telemetry excluded.
- AI, worker, cloud, and training lanes must not mutate the main workspace directly.
- Cloud and raw trace/model-output retention must remain opt-in, consent-gated, redacted, and default-deny.
- User-facing naming should use Legion; internal crate names remain `devil-*` unless a separate rename plan exists.

---

## Proposed Approach

Implement in five PR-sized phases:

1. **Audit traceability and stale docs cleanup:** Reconcile product-readiness status and repo guidance so the codebase accurately reflects implemented vs deferred surfaces.
2. **Unified beta acceptance e2e:** Add a single integrated beta acceptance test or explicit scenario harness that proves the current daily user loop end to end.
3. **Production cloud lane transport:** Add a real HTTP JSON transport and local mock-server contract tests while retaining default-deny policy and fixture coverage.
4. **Real optional training/eval harness:** Add optional Python dependency metadata and real-mode hooks for training/eval/convert without making heavyweight dependencies mandatory in CI.
5. **CI and evidence hardening:** Add CI dry-run coverage, evidence artifacts, and final full gates.

Do not parallelize tasks that touch overlapping files. Safe parallel lanes are:

- Ledger/docs reconciliation and AGENTS update can run together only after both are bounded to non-overlapping docs.
- Cloud transport and training/eval harness can run in parallel if they do not touch `.github/workflows/ci.yml`, `ENGINEERING_AUDIT.yaml`, or shared evidence docs.
- CI hardening waits until cloud/training command surfaces are stable.

---

## Phase 0 — Pre-flight and Artifact Hygiene

### Task 0.1: Confirm baseline branch and preserve audit artifacts

**Objective:** Ensure implementation starts from `main` with only known audit artifacts present.

**Files:**
- Read: `ENGINEERING_AUDIT.yaml`
- Read: `ENGINEERING_STATUS.md`
- Read: `git status --short` output
- No code changes

**Steps:**
1. Run:
   - `git status --short --branch`
   - `git log -1 --oneline`
2. Confirm branch is `main` and HEAD is at or after `5341837`.
3. Decide whether audit artifacts should be committed in the implementation branch. Recommended: keep `ENGINEERING_AUDIT.yaml`, `ENGINEERING_AUDIT.html`, `ENGINEERING_STATUS.md`, `.hermes/audit-packets/*.md`, and `audit-reports/*.md` as committed evidence unless the repo policy excludes `.hermes/`.
4. If `.hermes/` should not be committed, move implementation task packets into `plans/evidence/legion-e2e/` or another accepted evidence path before implementation.

**Validation:**
- `git status --short --branch` shows expected branch and only intentional untracked audit artifacts.

**Stop condition:**
- Stop if repo is not on `main` or there are unrelated modifications.

---

## Phase 1 — Product Readiness Ledger and Guidance Reconciliation

### Task 1.1: Add product-readiness traceability model

**Objective:** Make `plans/product-readiness-ledger.md` represent actual code/test/evidence status without overstating product readiness.

**Files:**
- Modify: `plans/product-readiness-ledger.md`
- Read: `ENGINEERING_AUDIT.yaml`
- Read: `audit-reports/manual-ui-e2e-audit-2026-06-02.md`
- Read: `plans/evidence/legion-e2e/README.md`
- Possibly modify: `ENGINEERING_AUDIT.yaml`

**Implementation details:**
1. Add status vocabulary near `## Gate Rules`:
   - `Not started`
   - `In progress`
   - `Substrate validated`
   - `Product workflow validated`
   - `Deferred with explicit cut line`
   - `Blocked`
2. Add a rule that `Substrate validated` means code/tests pass but beta UX may still require product workflow evidence.
3. Add an `Evidence References` column if the table remains readable, or add per-gate detail sections below the table.
4. Update each current gate conservatively:
   - `PR-UI-001`: likely `In progress` or `Substrate validated`, because UI/projection tests pass but renderer latency/accessibility budgets may need more evidence.
   - `PR-UI-002`: likely `In progress`, because large workspace/100MB degraded-mode remains a known gap in `AGENTS.md`.
   - `PR-LANG-001`: likely `In progress`, unless Rust LSP workflow evidence proves full completion.
   - `PR-LANG-002`: likely `Substrate validated` or `In progress` depending on debug/test/SCM GUI evidence.
   - `PR-AI-001`: likely `Substrate validated` for provider policy/context surfaces, with real provider tests still optional.
   - `PR-AI-002`: likely `Substrate validated` for proposal safety/evals dry-run, with real adversarial evals deferred.
   - `PR-VSC-001`: remains `In progress` unless manifest/contribution tests prove all criteria.
   - `PR-VSC-002`: remains `Deferred with explicit cut line` or `Not started` because isolated extension host sidecar is explicitly deferred.
   - `PR-ENT-001`: likely `In progress` / `Substrate validated` if remote transport/session tests pass but product UX remains incomplete.
   - `PR-ENT-002`: likely `In progress` or `Deferred with explicit cut line`, depending on collaboration/admin evidence.
   - `PR-REL-001`: likely `In progress`, because dry-run packaging and evidence gates pass but signed installers/auto-update/rollback may not be complete.
5. Add explicit evidence references to exact tests/evidence files instead of generic claims.

**Tests / validation:**
- `cargo run -p devil-cli -- evidence check --phase gui-phase8`
- `cargo test --workspace --all-targets`
- Manual review: every upgraded status has at least one evidence reference.

**Acceptance criteria:**
- No product gate remains stale simply because the ledger was not updated.
- No gate is marked complete without the exact working UX path and evidence required by ledger rules.
- `finding-product-readiness-ledger-stale` can move to `fixed` in `ENGINEERING_AUDIT.yaml`.

### Task 1.2: Reconcile `AGENTS.md` placeholder-crate guidance

**Objective:** Replace stale placeholder guidance with accurate active/deferred crate boundaries.

**Files:**
- Modify: `AGENTS.md`
- Read: `crates/devil-agent/src/lib.rs`
- Read: `crates/devil-tracker/src/lib.rs`
- Read: `crates/devil-memory/src/lib.rs`
- Read: `crates/devil-index/src/lib.rs`

**Implementation details:**
1. Replace the broad line:
   - `Placeholder crates (devil-index, devil-agent, devil-tracker, devil-memory, parts of AI/provider surface) must remain inert...`
2. New guidance should distinguish:
   - Active and phase-gated: `devil-agent`, `devil-tracker`, `devil-memory` if their tests and phase gates exist.
   - Still inert/deferred: any truly placeholder crates/surfaces, likely `devil-index` or specific AI/provider subfeatures if audit confirms.
3. Preserve warning that future surfaces need ADR/phase gate, dependency policy, and contract tests.
4. Do not declare broad product readiness from crate activation alone.

**Tests / validation:**
- `cargo test -p devil-agent --all-targets`
- `cargo test -p devil-tracker --all-targets`
- `cargo test -p devil-memory --all-targets`
- `cargo run -p xtask -- check-deps`

**Acceptance criteria:**
- Guidance no longer mislabels active crates as inert placeholders.
- Still-deferred surfaces remain protected.
- `finding-agents-placeholder-note-stale` can move to `fixed`.

### Task 1.3: Update audit artifacts after docs reconciliation

**Objective:** Keep canonical audit artifacts synchronized with resolved docs findings.

**Files:**
- Modify: `ENGINEERING_AUDIT.yaml`
- Regenerate: `ENGINEERING_AUDIT.html`
- Modify: `ENGINEERING_STATUS.md`

**Steps:**
1. Set `finding-product-readiness-ledger-stale.status` to `fixed` only after Task 1.1 passes.
2. Set `finding-agents-placeholder-note-stale.status` to `fixed` only after Task 1.2 passes.
3. Add validation entries for the commands run above.
4. Regenerate HTML:
   - `python3 <skill>/scripts/validate_engineering_audit.py ENGINEERING_AUDIT.yaml`
   - `python3 <skill>/scripts/generate_engineering_audit_html.py ENGINEERING_AUDIT.yaml ENGINEERING_AUDIT.html`
5. Update `ENGINEERING_STATUS.md` counts.

**Validation:**
- `ENGINEERING_AUDIT.yaml` validates.
- `ENGINEERING_AUDIT.html` regenerates.

---

## Phase 2 — Unified Beta Acceptance E2E Scenario

### Task 2.1: Define the beta scenario contract in code

**Objective:** Turn the beta acceptance paragraph into a stable, executable scenario contract.

**Files:**
- Modify or create: `crates/devil-desktop/tests/beta_acceptance_e2e.rs`
- Modify if needed: `crates/devil-desktop/Cargo.toml`
- Read: `crates/devil-desktop/tests/beta_workflow.rs`
- Read: `crates/devil-app/tests/legion_workflow_integration.rs`
- Read: `crates/devil-app/tests/assist_inline_prediction_workflow.rs`
- Read: `crates/devil-app/tests/git_workflow.rs`
- Read: `crates/devil-app/tests/debug_workflow.rs`
- Read: `crates/devil-app/tests/workspace_vfs_integration.rs`
- Read: `crates/devil-vscode-compat/tests` if present

**Scenario requirements from `plans/product-readiness-ledger.md`:**
A user can:
1. Open a large repository.
2. Install an approved VSIX.
3. Run Rust LSP completion.
4. Ask AI for a multi-file change.
5. Inspect the context manifest.
6. Review the proposal diff.
7. Run tests.
8. Debug a failure.
9. Collaborate on review.
10. Save safely.
11. Export audit evidence.
12. Do all of the above without bypassing policy or proposal gates.

**Implementation details:**
1. Start with a deterministic mock workspace fixture; do not require real VS Code marketplace, live LSP, real AI provider, or cloud.
2. Use existing DTOs/projections/mocks rather than shelling out to external tools unless the repo already has supported fixtures.
3. Create one `#[test]` with clear stage comments, or multiple ordered helper assertions inside one test file.
4. If a requirement is genuinely not implemented, write an explicit failing test first and then implement the missing product path. Do not mark it skipped unless the product cut line explicitly says deferred.
5. Avoid broad sleep/time-based tests; use deterministic state transitions.

**Expected test structure:**

```rust
#[test]
fn beta_acceptance_e2e_policy_gated_local_loop() {
    // 1. open fixture workspace / large repo marker
    // 2. ingest approved VSIX manifest metadata
    // 3. project Rust language action/completion fixture
    // 4. request AI multi-file proposal via local/mock provider
    // 5. inspect context manifest and privacy labels
    // 6. review proposal diff/hunks
    // 7. run validation/test command fixture
    // 8. project debug failure and resolution path
    // 9. project collaboration review metadata
    // 10. save through proposal-mediated flow
    // 11. export audit evidence bundle
    // 12. assert no direct mutation, no forbidden egress, no policy bypass
}
```

**Validation:**
- First run targeted test and confirm it fails for the missing integration point.
- Implement missing integration.
- Then run:
  - `cargo test -p devil-desktop --test beta_acceptance_e2e -- --nocapture`
  - `cargo test -p devil-desktop --test beta_workflow -- --nocapture`
  - `cargo test -p devil-app --test legion_workflow_integration --all-targets`
  - `cargo test -p devil-vscode-compat --all-targets`

**Acceptance criteria:**
- The full beta acceptance scenario is represented by one explicit e2e test file.
- The test validates policy/proposal gates, not just UI labels.
- `finding-unified-beta-e2e-test-missing` can move from `observed` to `fixed`.

### Task 2.2: Add a beta scenario evidence artifact

**Objective:** Store the exact beta e2e command output under accepted evidence paths.

**Files:**
- Create: `plans/evidence/legion-e2e/<timestamp>_beta_acceptance_e2e.txt`
- Modify: `plans/evidence/legion-e2e/README.md`
- Modify: `plans/product-readiness-ledger.md`
- Modify: `ENGINEERING_AUDIT.yaml`
- Regenerate: `ENGINEERING_AUDIT.html`

**Steps:**
1. Run `cargo test -p devil-desktop --test beta_acceptance_e2e -- --nocapture` and tee output into an evidence file.
2. Add a short README entry explaining what the scenario covers.
3. Link the evidence from product-readiness `Beta Acceptance Scenario` section.
4. Update audit validation matrix.

**Validation:**
- Evidence file exists and contains the passing command output.
- `cargo run -p devil-cli -- evidence check --phase gui-phase8` still passes, or update the evidence checker if it should validate this new artifact.

---

## Phase 3 — Production Cloud Lane Transport

### Task 3.1: Audit current cloud transport seam and define the transport API contract

**Objective:** Identify the smallest production transport implementation that satisfies Phase 7 without weakening policy.

**Files:**
- Read: `crates/devil-remote/src/lib.rs`
- Read: `crates/devil-remote-transport/src/lib.rs`
- Read: `crates/devil-security/src/lib.rs`
- Read: `crates/devil-protocol/src/lib.rs`
- Modify: `plans/evidence/legion-e2e/<timestamp>_cloud_transport_contract.md`

**Design decision:** Prefer an HTTP JSON transport first, because the existing workspace already has `reqwest` in `[workspace.dependencies]` and cloud plan endpoints are HTTP-like: submit task, status, stream events, cancel, fetch proposal, fetch evidence.

**Contract requirements:**
- Submit signed/scoped task packet.
- Enforce upload scope visibility before request leaves local process.
- Enforce max upload bytes and max cost cents.
- Reject secrets / forbidden files by default.
- Preserve request/response correlation.
- Support status fetch and cancellation.
- Support proposal/evidence fetch through policy gates.
- Support deterministic mock server tests.
- Do not auto-apply fetched proposals.

**Validation:**
- Contract doc references exact DTOs and policy checks.

### Task 3.2: Add failing HTTP cloud transport tests

**Objective:** Establish expected production transport behavior before implementation.

**Files:**
- Modify or create: `crates/devil-remote/tests/cloud_lane_http_transport.rs`
- Possibly modify: `crates/devil-remote/Cargo.toml`
- Possibly modify: `Cargo.toml` workspace dependencies if test server dependency is needed

**Test cases:**
1. `http_transport_submits_task_packet_with_policy_headers_and_correlation`
2. `http_transport_rejects_when_policy_disables_submission_before_network`
3. `http_transport_rejects_forbidden_upload_scope_before_network`
4. `http_transport_fetches_status_and_matches_response_id`
5. `http_transport_cancels_task_with_correlation`
6. `http_transport_fetches_proposal_without_applying_workspace_mutation`
7. `http_transport_fetches_evidence_metadata_without_raw_secret_payload`
8. `http_transport_times_out_or_errors_as_classified_transport_error`

**Implementation notes:**
- Use a local in-process mock HTTP server if the repo already has one.
- If adding a dependency, choose a minimal dev-dependency and update `plans/dependency-policy.md` plus `xtask` if required.
- If avoiding new dependencies, use `std::net::TcpListener` in the test to serve deterministic HTTP responses.

**Validation:**
- Run the new test and confirm it fails because transport is not implemented yet:
  - `cargo test -p devil-remote --test cloud_lane_http_transport -- --nocapture`

### Task 3.3: Implement `HttpLegionCloudLaneTransport`

**Objective:** Complete a production-capable HTTP JSON carrier for the cloud lane.

**Files:**
- Modify: `crates/devil-remote/src/lib.rs`
- Modify: `crates/devil-remote/Cargo.toml` if feature/dev dependency changes are needed
- Modify: `Cargo.toml` only if adding workspace dependency config
- Modify: `plans/dependency-policy.md` and `xtask/src/main.rs` if new dependency policy requires it

**Implementation details:**
1. Add a transport config struct, e.g.:
   - endpoint/base URL
   - timeout
   - optional auth token reference or header provider (do not embed secrets in config)
   - client identity metadata
2. Add `HttpLegionCloudLaneTransport` implementing the existing `LegionCloudLaneTransport` trait.
3. Use `reqwest` blocking or async consistently with current crate style.
4. Validate policy before request construction.
5. Map HTTP errors to existing or new typed remote errors.
6. Do not log raw request bodies, secrets, or raw source payloads.
7. Do not apply proposals; return proposal DTOs only.
8. Add serialization/deserialization roundtrips if not already covered.

**Validation:**
- `cargo test -p devil-remote --test cloud_lane_http_transport -- --nocapture`
- `cargo test -p devil-remote --all-targets`
- `cargo test -p devil-security --all-targets cloud`
- `cargo run -p xtask -- check-deps`

### Task 3.4: Wire cloud transport into app/config without making cloud default-on

**Objective:** Make the production transport reachable from app/config while preserving fail-closed local defaults.

**Files:**
- Modify: `crates/devil-app/src/lib.rs`
- Modify: `crates/devil-remote/src/lib.rs`
- Modify: `config/workers.example.yaml` only if appropriate, but cloud config should likely get a separate example
- Create or modify: `config/cloud-lane.example.toml` or equivalent if the repo uses config files
- Modify: `docs/OPERATOR_RUNBOOK.md`

**Implementation details:**
1. Add config parsing or constructor path for a cloud endpoint.
2. Default remains no endpoint / disabled.
3. App-level API should refuse cloud submission until explicit policy permits it.
4. Add tests proving Manual/offline mode cannot submit cloud tasks even if endpoint config exists.

**Validation:**
- `cargo test -p devil-app --all-targets cloud`
- `cargo test -p devil-remote --all-targets`
- `cargo test -p devil-security --all-targets cloud`

### Task 3.5: Add cloud lane evidence and update audit

**Objective:** Close cloud production transport finding with evidence.

**Files:**
- Create: `plans/evidence/legion-e2e/<timestamp>_cloud_lane_http_transport_gates.txt`
- Modify: `ENGINEERING_AUDIT.yaml`
- Regenerate: `ENGINEERING_AUDIT.html`
- Modify: `ENGINEERING_STATUS.md`

**Validation:**
- Evidence file contains passing cloud transport commands.
- `finding-cloud-lane-production-transport-missing.status` becomes `fixed` only after production transport and tests pass.

---

## Phase 4 — Real Optional Training / Eval Harness

### Task 4.1: Add Python project/dependency metadata for optional training extras

**Objective:** Make real training/eval dependencies explicit and installable without requiring them for normal CI.

**Files:**
- Create: `pyproject.toml` or `training/pyproject.toml` depending on repo convention
- Possibly create: `requirements-training.txt` if preferred by operator docs
- Modify: `training/README.md`
- Modify: `docs/OPERATOR_RUNBOOK.md`

**Recommended structure:**
- Base dependencies: lightweight, CI-safe (`pyyaml` only if needed)
- Optional extras:
  - `training`: `torch`, `transformers`, `peft`, `trl`, `datasets`, `accelerate`, `bitsandbytes` where platform-compatible
  - `eval`: `transformers`, `datasets`, maybe `jsonschema`
  - `gguf`: document external `llama.cpp` tooling rather than vendoring it

**Constraints:**
- Do not install heavyweight packages in default CI.
- Do not require GPU for dry-run tests.
- Document RTX 5070 / Blackwell caveats already known for Phoenix/local training.

**Validation:**
- `python3 -m compileall training evals scripts/models`
- Documentation clearly explains real-mode install command.

### Task 4.2: Implement real-mode trace dataset export bridge

**Objective:** Connect consent-gated memory/trace records to a dataset file usable by training/eval scripts.

**Files:**
- Modify: `crates/devil-memory/src/lib.rs`
- Modify or create: `crates/devil-memory/tests/trace_dataset_export.rs`
- Modify: `crates/devil-cli/src/main.rs`
- Modify: `training/README.md`

**Implementation details:**
1. Add or verify CLI command such as:
   - `devil-cli trace export --format jsonl --output datasets/legion-traces.jsonl`
2. Export must:
   - require explicit consent state
   - use redacted payload summaries by default
   - include payload hashes / provenance
   - reject raw secret markers
   - support delete/export controls
3. If raw payload export is supported, require a separate explicit flag and policy gate.

**Tests:**
- Export without consent fails.
- Export with consent writes JSONL with no raw secret markers.
- Export includes expected schema fields for training/eval.
- Delete/export controls work.

**Validation:**
- `cargo test -p devil-memory --all-targets trace`
- `cargo test -p devil-cli --all-targets trace`
- `cargo test -p devil-security --all-targets redaction`

### Task 4.3: Replace `evals/run_eval.py` scaffold with real local inference/eval path plus dry-run

**Objective:** Keep dry-run behavior but add real evaluation when model endpoint or model path is provided.

**Files:**
- Modify: `evals/run_eval.py`
- Create: `evals/fixtures/` if not present
- Create or modify: `evals/README.md`
- Possibly create: `evals/schema.py` or `evals/harness.py` if splitting logic improves maintainability

**Implementation details:**
1. Preserve `--dry-run` output and CI compatibility.
2. Add CLI options:
   - `--dataset <path>`
   - `--endpoint <openai-compatible-url>`
   - `--model <name>`
   - `--output <path>`
   - `--max-examples <n>`
   - `--offline-fixture` for deterministic local fixture mode
3. Implement evals for:
   - schema compliance
   - proposal patch applies
   - verification command success metadata
   - regression preservation metadata
   - latency/cost/refusal metrics where available
4. Use local HTTP endpoint if provided; otherwise fixture mode should run without network.
5. Ensure no raw secrets in output.

**Tests / validation:**
- `python3 evals/run_eval.py --dry-run`
- `python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/legion-eval.json`
- `python3 -m compileall evals`
- If Python unit test framework is introduced: `python3 -m unittest discover evals`

**Acceptance criteria:**
- Eval script can run a real deterministic fixture path and produce a metrics JSON artifact.
- Real endpoint path is implemented but optional.

### Task 4.4: Replace `training/qlora_train.py` scaffold with real training entrypoint plus dry-run

**Objective:** Add a real operator-provisioned QLoRA training path while preserving dry-run for CI.

**Files:**
- Modify: `training/qlora_train.py`
- Possibly create: `training/config.py`
- Possibly create: `training/dataset.py`
- Possibly create: `training/fixtures/minimal_traces.jsonl`
- Modify: `training/README.md`

**Implementation details:**
1. Preserve current `--dry-run` behavior.
2. Add CLI options:
   - `--dataset`
   - `--base-model`
   - `--output-dir`
   - `--max-steps`
   - `--learning-rate`
   - `--lora-rank`
   - `--sequence-length`
   - `--device auto|cuda|mps|cpu`
   - `--fixture-smoke` for tiny CPU-safe smoke that does not require full model training
3. In real mode, import training dependencies lazily after argument validation.
4. If dependencies are missing, print exact install instructions and exit non-zero with a clear error.
5. Validate dataset schema before training.
6. Write training metadata manifest with model ID, dataset hash, parameters, and consent/export provenance.

**Tests / validation:**
- `python3 training/qlora_train.py --dry-run`
- `python3 training/qlora_train.py --fixture-smoke --dataset training/fixtures/minimal_traces.jsonl --output-dir /tmp/legion-train-smoke` if fixture smoke is implemented without heavyweight deps
- `python3 -m compileall training`

**Acceptance criteria:**
- Script is no longer only a print-plan scaffold; it has a real operator-provisioned code path.
- Dry-run and fixture-smoke remain CI-safe.

### Task 4.5: Replace `training/convert_to_gguf.py` scaffold with real conversion command wrapper plus dry-run

**Objective:** Add a real GGUF conversion wrapper around an operator-provided llama.cpp conversion tool.

**Files:**
- Modify: `training/convert_to_gguf.py`
- Modify: `training/README.md`
- Possibly create: `training/tests` or fixture metadata

**Implementation details:**
1. Preserve `--dry-run`.
2. Add CLI options:
   - `--model-dir`
   - `--output`
   - `--llama-cpp-convert-script`
   - `--quantize-command` or `--quantization`
   - `--metadata-output`
3. Validate paths and tool availability.
4. Run conversion via `subprocess.run` with explicit args, not shell interpolation.
5. Capture stdout/stderr to an evidence log.
6. Write conversion manifest with source model hash/metadata and output path.

**Tests / validation:**
- `python3 training/convert_to_gguf.py --dry-run`
- A fixture mode that invokes a fake converter script and verifies command construction/output manifest.
- `python3 -m compileall training`

### Task 4.6: Add training/eval evidence and close finding

**Objective:** Convert the training/eval finding from deferred/scaffold to completed optional real execution support.

**Files:**
- Create: `plans/evidence/legion-e2e/<timestamp>_training_eval_real_mode_gates.txt`
- Modify: `ENGINEERING_AUDIT.yaml`
- Regenerate: `ENGINEERING_AUDIT.html`
- Modify: `ENGINEERING_STATUS.md`

**Validation commands:**
- `python3 evals/run_eval.py --dry-run`
- `python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/legion-eval.json`
- `python3 training/qlora_train.py --dry-run`
- `python3 training/qlora_train.py --fixture-smoke ...` if implemented
- `python3 training/convert_to_gguf.py --dry-run`
- fake-converter fixture command
- `python3 -m compileall training evals scripts/models`
- `cargo test -p devil-memory --all-targets trace`
- `cargo test -p devil-security --all-targets redaction`

**Acceptance criteria:**
- `finding-training-eval-real-execution-deferred.status` becomes `fixed` only if real optional execution paths exist, not just dry-run docs.

---

## Phase 5 — CI Coverage for Model/Training/Eval Dry-runs

### Task 5.1: Add Python/model dry-run CI job or steps

**Objective:** Ensure CI directly exercises model/training/eval dry-run and compileall commands.

**Files:**
- Modify: `.github/workflows/ci.yml`
- Possibly modify: `docs/OPERATOR_RUNBOOK.md`

**Implementation details:**
1. Add a step after Rust setup or after evidence gates:

```yaml
      - name: Phase 8 model/training/eval dry runs
        if: runner.os == 'Linux'
        run: |
          bash scripts/models/download-models.sh --dry-run
          bash scripts/models/start-local-workers.sh --dry-run --config config/workers.example.yaml
          python3 evals/run_eval.py --dry-run
          python3 training/qlora_train.py --dry-run
          python3 training/convert_to_gguf.py --dry-run
          python3 -m compileall training evals scripts/models
```

2. Prefer Linux-only for speed unless cross-platform parity is required.
3. If scripts are intended cross-platform, add macOS too and leave Windows for PowerShell-compatible future task.
4. Do not install heavyweight training extras in CI.

**Validation:**
- Locally run the exact command block.
- If using `act` or CI is available, verify workflow syntax.
- `cargo test --workspace --all-targets`

**Acceptance criteria:**
- CI directly catches syntax/regression failures in training/eval/model scripts.
- `finding-training-dry-runs-not-in-ci.status` can move to `fixed`.

### Task 5.2: Add local cargo-deny parity instructions or installer check

**Objective:** Resolve local/CI gate parity gap without requiring cargo-deny for every developer shell.

**Files:**
- Modify: `docs/OPERATOR_RUNBOOK.md`
- Possibly modify: `AGENTS.md`
- Possibly modify: `scripts/run-phase-gates.sh`

**Implementation options:**
1. Add docs:
   - `cargo install cargo-deny`
   - `cargo deny check`
2. Add optional `scripts/run-phase-gates.sh --with-deny` behavior if script already supports flags.
3. Do not make local deny mandatory if CI already runs action and setup is heavyweight.

**Validation:**
- Docs contain exact command.
- If script changed, run script dry-run or targeted command.

---

## Phase 6 — Evidence Gate Integration and Audit Closure

### Task 6.1: Extend `devil-cli evidence check` for new artifacts if appropriate

**Objective:** Make new beta/cloud/training evidence first-class rather than loose files.

**Files:**
- Modify: `crates/devil-cli/src/main.rs`
- Modify: `plans/evidence/legion-e2e/README.md`
- Possibly modify: `plans/phase-status-ledger.md`

**Implementation details:**
1. Inspect existing evidence checker phases: `phase8`, `gui-phase6`, `gui-phase7`, `gui-phase8`.
2. Add checks for:
   - beta acceptance e2e artifact
   - cloud lane HTTP transport gate artifact
   - training/eval real-mode gate artifact
   - CI dry-run script coverage if represented in docs
3. Avoid brittle exact timestamp requirements; check for latest matching artifact pattern and required markers.
4. Add tests in `devil-cli` for missing/stale marker rejection.

**Validation:**
- `cargo test -p devil-cli --all-targets evidence`
- `cargo run -p devil-cli -- evidence check --phase legion-e2e` if new phase is added
- Existing evidence checks still pass.

### Task 6.2: Update canonical audit statuses and generated report

**Objective:** Close all findings in the canonical audit after implementation evidence exists.

**Files:**
- Modify: `ENGINEERING_AUDIT.yaml`
- Regenerate: `ENGINEERING_AUDIT.html`
- Modify: `ENGINEERING_STATUS.md`

**Status changes only after evidence passes:**
- `finding-product-readiness-ledger-stale`: `fixed`
- `finding-unified-beta-e2e-test-missing`: `fixed`
- `finding-cloud-lane-production-transport-missing`: `fixed`
- `finding-training-eval-real-execution-deferred`: `fixed`
- `finding-agents-placeholder-note-stale`: `fixed`
- `finding-training-dry-runs-not-in-ci`: `fixed`

**Validation:**
- `python3 <skill>/scripts/validate_engineering_audit.py ENGINEERING_AUDIT.yaml`
- `python3 <skill>/scripts/generate_engineering_audit_html.py ENGINEERING_AUDIT.yaml ENGINEERING_AUDIT.html`

### Task 6.3: Final full gates

**Objective:** Verify the implementation line is complete.

**Commands:**

```bash
cargo run -p xtask -- check-deps
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p devil-cli -- evidence check --phase phase8
cargo run -p devil-cli -- evidence check --phase gui-phase6
cargo run -p devil-cli -- evidence check --phase gui-phase7
cargo run -p devil-cli -- evidence check --phase gui-phase8
# plus new evidence phase if added:
cargo run -p devil-cli -- evidence check --phase legion-e2e
bash scripts/models/download-models.sh --dry-run
bash scripts/models/start-local-workers.sh --dry-run --config config/workers.example.yaml
python3 evals/run_eval.py --dry-run
python3 training/qlora_train.py --dry-run
python3 training/convert_to_gguf.py --dry-run
python3 -m compileall training evals scripts/models
```

If `cargo-deny` is installed locally:

```bash
cargo deny check
```

Otherwise verify CI deny remains configured in `.github/workflows/ci.yml`.

**Acceptance criteria:**
- All local gates pass.
- CI passes on pushed branch.
- `ENGINEERING_STATUS.md` shows 0 active unresolved findings.

---

## Files Likely to Change

Documentation / audit:

- `AGENTS.md`
- `plans/product-readiness-ledger.md`
- `plans/evidence/legion-e2e/README.md`
- `plans/evidence/legion-e2e/*_beta_acceptance_e2e.txt`
- `plans/evidence/legion-e2e/*_cloud_lane_http_transport_gates.txt`
- `plans/evidence/legion-e2e/*_training_eval_real_mode_gates.txt`
- `ENGINEERING_AUDIT.yaml`
- `ENGINEERING_AUDIT.html`
- `ENGINEERING_STATUS.md`
- `docs/OPERATOR_RUNBOOK.md`
- `training/README.md`
- `evals/README.md`

Beta e2e:

- `crates/devil-desktop/tests/beta_acceptance_e2e.rs`
- possibly `crates/devil-desktop/Cargo.toml`
- possibly `crates/devil-vscode-compat/tests/*`

Cloud lane:

- `crates/devil-remote/src/lib.rs`
- `crates/devil-remote/tests/cloud_lane_http_transport.rs`
- `crates/devil-remote/Cargo.toml`
- possibly `crates/devil-app/src/lib.rs`
- possibly `config/cloud-lane.example.toml`
- possibly `Cargo.toml`
- possibly `plans/dependency-policy.md`
- possibly `xtask/src/main.rs`

Training/eval:

- `pyproject.toml` or `training/pyproject.toml`
- `evals/run_eval.py`
- `evals/fixtures/minimal.jsonl`
- possibly `evals/harness.py`
- `training/qlora_train.py`
- `training/convert_to_gguf.py`
- `training/fixtures/minimal_traces.jsonl`
- possibly `training/dataset.py`
- possibly `crates/devil-memory/src/lib.rs`
- possibly `crates/devil-memory/tests/trace_dataset_export.rs`
- possibly `crates/devil-cli/src/main.rs`

CI:

- `.github/workflows/ci.yml`
- possibly `scripts/run-phase-gates.sh`

---

## Test / Validation Matrix

| Area | Target commands |
| --- | --- |
| Docs/guidance | `cargo run -p xtask -- check-deps`; audit validator/generator |
| Product readiness evidence | `cargo run -p devil-cli -- evidence check --phase gui-phase8` |
| Beta e2e | `cargo test -p devil-desktop --test beta_acceptance_e2e -- --nocapture`; `cargo test -p devil-desktop --test beta_workflow -- --nocapture` |
| VS Code compatibility metadata | `cargo test -p devil-vscode-compat --all-targets` |
| Cloud transport | `cargo test -p devil-remote --test cloud_lane_http_transport -- --nocapture`; `cargo test -p devil-remote --all-targets`; `cargo test -p devil-security --all-targets cloud` |
| App cloud integration | `cargo test -p devil-app --all-targets cloud` |
| Trace export | `cargo test -p devil-memory --all-targets trace`; `cargo test -p devil-cli --all-targets trace`; `cargo test -p devil-security --all-targets redaction` |
| Eval real/fixture path | `python3 evals/run_eval.py --dry-run`; `python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/legion-eval.json` |
| Training path | `python3 training/qlora_train.py --dry-run`; fixture smoke if implemented |
| GGUF conversion | `python3 training/convert_to_gguf.py --dry-run`; fake-converter fixture if implemented |
| Python syntax | `python3 -m compileall training evals scripts/models` |
| Full Rust gates | `cargo fmt --all --check`; `cargo check --workspace --all-targets`; `cargo test --workspace --all-targets`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo run -p xtask -- check-deps` |
| CI-only deny | CI `EmbarkStudios/cargo-deny-action@v2`, or local `cargo deny check` if installed |

---

## Risks, Tradeoffs, and Open Questions

### Risks

1. **Cloud transport dependency creep:** Adding a production HTTP/gRPC carrier may require new dependencies and dependency-policy changes. Prefer existing `reqwest` if already available.
2. **Training dependency weight:** Real training dependencies are heavy and platform-sensitive. Keep them optional and out of normal CI.
3. **Beta e2e brittleness:** A single large scenario can become flaky if it depends on external services. Use deterministic local fixtures and mocks.
4. **Overstating readiness:** Product ledger reconciliation must not claim GA readiness just because substrate tests pass.
5. **Audit artifact policy:** `.hermes/` artifacts may or may not be intended for commit. Decide before implementation branch cleanup.
6. **CI runtime:** Additional dry-run steps should be lightweight; avoid downloading models or installing PyTorch in CI.

### Tradeoffs

- A real HTTP cloud carrier is more immediately useful than a full production control plane, but it should be contract-tested so it can later point at a real service.
- Fixture-based real eval/training smoke is safer for CI than GPU-dependent tests, but operator-provisioned real-mode docs and code must exist to resolve the scaffold finding.
- Updating `devil-cli evidence check` creates stronger release gates but may require maintenance when evidence artifact names evolve.

### Open Questions

1. Should `.hermes/audit-packets/` and this plan be committed, or should durable audit artifacts live only under `plans/evidence/`?
2. Should cloud transport be HTTP JSON first, or should it reuse `devil-remote-transport` TLS/mTLS carrier directly?
3. What exact cloud control-plane API shape should be considered production-compatible for Phase 7: local mock server only, hosted endpoint adapter, or both?
4. Should real training support target Unsloth first, Axolotl first, or a generic Transformers/PEFT QLoRA path first?
5. Should the beta acceptance e2e be one large test or one test file with stage-specific tests plus an evidence matrix? The audit finding asks for a single unified scenario or accepted traceability matrix.

---

## Suggested Implementation Sequencing with Subagents

Use one Kimi subagent per task packet. Do not give a subagent this whole plan except the specific task it owns.

Recommended batches:

1. **Batch A: Documentation/evidence reconciliation**
   - Task 1.1 product ledger
   - Task 1.2 AGENTS guidance
   - GPT-5.5 verifies and updates audit artifacts

2. **Batch B: Unified beta e2e**
   - Task 2.1 beta acceptance test
   - Task 2.2 evidence artifact
   - Reviewer 1: spec compliance
   - Reviewer 2: quality/flakiness/policy review

3. **Batch C: Cloud lane transport**
   - Task 3.1 contract doc
   - Task 3.2 failing tests
   - Task 3.3 implementation
   - Task 3.4 app/config wiring
   - Task 3.5 evidence/audit update

4. **Batch D: Training/eval real-mode harness**
   - Task 4.1 dependency metadata
   - Task 4.2 trace export bridge
   - Task 4.3 eval real/fixture path
   - Task 4.4 training real path
   - Task 4.5 conversion wrapper
   - Task 4.6 evidence/audit update

5. **Batch E: CI and final closure**
   - Task 5.1 CI dry-run coverage
   - Task 5.2 cargo-deny parity docs
   - Task 6.1 evidence checker integration
   - Task 6.2 audit closure
   - Task 6.3 final gates

Commit after each coherent task or small task group. Run targeted tests before commit; run full gates at batch boundaries and before push.

---

## Completion Definition

This plan is complete when:

- All six audit findings are fixed in code/docs/tests/evidence.
- No finding is resolved by deleting planned features or weakening acceptance criteria.
- `ENGINEERING_AUDIT.yaml` validates and shows no active unresolved findings.
- `ENGINEERING_AUDIT.html` is regenerated.
- `ENGINEERING_STATUS.md` reports zero active findings.
- Product readiness ledger accurately distinguishes completed, in-progress, substrate-validated, and deferred surfaces.
- Unified beta acceptance e2e or accepted traceability matrix exists and passes.
- Cloud lane has a production-capable transport path with default-deny policy and mock/server contract tests.
- Training/eval scripts have real optional execution paths plus CI-safe dry-run/fixture paths.
- CI directly runs model/training/eval dry-runs.
- Full local gates pass and CI passes.
