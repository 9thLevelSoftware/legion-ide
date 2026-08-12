# 05 — Legion Model Acquisition, Training, Evaluation, and Serving Plan

Generated: 2026-06-01 16:24:53 EDT

## 0. Executive summary

Legion should not try to train a general coding model first. It should acquire strong open code models and train narrow specialists.

Recommended first models:

1. `Qwen/Qwen2.5-Coder-1.5B-Instruct`
   - first tiny specialist base.
   - docs, summaries, simple tests, lint fixes.

2. `Qwen/Qwen2.5-Coder-3B-Instruct`
   - default local specialist base.
   - compiler-error fixer, simple Rust/TS patches, tests.

3. `Qwen/Qwen2.5-Coder-7B-Instruct`
   - stronger local reviewer/senior specialist.
   - harder debugging, cross-file reasoning.

4. `bigcode/starcoder2-3b`
   - comparison baseline.
   - useful for license/model diversity testing.

5. `deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct`
   - optional larger/moe-ish comparison and cloud specialist candidate.
   - not first local RTX 5070 target.

Remote escalation models:

- Kimi 2.6 via Fireworks/custom provider.
- Codex/GPT-5.5 subscription route as planner/architect where integration allows.
- Other strong providers as optional route.

Local RTX 5070 12GB plan:

- QLoRA fine-tune 1.5B and 3B locally.
- 7B QLoRA is possible but slower/tighter.
- Inference via llama.cpp GGUF quantized models.
- Multiple 1.5B/3B workers are realistic if context is controlled.

Cloud plan:

- Use RunPod/Lambda/Modal for larger experiments and training.
- Use cloud for 7B+ or multi-run hyperparameter searches.

## 1. Verified model metadata

Direct Hugging Face API checks performed 2026-06-01.

### 1.1 Qwen2.5-Coder-1.5B-Instruct

Model:

- `Qwen/Qwen2.5-Coder-1.5B-Instruct`

Observed:

- pipeline: text-generation.
- tags include: transformers, safetensors, qwen2, text-generation, code, qwen-coder, conversational.
- last modified: 2025-01-12.
- downloads observed: 726,756.

Use:

- docs specialist.
- changelog specialist.
- diff summarizer.
- simple test writer.
- lint fixer.
- issue/PR summarizer.

### 1.2 Qwen2.5-Coder-3B-Instruct

Model:

- `Qwen/Qwen2.5-Coder-3B-Instruct`

Observed:

- pipeline: text-generation.
- tags include: transformers, safetensors, qwen2, text-generation, code, qwen-coder, conversational.
- last modified: 2025-01-12.
- downloads observed: 209,703.

Use:

- default local specialist.
- Rust compiler-error fixer.
- TypeScript small patcher.
- test writer.
- structural refactor explainer.

### 1.3 Qwen2.5-Coder-7B-Instruct

Model:

- `Qwen/Qwen2.5-Coder-7B-Instruct`

Observed:

- pipeline: text-generation.
- tags include: transformers, safetensors, qwen2, text-generation, code, qwen-coder, conversational.
- last modified: 2025-01-12.
- downloads observed: 2,305,681.

Use:

- stronger local reviewer.
- harder debugging.
- cross-file reasoning.
- escalation before cloud.

### 1.4 Qwen2.5-Coder-14B-Instruct

Model:

- `Qwen/Qwen2.5-Coder-14B-Instruct`

Observed:

- pipeline: text-generation.
- tags include code/qwen-coder.
- last modified: 2025-01-12.
- downloads observed: 2,418,312.

Use:

- cloud candidate.
- not recommended for local RTX 5070 12GB default.

### 1.5 StarCoder2-3B

Model:

- `bigcode/starcoder2-3b`

Observed:

- pipeline: text-generation.
- license tag: bigcode-openrail-m.
- dataset: The Stack v2.
- last modified: 2024-03-04.

Use:

- comparison baseline.
- alternate 3B specialist base.

### 1.6 DeepSeek-Coder-V2-Lite-Instruct

Model:

- `deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct`

Observed:

- pipeline: text-generation.
- custom_code.
- text-generation-inference compatible tag.
- last modified: 2024-07-03.

Use:

- optional cloud comparison model.
- not first local training target.

## 2. Model acquisition plan

### Step 1: Create local model directory

```bash
mkdir -p ~/legion-models/base
mkdir -p ~/legion-models/adapters
mkdir -p ~/legion-models/gguf
mkdir -p ~/legion-models/evals
mkdir -p ~/legion-models/datasets
```

### Step 2: Install model tooling

Required:

- Python 3.11+.
- PyTorch with CUDA support.
- transformers.
- datasets.
- peft.
- trl.
- accelerate.
- bitsandbytes.
- unsloth.
- huggingface_hub.
- sentencepiece.
- protobuf.
- llama.cpp.
- jq.
- git-lfs.

Suggested install:

```bash
python3 -m venv ~/venvs/legion-train
source ~/venvs/legion-train/bin/activate
pip install -U pip
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu128
pip install -U transformers datasets accelerate peft trl bitsandbytes huggingface_hub sentencepiece protobuf evaluate scikit-learn pandas numpy
pip install -U unsloth
```

Note:

- Exact CUDA wheel depends on installed driver/toolkit. Verify with `nvidia-smi` and PyTorch CUDA availability.
- On RTX 5070/Blackwell, confirm PyTorch build supports the GPU.

### Step 3: Install Hugging Face CLI

```bash
pip install -U huggingface_hub
huggingface-cli login
```

If avoiding token in shell history:

```bash
huggingface-cli login
```

Use interactive prompt.

### Step 4: Download base models

```bash
huggingface-cli download Qwen/Qwen2.5-Coder-1.5B-Instruct --local-dir ~/legion-models/base/qwen2.5-coder-1.5b-instruct
huggingface-cli download Qwen/Qwen2.5-Coder-3B-Instruct --local-dir ~/legion-models/base/qwen2.5-coder-3b-instruct
huggingface-cli download Qwen/Qwen2.5-Coder-7B-Instruct --local-dir ~/legion-models/base/qwen2.5-coder-7b-instruct
huggingface-cli download bigcode/starcoder2-3b --local-dir ~/legion-models/base/starcoder2-3b
```

Optional:

```bash
huggingface-cli download deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct --local-dir ~/legion-models/base/deepseek-coder-v2-lite-instruct
```

### Step 5: Acquire GGUFs for inference

Option A:

- Download existing GGUF quantizations from trusted Hugging Face repos.

Option B:

- Convert yourself with llama.cpp after fine-tuning/merging.

For initial local inference, use existing GGUF if available.

Suggested quantization levels:

- Q4_K_M for cheapest memory.
- Q5_K_M for better quality.
- Q6_K for reviewer if memory allows.

## 3. First specialist roster

### 3.1 Specialist 1: Legion Docs Summarizer 1.5B

Base:

- Qwen2.5-Coder-1.5B-Instruct.

Tasks:

- summarize diff.
- write release notes.
- write PR description.
- explain validation result.
- summarize worker evidence.

Why first:

- easiest to train/evaluate.
- low risk.
- useful immediately.
- generates product-visible value.

Output schema:

```json
{
  "status": "ok",
  "summary": "...",
  "changed_files": [],
  "risk_notes": [],
  "user_facing_release_note": "..."
}
```

### 3.2 Specialist 2: Legion Rust Compiler Fixer 3B

Base:

- Qwen2.5-Coder-3B-Instruct.

Tasks:

- given cargo error + relevant files, propose minimal patch.
- output unified diff only or BLOCKED.

Output schema:

```json
{
  "status": "patch|blocked",
  "rationale": "...",
  "allowed_files_used": [],
  "diff": "...",
  "validation_commands": ["cargo check -p ..."]
}
```

### 3.3 Specialist 3: Legion Test Writer 3B

Base:

- Qwen2.5-Coder-3B-Instruct.

Tasks:

- write focused unit tests.
- identify edge cases.
- avoid broad unrelated edits.

Output schema:

```json
{
  "status": "patch|blocked",
  "test_intent": "...",
  "cases": [],
  "diff": "...",
  "validation_commands": []
}
```

### 3.4 Specialist 4: Legion Reviewer 7B

Base:

- Qwen2.5-Coder-7B-Instruct.

Tasks:

- review proposal.
- detect likely issues.
- classify risk.
- suggest validation commands.

Output schema:

```json
{
  "status": "approve|reject|needs_human|needs_more_validation",
  "risk_level": "low|medium|high",
  "findings": [],
  "required_validation": [],
  "human_review_reason": null
}
```

## 4. Data collection plan

### 4.1 Trace schema

Every Legion task should optionally produce a training trace:

```json
{
  "trace_id": "...",
  "project_hash": "...",
  "task_kind": "rust_compiler_fix",
  "role": "rust_compiler_fixer",
  "input": {
    "objective": "...",
    "allowed_files": [],
    "context_files": [],
    "diagnostics": [],
    "command_outputs": []
  },
  "model_output": {
    "raw": "...",
    "parsed": {}
  },
  "validation": {
    "commands": [],
    "passed": true,
    "logs_redacted": "..."
  },
  "human_decision": {
    "accepted": true,
    "rejected_reason": null
  },
  "final_diff": "...",
  "metadata": {
    "model": "...",
    "provider": "...",
    "tokens": 0,
    "duration_ms": 0
  }
}
```

### 4.2 What to include

Include:

- task packet.
- selected context.
- diagnostics/test output.
- generated output.
- validation result.
- accepted final diff.
- user rejection reason.

Exclude/redact:

- secrets.
- credentials.
- private customer identifiers.
- proprietary code unless user opted in.
- unnecessary full repo context.

### 4.3 Consent model

Default:

- local trace collection only.
- no upload.

Opt-in options:

1. Do not collect traces.
2. Collect local traces for personal fine-tuning.
3. Share anonymized traces with Legion to improve models.
4. Enterprise private trace store.

## 5. Dataset construction

### 5.1 Start with synthetic/teacher data

Before many real traces exist, use teacher models to generate examples.

Teacher models:

- Kimi 2.6.
- Codex/GPT-5.5 route.
- other strong coding models.

Procedure:

1. Gather real compiler errors/test failures from open-source repos.
2. Build task packets.
3. Ask teacher model for minimal patch.
4. Apply patch.
5. Run validation.
6. Keep only passing examples.
7. Store failed attempts for eval/rejection training later.

### 5.2 Use open-source repos

Candidate data sources:

- Legion repo itself.
- Rust small crates.
- TypeScript sample projects.
- deliberately seeded bugs.
- historical commits where error before/after can be reconstructed.

### 5.3 Dataset splits

For each specialist:

- train: 80%.
- validation: 10%.
- held-out eval: 10%.

Avoid leakage:

- split by repo/project, not random examples, if possible.
- keep Legion current tasks out of training eval.

### 5.4 Minimum useful dataset sizes

Docs summarizer:

- 500 examples for first LoRA.
- 2,000+ better.

Rust compiler fixer:

- 1,000 validated examples minimum.
- 5,000+ better.

Test writer:

- 1,000 examples minimum.
- 3,000+ better.

Reviewer:

- 2,000 examples minimum.
- include accepted/rejected/needs-human labels.

## 6. Training approach

### 6.1 Use QLoRA first

Why:

- fits RTX 5070 12GB for 1.5B/3B.
- cheap.
- easy to iterate.
- adapters can be swapped per specialist.

LoRA target modules for Qwen-like models:

- q_proj.
- k_proj.
- v_proj.
- o_proj.
- gate_proj.
- up_proj.
- down_proj.

Start settings:

```yaml
lora_r: 16
lora_alpha: 32
lora_dropout: 0.05
learning_rate: 2e-4
batch_size: 1
gradient_accumulation_steps: 8
max_seq_length: 4096
num_epochs: 2
warmup_ratio: 0.03
```

Tune later.

### 6.2 Sequence lengths

Docs summarizer:

- 2048–4096.

Compiler fixer:

- 4096–8192 if memory permits.

Test writer:

- 4096–8192.

Reviewer:

- 8192 desirable, likely cloud for 7B.

### 6.3 Local vs cloud training

Local RTX 5070 12GB:

- 1.5B QLoRA: yes.
- 3B QLoRA: yes.
- 7B QLoRA: possible but constrained.
- 14B: not recommended locally.

Cloud:

- 7B multi-run sweeps.
- 14B experiments.
- larger context training.
- faster iteration.

## 7. Training tools

### 7.1 Unsloth

Use for:

- fast QLoRA.
- Qwen fine-tuning.
- lower VRAM.
- quick local iteration.

### 7.2 Axolotl

Use for:

- reproducible YAML-based training.
- multi-GPU/cloud training.
- production training configs.

### 7.3 TRL

Use for:

- SFT.
- DPO later.
- preference tuning with accepted/rejected examples.

### 7.4 llama.cpp

Use for:

- GGUF conversion.
- quantization.
- local model serving.
- OpenAI-compatible server.

### 7.5 vLLM

Use for:

- cloud serving.
- batching.
- higher throughput.

### 7.6 Evaluation tools

Build custom harness first.

Use:

- Python eval scripts.
- pytest for harness.
- git apply checks.
- cargo check/test for patch tasks.
- schema compliance checks.

Optional:

- lm-eval-harness for generic comparison, but custom task eval matters more.

## 8. Baby-step training plan

## Stage 1 — Environment verification

1. Run `nvidia-smi`.
2. Verify GPU VRAM.
3. Verify PyTorch sees CUDA.
4. Run tiny inference with Qwen2.5-Coder-1.5B.
5. Run a 10-example LoRA smoke test.
6. Save adapter.
7. Merge adapter.
8. Quantize to GGUF.
9. Serve via llama.cpp.
10. Run one Legion task packet through it.

Exit criteria:

- full train → merge → quantize → serve pipeline works.

## Stage 2 — Docs summarizer specialist

1. Create 200 hand/teacher-generated examples.
2. Define schema.
3. Train Qwen2.5-Coder-1.5B LoRA.
4. Evaluate schema compliance.
5. Evaluate summary quality manually.
6. Generate 500 more examples.
7. Retrain.
8. Quantize Q5_K_M.
9. Serve locally.
10. Add to Legion as docs worker.

Exit criteria:

- beats base model on held-out doc summary tasks.

## Stage 3 — Rust compiler fixer specialist

1. Collect compiler errors.
2. Build task packets with allowed files.
3. Use teacher to propose patches.
4. Validate with cargo check.
5. Keep passing examples.
6. Train Qwen2.5-Coder-3B LoRA.
7. Evaluate:
   - schema compliance.
   - patch applies.
   - cargo check passes.
   - allowed files only.
8. Quantize.
9. Serve as local worker.
10. Add retry/escalation path.

Exit criteria:

- on held-out seeded bugs, model produces valid passing patch at useful rate.

## Stage 4 — Test writer specialist

1. Collect functions/modules and existing test style.
2. Generate candidate tests with teacher.
3. Run tests.
4. Keep examples that pass and add coverage/value.
5. Train Qwen2.5-Coder-3B LoRA.
6. Evaluate on held-out modules.
7. Quantize.
8. Add to worker roster.

Exit criteria:

- generated tests compile and catch seeded bugs.

## Stage 5 — Reviewer specialist

1. Collect proposals.
2. Label accepted/rejected/high-risk.
3. Train Qwen2.5-Coder-7B LoRA or use cloud.
4. Evaluate findings against known bad patches.
5. Require conservative behavior.

Exit criteria:

- reviewer catches out-of-scope and risky patches better than base.

## Stage 6 — Preference tuning

After enough rejected/accepted pairs:

1. Build pairs:
   - chosen: accepted/validated output.
   - rejected: invalid/rejected output.
2. Run DPO/ORPO experiment with TRL.
3. Evaluate schema compliance and patch success.
4. Do not deploy unless improves real eval.

## 9. Evaluation harness

### 9.1 Eval types

Docs:

- schema compliance.
- factuality against diff.
- brevity.
- no invented files.

Compiler fixer:

- JSON/schema compliance.
- diff parse success.
- patch apply success.
- allowed path compliance.
- cargo check pass.
- cargo test pass if relevant.

Test writer:

- patch apply.
- tests compile.
- tests pass.
- tests fail on seeded bug if possible.

Reviewer:

- catches known bad patch.
- flags high-risk path.
- avoids false approval.
- suggests correct validation.

### 9.2 Eval command sketch

```bash
python evals/run_eval.py \
  --model http://localhost:8101/v1 \
  --suite rust_compiler_fixer \
  --dataset ~/legion-models/evals/rust_compiler_fixer_heldout.jsonl \
  --out ~/legion-models/evals/reports/run.json
```

### 9.3 Metrics

Track:

- schema compliance rate.
- patch apply rate.
- validation pass rate.
- allowed file violation rate.
- blocked-when-should-block rate.
- hallucinated file rate.
- average tokens.
- latency.
- cost.

Deploy threshold example:

```text
schema compliance: >= 98%
allowed file violations: 0%
patch apply rate: >= 80%
validation pass rate: >= 40% for compiler-fixer seeded tasks
hallucinated files: <= 2%
```

## 10. Serving plan

### 10.1 Local serving with llama.cpp

Example:

```bash
llama-server \
  -m ~/legion-models/gguf/legion-docs-1.5b-q5_k_m.gguf \
  --host 127.0.0.1 \
  --port 8101 \
  --ctx-size 4096
```

Second worker:

```bash
llama-server \
  -m ~/legion-models/gguf/legion-rust-fixer-3b-q5_k_m.gguf \
  --host 127.0.0.1 \
  --port 8102 \
  --ctx-size 8192
```

### 10.2 Local worker config

```yaml
workers:
  docs_summarizer:
    provider: openai_compatible
    endpoint: http://127.0.0.1:8101/v1
    model: legion-docs-1.5b
    max_context: 4096

  rust_compiler_fixer:
    provider: openai_compatible
    endpoint: http://127.0.0.1:8102/v1
    model: legion-rust-fixer-3b
    max_context: 8192
```

### 10.3 Cloud serving

Use:

- vLLM for batching.
- llama.cpp for cheap small-model serving.
- autoscaling pool.
- warm models.

## 11. Logistical plan

### 11.1 What to do this week

1. Download Qwen2.5-Coder-1.5B and 3B.
2. Get llama.cpp serving working.
3. Build task packet format.
4. Run base models on 20 tasks.
5. Build eval harness skeleton.
6. Generate first 200 docs examples.
7. Train first docs LoRA.
8. Quantize and serve.

### 11.2 What to do next month

1. Build trace collection into Legion.
2. Collect 1,000 docs traces.
3. Collect 1,000 compiler-fix traces.
4. Train docs v1.
5. Train rust-fixer v1.
6. Add local worker roster.
7. Add cloud small-model lane.
8. Benchmark cost/latency.

### 11.3 What to do after beta

1. Use accepted traces for continuous improvement.
2. Train per-language specialists.
3. Train per-framework specialists.
4. Add DPO preference tuning.
5. Add enterprise private fine-tunes.

## 12. Recommended exact acquisition order

1. Download `Qwen/Qwen2.5-Coder-1.5B-Instruct`.
2. Download `Qwen/Qwen2.5-Coder-3B-Instruct`.
3. Download GGUF quantizations for both or convert yourself.
4. Download `Qwen/Qwen2.5-Coder-7B-Instruct`.
5. Download `bigcode/starcoder2-3b` for baseline.
6. Only then test `DeepSeek-Coder-V2-Lite-Instruct` in cloud.
7. Do not spend time on 14B locally until the pipeline works.

## 13. Risks

1. Blackwell/PyTorch compatibility.
   - Verify before committing to local training pipeline.

2. Bad data.
   - Keep only validated examples for patch specialists.

3. Overfitting to one repo.
   - Split eval by repo/project.

4. Schema failures.
   - Train with strict output schema and validate parser.

5. Tiny model brittleness.
   - Use narrow task packets and escalation.

6. Fine-tune worse than base.
   - Require held-out eval improvement before deployment.

## 14. Final recommendation

Start with two local bases:

- Qwen2.5-Coder-1.5B-Instruct.
- Qwen2.5-Coder-3B-Instruct.

Build one full specialist pipeline before training many models:

```text
task packet → teacher/base output → validation → accepted trace → QLoRA → eval → GGUF → local server → Legion worker lane
```

The first production specialist should be docs/diff summarization. The second should be Rust compiler-error fixing. The third should be test generation.

Do not optimize for model cleverness first. Optimize for task packet quality, validation, and eval. Those are what make tiny Legion workers reliable.
