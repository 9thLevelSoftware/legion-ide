# Legion IDE planning package

Generated: 2026-06-01 16:24:53 EDT

This directory contains the initial end-to-end planning package for pivoting `devil-ide` into `Legion IDE`.

Source inputs used:

- Existing repo inspection of `9thLevelSoftware/devil-ide`, cloned at `/tmp/devil-ide-inspect`.
- Current Phase 12 delegated-task runtime context.
- Current Phase 13 Legion workflow orchestration context.
- Discord-sent artifact downloaded to:
  `/Users/christopherwilloughby/Downloads/discord-legion-attachments/devils-den-bot-chat_1511102069061189735_compass_artifact_wf-0a626b05-1988-43e1-8592-3a91c5d2a31c_text_markdown.md`
- Direct web/API checks for Hugging Face model metadata and cloud provider pricing pages.

Plans in this package:

1. `01_FRONTEND_APP_ARCHITECTURE_PLAN.md`
   - End-to-end front-end architecture plan.
   - Dock/panel registry.
   - Manual/Assist/Delegate/Automate mode UX.
   - AI exclusion in Manual mode.
   - Kanban/fleet console UX.
   - Proposal/evidence review UX.

2. `02_BACKEND_APP_ARCHITECTURE_PLAN.md`
   - End-to-end back-end/local-runtime architecture plan.
   - Rust crate responsibilities.
   - Protocol DTOs.
   - LSP/DAP/index/git/sandbox/provider/agent layers.
   - Proposal-only mutation and authority boundaries.
   - Legion worker lifecycle.

3. `03_CLOUD_OFFERING_ARCHITECTURE_PLAN.md`
   - Hosted Legion Cloud architecture.
   - Provider suggestions.
   - Worker lanes, validation lanes, model pools, sandbox pools.
   - Cost controls and suggested pricing surfaces.
   - Deployment topology and security model.

4. `04_PRODUCT_IMPLEMENTATION_ROADMAP.md`
   - Detailed design/development/implementation roadmap based on what exists now.
   - Rename/pivot work.
   - Phases from deterministic IDE foundations through AI Assist/Delegate/Automate.
   - Exit criteria, tests, and risk triggers.

5. `05_MODEL_ACQUISITION_AND_TRAINING_PLAN.md`
   - Baby-stepped plan for exactly which models to acquire.
   - Local RTX 5070 12GB strategy.
   - Cloud training path.
   - Data collection, fine-tuning, evaluation, quantization, serving.
   - Tools required.

Recommended reading order:

1. `04_PRODUCT_IMPLEMENTATION_ROADMAP.md`
2. `01_FRONTEND_APP_ARCHITECTURE_PLAN.md`
3. `02_BACKEND_APP_ARCHITECTURE_PLAN.md`
4. `03_CLOUD_OFFERING_ARCHITECTURE_PLAN.md`
5. `05_MODEL_ACQUISITION_AND_TRAINING_PLAN.md`

Key strategic conclusion:

Legion should be positioned as a local-first, proposal-gated, evidence-driven AI IDE where software work becomes a visible assembly line. The differentiator is not merely “many agents.” It is narrow, disposable, capability-scoped worker lanes with deterministic validation and human/app-owned authority gates.
