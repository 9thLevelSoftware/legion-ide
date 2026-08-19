# P9.F4.T3 — QLoRA training from a consented corpus, paired with the Legion-Bench baseline

Date: 2026-08-19
Source backlog row: `P9.F4.T3`
Git SHA at measurement: `6c27c8c27153241677e21d13d376ede54c16faa8`

## What this is, in one paragraph

A LoRA adapter was really trained, on a real GPU, from a corpus that really
passed the consent gate landed in P9.F4.T1/T2, and it was really evaluated
against the base model on a split withheld from its training. It was **not**
trained on the model the Legion-Bench baseline was measured with, and it does
not license any claim about that model. The section
[What this does and does not license](#what-this-does-and-does-not-license) is
the part to read before quoting any number here.

## The base model, and why it is not the benched one

| | Model | Where it appears |
|---|---|---|
| Legion-Bench raw baseline | `qwen2.5-coder:14b` (Q4_K_M, ~10 GB) | `plans/evidence/production/BENCH/baseline-raw-v1.md` |
| **Trained here** | **`Qwen/Qwen2.5-Coder-1.5B-Instruct`** (NF4, ~1.1 GB) | this document |

The GPU on this machine is an NVIDIA RTX 4070 Laptop with **8188 MiB** of VRAM.
A 14B model does not QLoRA-train in 8 GB: NF4 weights alone are roughly 8 GB
before optimizer state, activations, or gradients, and the run would not start.
That is not a tuning problem, it is arithmetic, so no amount of batch-size or
sequence-length reduction reaches it.

`Qwen2.5-Coder-1.5B-Instruct` was chosen because it is the closest thing to the
benched model that fits: same family, same tokenizer, same instruction format,
one thirtieth the parameters. That similarity narrows the gap between the two
models — it does not close it, and nothing below should be read as if it did.

## The corpus, and what it actually contains

The consented pipeline is metadata-first. A `TrainingCandidate` carries payload
kind, affected-file count, risk labels, privacy labels, and hashes; the proposal
title is stripped unless a live raw-trace opt-in row exists, and
`raw_trace_reference` is `null` throughout. There is no prompt text, no code, and
no user prose in the corpus, so the trainable task is the one the metadata
supports: **predict whether a reviewer accepted or rejected a proposal, from its
metadata alone.**

The checked-in fixture batch (`evals/training-candidates/source_traces.json`) is
seven traces, of which three are consented and terminal. Seven traces cannot
produce a train/holdout split with anything to measure. `xtask training-corpus
--expand N` therefore mints a larger **fixture** batch from the same template,
and this is the single most important caveat about the corpus:

> **No real user telemetry exists in this repository, and none was used.** The
> expanded batch is synthetic. Its labels are a stated function of the metadata
> features plus 8% seeded noise (`synthetic_lifecycle` in
> `xtask/src/training_corpus.rs`): rejected when the change is destructive
> (`DeleteFile`, `TerminalCommand`), high risk, or medium risk touching four or
> more files. A model that learns this corpus has learned that rule. It has not
> learned how reviewers behave.

What the expanded batch *is* good for is exercising the consent gate on every
run: its consent mix is 2-of-5 consented, so roughly 60% of traces are dropped
before anything reaches a trainer, and the drop is never zero by accident.

Export, at seed `20260819`, expanding to 1200 traces:

```
source_traces=1200 consented=419 dropped_unconsented=752 dropped_non_terminal=29
accepted=182 rejected=237 train=315 holdout=104
corpus_fingerprint=training-corpus-v1:4a94ac94e158dc9b
dataset_fingerprint=training-adapter-v1:35266a657f6d67f4
```

Retained consent states: `Granted: 212`, `NotRequired: 207`. Nothing else.

## Proving un-consented data cannot enter the training set

Three gates, at three different boundaries, plus the two that P9.F4.T1/T2
already had:

1. **Stage 1** (`build_training_candidate_corpus`) filters non-consented traces.
2. **Stage 2** (`build_training_adapter_dataset`) re-checks consent on every
   candidate, because a corpus is a file and files get hand-edited.
3. **Export** (`assert_export_is_consented`, new) re-derives consent *from the
   corpus candidate behind every emitted line*. This is the boundary that
   matters for this task: stages 1 and 2 protect the corpus, and the corpus is
   not what the GPU reads. `train.jsonl` is.
4. **Trainer** (`assert_dataset_is_consented` in `training/qlora_train.py`, new)
   refuses to load a model without an `export_manifest.json` whose retained
   consent states are all permitted, whose state counts add up to the candidate
   count, whose declared split size matches the line count, and whose every line
   carries a unique `example_id`.
5. **Eval** (`training/eval_adapter.py`) runs the same check on the holdout
   split, because an eval that reads unconsented data is the same leak.

Test coverage for these, all run below:

- `xtask` (13 tests in `training_corpus::tests`), including
  `every_unconsented_state_is_refused_at_the_export_boundary`, which builds a
  corpus carrying each of `Denied` / `Missing` / `RenewalRequired` and asserts
  the export refuses each one by name.
- `training/test_qlora_train.py` (15 tests), including
  `test_real_mode_refuses_to_train_without_a_consent_manifest`, which runs the
  CLI and asserts exit 2 with no output directory created.

### Mutation results, including one that did not kill

| Mutation | Result |
|---|---|
| `assert_export_is_consented`: short-circuit the consent branch to `false && ...` | **Killed** — `a_hand_edited_corpus_cannot_smuggle_an_unconsented_line_past_the_export` failed |
| `export_permits_consent`: add `RenewalRequired` to the permitted set | **Killed only by luck, first time round** — see below |
| `render_prompt`: drop the trailing space | **Killed** — prompt-masking test failed |
| `label_token_ids`: restore the original ` {label}` construction | **Killed** — `test_the_eval_prompt_and_label_reproduce_the_training_tokens` failed |

**The masking finding.** Widening `export_permits_consent` to accept
`RenewalRequired` was first caught only by an incidental count assertion
(`unconsented_audit_ids.len() == 3`) in a neighbouring test. The two tests that
were *supposed* to catch it —
`checked_in_batch_exports_only_consented_candidates` and
`the_expander_is_deterministic_and_drops_unconsented_traces` — asserted
consent by calling `export_permits_consent` itself, so widening the function
widened the assertion with it. Both were rewritten to compare against a literal
list of states.

Investigating why the *end-to-end* path did not notice turned up a second,
more interesting fact: it structurally cannot. `build_training_candidate_corpus`
filters first, so an export-side list that is *wider* than the pipeline's is
unobservable through any batch — the second list can only ever be narrower in
effect. The direction that can leak is the pipeline widening, and that is now
covered directly by `every_unconsented_state_is_refused_at_the_export_boundary`,
which constructs the offending corpus rather than hoping a batch produces one.

## The training run

Command (real, not a dry run, not a fixture smoke):

```sh
python training/qlora_train.py \
  --model-id Qwen/Qwen2.5-Coder-1.5B-Instruct \
  --dataset target/training-flywheel/train.jsonl \
  --consent-manifest target/training-flywheel/export_manifest.json \
  --output-dir target/training-flywheel/adapter \
  --max-steps 200 --batch-size 8 --sequence-length 256 \
  --learning-rate 2e-4 --lora-rank 16 --seed 20260819 --device cuda
```

Real output (abridged; the full manifest is archived at
`artifacts/adapter_training_manifest.json`):

```
consent gate: corpus=consented-accept-reject-v1 consented=419 dropped_unconsented=752 dropped_non_terminal=29 states={'Granted': 212, 'NotRequired': 207}
trainable params: 18464768 / 907081216
step 1/200 loss=11.1078
step 100/200 loss=0.1195
step 200/200 loss=0.1159
```

| | |
|---|---|
| Quantization | NF4, double-quantized, bf16 compute (`bitsandbytes` 0.50.1) |
| LoRA | rank 16, alpha 32, dropout 0.05, on `q,k,v,o,gate,up,down_proj` |
| Trainable / total params | 18,464,768 / 907,081,216 |
| Loss, first step → last step | 11.1078 → 0.1159 |
| Loss, first decile → last decile (mean) | 3.6116 → 0.1232 |
| Wall clock | 77.836 s |
| Peak VRAM | 2845 MiB (of 8188 available) |
| Adapter weights SHA-256 | `a84e60c3f6776bd8603d5c278ce8d98c60def5f59b5b95543c8935cb3ef059b0` |
| Stack | torch 2.6.0+cu124, transformers 4.46.3, peft 0.13.2, datasets 3.1.0, trl 0.12.1 |

The 74 MB `adapter_model.safetensors` is **not** checked in; the config,
manifest, loss curve, and digest are. The command above and seed `20260819`
regenerate it.

### Reproducibility, checked rather than asserted

The same command was run a second time into `adapter-repro/`. The result is
**bit-identical**, not merely close:

```
a84e60c3f6776bd8603d5c278ce8d98c60def5f59b5b95543c8935cb3ef059b0 *adapter/adapter_model.safetensors
a84e60c3f6776bd8603d5c278ce8d98c60def5f59b5b95543c8935cb3ef059b0 *adapter-repro/adapter_model.safetensors
```

All 200 loss values match exactly (`curve identical: True`, 0 differing steps).
This holds on one machine with one driver and one library stack; it is a
determinism claim about the harness — seeding, batch order, and data pipeline —
not a promise that a different GPU produces the same bytes.

## The comparison

Two different things are being compared, and conflating them is the easiest
mistake to make with this evidence.

### 1. Adapter vs. base, on the withheld holdout split

Both arms ran in one process against the same NF4-quantized base weights; the
adapter was attached after the base arm was scored, so the adapter is the only
difference between them. Scoring is a forced choice between the log-probabilities
of `Accepted` and `Rejected` — no sampling, no parser.

| Arm | Accuracy | Predicted `Accepted` | Predicted `Rejected` |
|---|---|---|---|
| Majority-class floor (`Rejected`) | 52.88% | 0 | 104 |
| Base `Qwen2.5-Coder-1.5B-Instruct` | **59.62%** (62/104) | 11 | 93 |
| Base + adapter | **95.19%** (99/104) | 50 | 54 |
| Delta | **+35.58 pp** | | |

Confusion, base: 9 of 49 true-`Accepted` correct, 53 of 55 true-`Rejected`
correct — the base model is nearly a `Rejected`-only predictor and clears the
majority floor by 6.7 points mostly by accident of which way it leans. Adapter:
47/49 and 52/55, balanced. Full per-example predictions are archived at
`artifacts/adapter_vs_base_holdout.json`.

### 2. Corpus acceptance rate vs. the archived Legion-Bench baseline

This is the comparison `legion_observability::training::build_training_eval_comparison`
produces, and it is a property of the *corpus*, not of any model:

| | |
|---|---|
| Baseline | `legion-bench-v0`, fingerprint `bench-suite-v1:bd2aa3a7d84d9485` |
| Baseline accepted rate | 6666 bp |
| Dataset accepted rate | 4343 bp |
| Delta | **−2323 bp**, `regressed = true` |

`regressed = true` here is expected and is **not** a training failure. The
archived baseline rate of 6666 bp is 2-of-3 from the seven-trace checked-in
fixture; the expanded batch's label rule rejects more often than that. The number
says the two batches have different label distributions, which they do by
construction.

### 3. The Legion-Bench suite report itself

```
cargo run -p xtask -- legion-bench --mode recorded
legion bench: total=20 passed=20 failed=0 regressed=0 strict=true mode=recorded_offline provider=recorded:gpt-5.5 fingerprint=bench-suite-v1:bd2aa3a7d84d9485

cargo run -p xtask -- verify-legion-bench
legion bench verify: total=20 passed=20 failed=0 regressed=0 skipped=0 strict=true mode=recorded_offline provider=recorded:gpt-5.5 fingerprint=bench-suite-v1:bd2aa3a7d84d9485
```

Archived at `artifacts/legion_bench_report.toml`. Note that this report's own
`scoring_mode` is `synthetic_budget_arithmetic`: recorded mode does not open
fixture repos or run agents, so it is a suite-integrity gate, not a model
measurement. The report is pinned here because the stop condition requires the
training run to be paired with a baseline comparison, and this is the baseline
artifact the backlog row names.

A correction to the previous version of this file: it recorded the suite
fingerprint as `bench-suite-v1:fb767be844a28833`. The suite's current fingerprint
is `bench-suite-v1:bd2aa3a7d84d9485`, which is also what
`evals/training-candidates/eval_baseline.json` carries. The old value was stale.

## A measurement bug found and fixed on the way

The first eval run scored the adapter at **47.1%** — below the 52.9%
majority-class floor — with the adapter predicting `Accepted` for all 104
holdout examples. Training loss had fallen to 0.12, so the loss curve gave no
hint anything was wrong.

The cause was a tokenizer boundary. Training conditioned on
`instruction + " "` and taught the completion `"Accepted"`; the eval scored
`instruction` followed by `" Accepted"`. On Qwen's BPE those are different token
sequences:

```
train: [..., 25 (':'), 220 (' '), 65906 ('Accepted'), 151645 ('<|im_end|>')]
eval : [..., 25 (':'),            63289 (' Accepted')]
```

The eval was scoring two continuations the adapter had never been trained on.
`render_prompt` and `label_token_ids` are now shared by both scripts, and
`test_the_eval_prompt_and_label_reproduce_the_training_tokens` fails if they
diverge again — verified by restoring the original construction and watching it
fail. Both eval numbers in this document come from the corrected run; the 47.1%
figure is recorded here only because a failed measurement that looks like a
failed training run is worth leaving a marker for.

## What this does and does not license

**It licenses:**

- The claim that the consent-gated pipeline reaches a GPU: consented metadata →
  export → QLoRA → adapter → held-out eval → archived baseline comparison, with
  a refusal at every boundary and a test for each refusal.
- The claim that the run is reproducible from checked-in inputs: the corpus is
  regenerated from a seed by an xtask command, the training command is fixed, and
  the artifacts carry fingerprints and digests.
- The claim that on this corpus, LoRA fine-tuning of `Qwen2.5-Coder-1.5B-Instruct`
  moves held-out accuracy from 59.6% to 95.2%.

**It does not license:**

- Any claim about `qwen2.5-coder:14b`, the model the Legion-Bench baseline was
  measured with. The adapter was trained on a different, thirty-times-smaller
  base and has never been attached to the 14B. The two cannot be composed.
- Any claim that Legion-Bench scores would improve. Legion-Bench measures an
  agent loop on code tasks; this adapter classifies proposal metadata. They do
  not share a task, a prompt format, or an output space.
- Any claim about reviewer behaviour. The corpus is synthetic fixture data with a
  stated labelling rule; 95.2% means the adapter learned that rule, and the rule
  was written in this repository.
- Any claim that this is a production training pipeline. It is a proven path with
  a fixture corpus at the input end.

## Verification commands, all run

```sh
cargo run -p xtask -- legion-bench --mode recorded    # 20/20 passed
cargo run -p xtask -- verify-legion-bench             # 20/20 passed
cargo run -p xtask -- training-corpus --expand 1200 --out target/training-flywheel
cargo test -p xtask --lib training_corpus             # 13 passed
python -m unittest training.test_qlora_train          # 15 passed
ls plans/evidence/training-flywheel/
```

## Archived artifacts

| File | What it is |
|---|---|
| `artifacts/export_manifest.json` | Consent-gated export: counts, fingerprints, retained consent states, baseline comparison |
| `artifacts/adapter_training_manifest.json` | The real training run: config, provenance, full 200-step loss curve, VRAM, wall clock |
| `artifacts/adapter_config.json` | PEFT LoRA config as saved with the adapter |
| `artifacts/adapter_vs_base_holdout.json` | Base vs adapter on the withheld split, with all 104 per-example predictions |
| `artifacts/legion_bench_report.toml` | The recorded Legion-Bench report the run is paired with |
