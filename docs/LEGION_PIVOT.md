# Legion Pivot

Legion IDE is the user-facing product direction for this repository.

The product is a local-first, proposal-gated, evidence-driven AI IDE. It is not merely a chat panel bolted onto an editor. The intended product shape is:

- Manual: a fast deterministic IDE with no AI dependency.
- Assist: human-in-control AI assistance that can suggest, explain, and propose changes.
- Delegate: bounded disposable workers that execute scoped tasks and return proposals/evidence.
- Automate: multi-step Legion workflows with task graphs, validation gates, risk gates, and final human authority.
- Cloud Lane: opt-in hosted worker capacity with visible upload scope, cost/budget limits, and cancellation.
- Training Flywheel: opt-in, redacted trace collection and reproducible specialist model training/evaluation.

## Naming strategy

Use `Legion` for user-facing product language immediately:

- Legion IDE
- Legion Board
- Legion Fleet Console
- Legion Workflows
- Legion Cloud Lane
- Legion specialists

Keep internal `devil-*` crate names until a dedicated rename phase can update workspace manifests, dependency-policy checks, tests, CI, imports, package metadata, and migration docs in one controlled PR.

## Product promise

Legion makes software work visible and governable:

1. deterministic manual work remains the fastest path;
2. AI output is proposal-only unless explicitly approved;
3. workers are disposable and capability-scoped;
4. validation evidence is first-class;
5. cloud execution is opt-in and cost/policy bounded;
6. training data is opt-in, redacted, and deletable.

## Source plan package

The consolidated implementation plan and source package live under:

- `plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md`
- `plans/legion-e2e/source-package/`
