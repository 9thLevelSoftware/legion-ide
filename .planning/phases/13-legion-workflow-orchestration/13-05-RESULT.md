# 13-05: App Workflow Composition (Wave 5)

## Outcome
Complete

## Tasks Performed
1. Added app-owned Legion workflow session storage, seeding, projection, execution, verification, sign-off, conflict-resolution, and approval metadata APIs on `AppComposition`.
2. Routed local workflow workers through existing delegated-task sandbox/proposal metadata boundaries and provider-backed workers through assisted-AI route metadata without provider invocation.
3. Added app-owned merge-readiness routing that blocks for dirty workspace state, stale approval metadata, unresolved conflicts, missing verification, missing sign-off, missing audit, missing rollback, or absent approval.
4. Appended metadata-only Legion workflow tracker records and proposed consent-gated memory outcome candidates without retaining raw payloads.
5. Added app integration tests covering session-not-found, local proposal metadata, provider-route metadata, same-target conflicts, dirty workspace blockers, missing verification, missing sign-off, and approved merge-ready state without file mutation.

## Files Changed
- `crates/devil-app/src/lib.rs`
- `crates/devil-app/tests/legion_workflow_integration.rs`

## Verifications Passed
- `rg -q "LegionWorkflow" crates/devil-app/src/lib.rs`: passed
- `rg -q "execute_legion_workflow" crates/devil-app/src/lib.rs`: passed
- `cargo test -p devil-app --test legion_workflow_integration -- --nocapture`: passed, 8 passed
- `cargo check -p devil-app --all-targets`: passed

## Decisions
- Local workflow workers reuse the existing delegated-task sandbox/proposal-output path and normalize the output into workflow-scoped proposal metadata. The app does not apply the proposal.
- Provider-backed workers emit `ProviderRouteRequired` metadata with `provider_route.not_invoked`; no provider call is made in this app workflow method.
- Dirty workspace state is detected from open editor buffers and represented as merge-approval blocker metadata.
- Memory outcome candidates are proposed with `MemoryConsentState::NotGranted`, proving metadata-only shape without retaining workflow output.

## Issues
- The first app integration test run failed because test delegated plan ids used `:` and the existing sandbox path embeds plan ids in Windows directory names. Test ids were changed to Windows-safe labels; production sandbox behavior was not changed in this app-only wave.
- The successful test run left one prunable Git worktree metadata entry under `crates/devil-app/target`; `git worktree prune` removed it before continuing.
