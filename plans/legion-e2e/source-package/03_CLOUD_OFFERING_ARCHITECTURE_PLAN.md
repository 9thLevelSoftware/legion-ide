# 03 — Legion Cloud Offering Architecture and Provider Plan

Generated: 2026-06-01 16:24:53 EDT

## 0. Executive summary

Legion Cloud should not be sold as generic GPU hosting. It should be sold as dedicated worker lanes for Legion IDE.

The user’s local machine becomes the control room. Legion Cloud supplies disposable, preconfigured worker lanes for users with lesser hardware or users who want parallel automation.

Core offering:

- cloud worker lanes for AI model tasks.
- cloud validation lanes for deterministic build/test/lint/debug tasks.
- cloud repo cache for speed.
- cloud model pool for small specialist models.
- proposal/evidence return to local Legion.
- no direct mutation of user repo unless explicitly approved through local app policy.

Strategic positioning:

```text
Legion IDE is local-first.
Legion Cloud adds disposable, scoped, validated worker lanes.
Workers receive task packets, not unlimited repo authority.
Every cloud result returns as a proposal with evidence.
```

## 1. Verified provider research summary

Direct HTTP checks performed 2026-06-01.

### 1.1 RunPod

Source checked: `https://www.runpod.io/pricing`

Observed page metadata:

- GPU cloud pricing.
- Per-second/pay-as-you-go framing.
- Page states examples:
  - H100 80GB from about `$1.99/hr`.
  - RTX 4090 from about `$0.34/hr`.

Best use:

- cheapest flexible GPU experiments.
- GPU pods for model serving.
- serverless GPU endpoints for bursty inference.
- early MVP cloud worker lanes.

Risks:

- capacity variability.
- operational complexity if using community machines.
- less enterprise polish than hyperscalers.

Recommendation:

- Use RunPod for MVP GPU inference experiments and low-cost cloud specialist hosting.

### 1.2 Fly.io

Source checked: `https://fly.io/docs/about/pricing/`

Observed GPU pricing snippet:

- A10: `$0.75/hr` per GPU.
- L40S: `$0.70/hr` per GPU.
- A100 40G PCIe: `$1.25/hr` per GPU.
- A100 80G SXM: `$1.50/hr` per GPU.
- Billed by second when machine is running.

Best use:

- global control plane.
- small API services.
- edge-ish low-latency app backend.
- possibly GPU worker machines where available.

Risks:

- GPU availability and regional constraints.
- less mature for GPU fleets than specialist GPU clouds.

Recommendation:

- Good candidate for control plane and lightweight regional services.
- Consider GPU workers only after proving availability/cost fit.

### 1.3 Modal

Source checked: `https://modal.com/pricing`

Observed pricing snippets:

- H100: `$0.001097/sec`, about `$3.95/hr`.
- A100 80GB: `$0.000694/sec`, about `$2.50/hr`.
- A100 40GB: `$0.000583/sec`, about `$2.10/hr`.
- L40S: `$0.000542/sec`, about `$1.95/hr`.
- L4: `$0.000222/sec`, about `$0.80/hr`.
- CPU and memory billed separately.

Best use:

- serverless GPU/CPU jobs.
- batch validation.
- training jobs.
- elastic workers.
- simple Python-based ML infrastructure.

Risks:

- can be more expensive than raw GPU clouds.
- vendor lock-in to Modal’s execution model.

Recommendation:

- Strong candidate for prototype cloud validation/training pipelines and serverless GPU workers where developer speed matters.

### 1.4 Vast.ai

Source checked: `https://vast.ai/pricing`

Observed:

- Marketplace GPU cloud.
- Live platform rates.

Best use:

- cheapest possible GPU experiments.
- one-off training.
- capacity arbitrage.

Risks:

- reliability variability.
- security/compliance concerns.
- not ideal for handling private customer repos unless wrapped carefully.

Recommendation:

- Use for internal experiments/training only, not customer production cloud lanes initially.

### 1.5 Lambda Labs / Lambda AI

Source checked: `https://www.lambda.ai/pricing`

Observed snippets:

- H100 SXM 80GB around `$3.99/hr` for some instance shapes.
- A100 SXM 80GB around `$2.79/hr`.
- A100 40GB around `$1.99/hr`.
- cluster pricing also listed.

Best use:

- training jobs.
- reliable GPU instances.
- dedicated/cloud GPU when availability fits.

Risks:

- higher cost than RunPod/Vast.
- availability constraints.

Recommendation:

- Use for serious training and reliable internal GPU workloads.

## 2. Product packaging

### 2.1 Product tiers

#### Tier 0: Legion Local

Target:

- users with strong local hardware.
- privacy-focused users.
- open-source/dev users.

Includes:

- local deterministic IDE.
- local model endpoints.
- local worker lanes.
- optional cloud disabled.

Cloud usage:

- none by default.

#### Tier 1: Legion Cloud Lane

Target:

- users with weak hardware.
- laptop users.
- hobbyists.

Includes:

- one hosted worker lane.
- one validation lane.
- small model tier.
- limited repo cache.
- monthly lane-minute cap.

Positioning:

```text
Your laptop is the control room. We provide one disposable worker lane.
```

#### Tier 2: Legion Cloud Team

Target:

- serious solo developers.
- small teams.
- users who want parallel workflows.

Includes:

- 3–5 worker lanes.
- multiple validation lanes.
- small + medium model tier.
- stronger remote escalation route.
- larger repo cache.
- project policies.
- team audit history.

#### Tier 3: Legion Cloud Forge

Target:

- companies.
- power users.
- custom specialist models.

Includes:

- dedicated GPU slice or reserved capacity.
- custom specialist fine-tunes.
- private model endpoints.
- extended artifact retention.
- organization policies.
- SSO/VPC/private deployment options later.

## 3. What to sell

Do not sell:

- GPU hours.
- generic VMs.
- arbitrary cloud dev boxes.

Sell:

- worker lane minutes.
- validation lane minutes.
- specialist task attempts.
- parallel lane capacity.
- model tier.
- repo cache size.
- custom specialist training.

Suggested billing primitive:

```text
lane-minute = one isolated worker lane executing one task for one minute
```

Additional primitives:

- validation-minute.
- model-escalation call.
- repo-cache GB-month.
- trace-retention GB-month.
- custom model training job.

## 4. Cloud architecture overview

```text
Local Legion IDE
  ↓
Cloud Gateway API
  ↓
Auth / Project Policy / Cost Guardrails
  ↓
Task Packet Service
  ↓
Scheduler
  ↓                 ↓
Worker Lane Pool    Validation Lane Pool
  ↓                 ↓
Model Pool          Build/Test Sandbox Pool
  ↓                 ↓
Proposal Builder ← Evidence Store
  ↓
Local Legion IDE Proposal Queue
```

Core cloud services:

1. API Gateway.
2. Auth/tenant service.
3. Project policy service.
4. Task packet service.
5. Scheduler.
6. Worker lane manager.
7. Validation lane manager.
8. Model pool manager.
9. Sandbox/repo cache manager.
10. Evidence store.
11. Proposal store.
12. Billing/usage service.
13. Audit service.

## 5. Control plane architecture

### 5.1 Responsibilities

The control plane handles:

- auth.
- tenant/project registration.
- cloud policy.
- task submission.
- routing.
- scheduling.
- usage metering.
- result retrieval.
- cancellation.
- audit.

### 5.2 Suggested stack

MVP option:

- Rust API service or TypeScript/Fastify API.
- Postgres.
- Redis/Valkey for queues.
- Object storage for artifacts.
- Fly.io, Railway, Render, or AWS ECS for control plane.

Recommended MVP stack:

- Fly.io for control plane API if simplicity/global latency matters.
- Supabase/Neon/Postgres for database.
- Upstash/Redis for queues if acceptable; otherwise self-hosted Redis.
- Cloudflare R2 or S3 for artifact storage.

Reason:

- control plane does not need GPU.
- keep it cheap and reliable.
- keep GPU provider abstracted behind worker plane.

## 6. Worker plane architecture

### 6.1 Worker lane types

#### AI worker lane

Runs:

- model inference.
- structured output generation.
- patch proposal generation.

May use:

- local small model in cloud GPU pool.
- remote API model.
- larger escalation model.

#### Validation lane

Runs:

- patch application.
- cargo check/test/clippy.
- pnpm test/typecheck.
- pytest.
- linters.
- security scans.

Usually CPU-only.

#### Index lane

Runs:

- repo checkout.
- symbol indexing.
- tree-sitter parsing.
- embeddings.
- cache refresh.

Can be CPU or low-end GPU if embedding model needs it.

#### Training lane

Runs:

- fine-tuning.
- eval.
- quantization.
- model packaging.

GPU-backed, not customer interactive path.

### 6.2 Worker lane lifecycle

```text
Idle
  ↓
LeaseAcquired
  ↓
SandboxPrepared
  ↓
TaskPacketLoaded
  ↓
ModelInvocation / ValidationCommand
  ↓
EvidenceCaptured
  ↓
ProposalCreated
  ↓
Cleanup
  ↓
Idle or Terminated
```

### 6.3 Warm pools

To reduce cost and latency:

Reusable:

- container image.
- model weights.
- dependency cache.
- repo clone.
- build artifacts.
- language toolchains.

Disposable:

- task context.
- worktree/sandbox.
- prompt/session.
- temporary files.
- worker lease.

This preserves disposable worker semantics while avoiding model cold starts.

## 7. Sandbox and repo cache architecture

### 7.1 Repo access modes

Mode A: Scoped packet only

- Local Legion sends selected files/snippets.
- Cloud never clones full repo.
- Best privacy.
- Lower functionality.

Mode B: Cloud repo cache

- Cloud clones repo using user-approved token or uploaded bundle.
- Task packets select files from cached repo.
- Best performance.
- Higher privacy/security burden.

Mode C: Enterprise private deployment

- Cloud workers run in customer VPC/private environment.
- Later product tier.

### 7.2 Recommended MVP

Start with Mode A and optional Mode B.

For Mode A:

- easiest trust story.
- easier security.
- enough for small task packets.

For Mode B:

- support explicit opt-in.
- require secret scanning.
- require path allowlist/denylist.
- show upload/clone scope.

### 7.3 Sandbox implementation

Each task gets:

- ephemeral container.
- mounted task packet.
- mounted repo snapshot or selected files.
- no host secrets.
- non-root user.
- network disabled by default during validation unless policy allows.
- resource limits.
- timeout.

Container runtime options:

- Docker for MVP.
- Firecracker/microVM later for stronger isolation.
- Kubernetes jobs for scalable production.
- Modal jobs for fast serverless path.

## 8. Model pool architecture

### 8.1 Small specialist pool

Models:

- Qwen2.5-Coder-1.5B-Instruct.
- Qwen2.5-Coder-3B-Instruct.

Use for:

- docs.
- summaries.
- simple test generation.
- simple lint fixes.
- scoped compiler error patches.

### 8.2 Medium specialist pool

Models:

- Qwen2.5-Coder-7B-Instruct.
- DeepSeek-Coder-V2-Lite-Instruct if licensing/serving constraints fit.

Use for:

- harder debugging.
- reviewer tasks.
- cross-file reasoning.

### 8.3 Remote escalation pool

Models/providers:

- Kimi 2.6 via Fireworks/custom provider.
- OpenAI/Codex route where product/subscription/API integration supports it.
- Anthropic/OpenRouter optional.

Use for:

- planner/architect.
- high-risk reviewer.
- failed local attempts.
- large-context tasks.

### 8.4 Serving stack

MVP:

- llama.cpp server for GGUF models.
- vLLM for higher-throughput GPU serving later.
- Ollama optional for dev/local convenience.

Production cloud:

- vLLM for GPU model pool if batching matters.
- llama.cpp for cheap CPU/GPU small specialist lanes.
- TGI optional.

## 9. Provider recommendations by phase

### Phase Cloud-0: Internal experiments

Use:

- RunPod for cheap RTX 4090/H100 tests.
- Vast.ai for very cheap non-customer experiments.
- Lambda for more reliable training if needed.

Do:

- benchmark model serving.
- benchmark validation containers.
- measure cost per task.

### Phase Cloud-1: MVP hosted lanes

Use:

- Fly.io or small VPS for control plane.
- RunPod serverless/pods for GPU inference lanes.
- CPU containers on Fly.io/Hetzner/AWS ECS for validation lanes.
- R2/S3 for artifacts.
- Postgres for metadata.

Reason:

- lowest friction.
- cost-conscious.
- no need for full Kubernetes yet.

### Phase Cloud-2: Production beta

Use:

- Kubernetes or Nomad for worker orchestration.
- Dedicated node pools:
  - CPU validation pool.
  - GPU inference pool.
  - index/cache pool.
- Multi-provider GPU abstraction.

Recommended providers:

- Control plane: Fly.io, AWS ECS, or GCP Cloud Run.
- Validation: AWS ECS/Fargate, Fly Machines, Hetzner bare metal, or Kubernetes CPU nodes.
- GPU inference: RunPod, Lambda, CoreWeave if budget/enterprise, Fly GPU if available/fit.
- Storage: S3/R2.
- DB: Postgres.

### Phase Cloud-3: Enterprise

Use:

- VPC deployment.
- customer-managed cloud.
- private model endpoints.
- SSO.
- audit retention.
- policy templates.

## 10. Cost control plan

### 10.1 Hard limits

Every task must have:

- max runtime.
- max retry count.
- max model tier.
- max tokens.
- max files uploaded.
- max artifact size.
- max validation time.

### 10.2 Queue controls

- per-user concurrency limit.
- per-project concurrency limit.
- per-org monthly budget.
- idle worker timeout.
- preemptible lane class for low-priority background tasks.

### 10.3 Cost estimate before cloud use

Before cloud task:

```text
Estimated cloud use:
  1 AI worker lane, small model, ~2 minutes
  1 validation lane, CPU, ~3 minutes
  estimated cost: $X or Y credits
Files to upload:
  crates/foo/src/lib.rs
  Cargo.toml snippet
```

### 10.4 Billing events

Record:

- worker lane start/end.
- validation lane start/end.
- model route.
- GPU type.
- tokens.
- bytes uploaded.
- artifacts stored.
- retries.
- escalations.

## 11. Security and privacy

### 11.1 Upload scope visibility

Before cloud upload, show:

- selected files.
- selected snippets.
- forbidden paths.
- secret scan result.
- repo cache mode.
- retention policy.

### 11.2 Forbidden by default

Do not upload:

- `.env`.
- SSH keys.
- API keys.
- credentials.
- production secrets.
- private certs.
- local config with secrets.
- payment/auth files unless explicitly approved.

### 11.3 Cloud worker permissions

Default:

- no arbitrary network during validation.
- no access to user credentials.
- no persistent shell.
- no direct repo mutation.
- no direct GitHub push.

### 11.4 Audit

Store:

- task packet hash.
- upload manifest.
- model route.
- validation commands.
- evidence hash.
- proposal diff hash.
- approval/rejection decision.

## 12. Cloud API sketch

### 12.1 Submit task

```http
POST /v1/projects/{project_id}/tasks
```

Body:

```json
{
  "idempotency_key": "...",
  "task_packet": { },
  "privacy_policy": { },
  "cost_cap": { },
  "route_preference": "prefer_local_cloud_small"
}
```

### 12.2 Get task status

```http
GET /v1/tasks/{task_id}
```

### 12.3 Stream events

```http
GET /v1/tasks/{task_id}/events
```

Server-sent events or websocket.

### 12.4 Cancel task

```http
POST /v1/tasks/{task_id}/cancel
```

### 12.5 Fetch proposal

```http
GET /v1/tasks/{task_id}/proposal
```

### 12.6 Fetch evidence

```http
GET /v1/evidence/{evidence_id}
```

## 13. Cloud MVP implementation sequence

### Cloud-1: Control plane skeleton

Tasks:

1. Create API service.
2. Add auth token/project key.
3. Add Postgres schema.
4. Add task submission.
5. Add task status.
6. Add audit event table.
7. Add artifact bucket.

Exit criteria:

- Local Legion can submit fake task and receive fake proposal.

### Cloud-2: CPU validation lane

Tasks:

1. Create validation container image templates.
2. Support Rust template first.
3. Upload task files.
4. Apply patch.
5. Run validation command.
6. Return logs/evidence.

Exit criteria:

- Cloud validates a Rust patch in an isolated container.

### Cloud-3: Small model worker lane

Tasks:

1. Start Qwen2.5-Coder-1.5B/3B model server.
2. Receive task packet.
3. Generate structured output.
4. Parse output.
5. Build proposal.
6. Run validation lane.

Exit criteria:

- Cloud docs/test/simple patch worker produces proposal.

### Cloud-4: Scheduler

Tasks:

1. Add queue.
2. Add worker leases.
3. Add timeouts.
4. Add retries.
5. Add cancellation.
6. Add usage metering.

Exit criteria:

- Multiple tasks schedule predictably and cost is recorded.

### Cloud-5: Repo cache

Tasks:

1. Add opt-in repo cache.
2. Add shallow clone support.
3. Add branch/commit pin.
4. Add dependency cache.
5. Add build cache.
6. Add cache invalidation.

Exit criteria:

- Cached repo task runs faster than upload-only task.

### Cloud-6: Production hardening

Tasks:

1. Secrets scanning.
2. Allowlist/denylist policy.
3. Tenant isolation.
4. Rate limits.
5. Billing guardrails.
6. Audit export.
7. Failure recovery.

Exit criteria:

- Beta users can safely run low-risk cloud lanes.

## 14. Suggested initial pricing experiments

These are product experiments, not final pricing.

### Free/Local

- Manual mode.
- local deterministic features.
- local worker config.
- no included cloud.

### Cloud Lane

- `$10–20/month`.
- 1 worker lane.
- small model tasks.
- limited lane-minutes.
- limited validation-minutes.

### Cloud Team

- `$30–80/month`.
- 3–5 lanes.
- medium model access.
- larger repo cache.
- stronger escalation budget.

### Cloud Forge

- `$150+/month` plus usage.
- dedicated capacity.
- custom specialist model.
- private endpoints.

## 15. Main cloud risks

1. Cost blowups.
   - Mitigate with hard quotas and lane caps.

2. Security concerns.
   - Mitigate with scoped packets, secret scanning, visible upload manifests.

3. Environment support burden.
   - Mitigate with templates, not arbitrary VMs.

4. Latency.
   - Mitigate with warm pools and local-first control.

5. GPU availability.
   - Mitigate with multi-provider abstraction.

6. Low-quality model outputs.
   - Mitigate with validation lanes and escalation.

## 16. Immediate cloud next actions

1. Build local/cloud protocol DTOs.
2. Prototype upload-only validation lane.
3. Benchmark RunPod small-model lane.
4. Benchmark Modal validation/training flow.
5. Build cost model from real task timings.
6. Add UI cloud scope preview.
7. Do not launch full repo cloud cache until secret scanning and policy exist.
