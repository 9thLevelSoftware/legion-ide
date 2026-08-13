# SPIKE-WS08-T6: jj-lib exploration result

Status: Complete
Date: 2026-06-13T17:00:41Z
Scope: report only; no product commitment.

## Question
Can `jj-lib` serve as the operation-log / undo model for agent-edit insurance in Legion, so an AI-driven edit can be rolled back cleanly after a bad mutation?

## Evidence reviewed
- Legion master plan WS08.T6 entry: `plans/legion-production-master-plan-v0.1.md:338`
- Jujutsu operation log docs: `https://docs.jj-vcs.dev/latest/operation-log/`
- Jujutsu architecture docs: `https://docs.jj-vcs.dev/latest/technical/architecture/`
- Jujutsu roadmap docs: `https://docs.jj-vcs.dev/latest/roadmap/`
- Library index summary for `jj-lib`: `https://lib.rs/crates/jj-lib`

## Findings

### What jj-lib is good at
- Jujutsu records each repo-modifying operation in an operation log, with a snapshot-style view at the end of each operation.
- The model supports `jj undo`, `jj op revert`, and `jj op restore`, which is exactly the kind of safety rail an edit-orchestrating agent wants.
- The operation log also supports lock-free concurrency semantics, which is useful when multiple processes may touch the same repo.
- `jj-lib` is explicitly intended as the reusable library crate for GUI, TUI, and server tooling, not just the CLI.

### What makes this only a conditional fit
- The docs also note that Rust API consumers are backend-bound to whatever was compiled in. That is fine for a controlled local product path, but it is not a universal portability story.
- Jujutsu’s roadmap calls out a future RPC API specifically because Rust-API tools are otherwise tied to compiled backends.
- Legion already has proposal-mediated mutation and fail-closed save semantics, so adopting `jj-lib` would add a second recovery model. That is not automatically bad, but it needs a clean adapter boundary to avoid muddling product authority.
- The spike evidence is conceptual only; there is no build-time, dependency-policy, or workspace-gate evidence yet for bringing `jj-lib` into Legion.

## Assessment

Verdict: conditional go.

`jj-lib` looks like a credible foundation for agent-edit insurance if the goal is local repo recovery, undoable operations, and operation-history inspection. It is not yet a go for product integration because we have not proven dependency cost, backend-policy fit, or integration shape with Legion’s existing save/proposal boundaries.

## Go / no-go criteria

### Go if all of these are true
1. The integration stays behind a narrow adapter boundary, not spread across editor or workspace ownership.
2. The product use case is local operation recovery / undo insurance, not generic SCM replacement.
3. Dependency-policy and cargo-deny review accept the added crate set.
4. A concrete prototype can map Legion actions to a reversible op-log flow without breaking current proposal-mediated save semantics.
5. The adapter can expose deterministic replay / rollback evidence for AI edits in tests.

### No-go if any of these are true
1. `jj-lib` would require broad UI, workspace, or editor ownership changes.
2. The integration depends on backend portability that `jj-lib` does not currently provide.
3. The dependency footprint destabilizes workspace check / test / clippy gates.
4. The design duplicates existing proposal/save guarantees without adding a materially better recovery story.
5. The team would need a separate RPC or daemon just to make the first useful prototype work.

## Recommendation
Proceed only as a bounded follow-up prototype if and when Legion wants explicit operation-log rollback insurance for agent edits.

Do not commit to product adoption yet.

## Next step suggested
If this moves forward, the follow-up should be a tiny adapter spike with:
- one reversible edit path,
- one rollback test,
- one dependency-policy review,
- and one gate report showing whether `jj-lib` stays cheap enough to keep.
