# Project Documentation Rules (Non-Obvious Only)

- Treat `plans/spikes/SPIKE-001A-result.md` and `plans/evidence/phase-0/*` as accepted Phase 0 evidence, but verify older architecture-review claims against current code before answering.
- The repository names many future subsystems, but semantic indexing, memory, agents, embeddings, and unrestricted plugin runtime are not active in Phase 0.
- "Native shell" currently means projection-only shell state plus a CLI proof; GPU renderer, native IME, clipboard, focus, and accessibility validation are follow-ups.
- `devil-index` is intentionally a placeholder, so current text latency evidence excludes index mutation on keystrokes.
- Documentation and tests use Windows 11 as the validated evidence platform; cross-platform parity is planned but not proven by the current artifacts.
- Dependency policy is split between `plans/dependency-policy.md` and hardcoded `xtask` constraints, so the markdown file alone is not the full rule set.

