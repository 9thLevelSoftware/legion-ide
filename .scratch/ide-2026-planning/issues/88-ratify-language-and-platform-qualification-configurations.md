# 88: Ratify language and platform qualification configurations

Status: needs-info

**What to build:** Resolve the existing provisional matrix into named OS/tool/server/runner/debugger/project/AT versions and fixture identities for required Rust, TS, JS and Python categories without narrowing approved coverage.

**Blocked by:** [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 88.

**Traceability:** M4; S0-02/S2-01. **Verification target:** A16.

## Acceptance criteria

- [ ] Resolve the existing provisional matrix into named OS/tool/server/runner/debugger/project/AT versions and fixture identities for required Rust, TS, JS and Python categories without narrowing approved coverage.
- [ ] Every scope, configuration and ownership decision is reviewable and traceable to the approved specification and existing canonical records; unresolved prerequisites are explicitly recorded.
- [ ] Preserve existing requirement IDs and approved scope; planning evidence does not promote implementation or product acceptance.

## Verification

- Run `cargo run -p xtask -- docs-hygiene` for documentation changes; use the existing completion/register validators when canonical records change. Record commands and their actual scope.
- Resolve named configurations and ownership against existing records; missing tools, access or ratification stay explicit, without fabricated acceptance.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-09: Coordinator independent doc review PASS for bounded inventory only; all42 IDs/scope preserved and no acceptance promoted. Partial docs commit authorized after confirming current integration177caa5. Tool pins remain provisional, requiring current maintained-patch review before future provisioning. Coordinator requested actual Mac/Linux host availability; response pending. Status needs-info unchanged; no installations or tool approval inferred.

- 2026-10-09: Claimed isolated worktree ticket-088 from integration177caa5; preserved ticket004. Prepared [configuration inventory and exact proposed pins](../../../plans/completion/ide-2026-language-platform-configurations.md) covering all32 language cells/four OS-architecture groups and10 broader required cells without changing canonical IDs/acceptance. Initial Windows/Rust ratification retained; new tool/AT tuples, actual Mac/Linux hosts and TS/JS/Python fixture revisions/locks remain owner inputs. Read-only host/version inventory and official-source candidate research only; no installations, secrets or qualification runs. Status needs-info pending concrete decisions; independent review before commit.

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.

- 2026-10-09: Planned existing validators ran once: `cargo run -p xtask --target-dir D:/legion-ide-2026-tools/qualification-target -- docs-hygiene` passed; same command prefix with `verify-completion-register --root .` passed structure only (419 acceptance entries unassessed); `git diff --check` passed. No runtime tests, canonical register edits or acceptance evidence. Partial planning patch awaits independent review; no commit.
