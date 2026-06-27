# Agent System Review

Scope: `crates/legion-agent` DAG scheduler, plan execution, scope enforcement, tools, evidence, merge readiness.

Reviewed files:
- `crates/legion-agent/src/lib.rs`
- `crates/legion-agent/src/comm.rs`
- `crates/legion-agent/src/dag.rs`
- `crates/legion-agent/src/evidence.rs`
- `crates/legion-agent/src/external.rs`
- `crates/legion-agent/src/merge_readiness.rs`
- `crates/legion-agent/src/plan.rs`
- `crates/legion-agent/src/scheduler.rs`
- `crates/legion-agent/src/scope.rs`
- `crates/legion-agent/src/tools.rs`

Verification run:
- `cargo test -p legion-agent --all-targets` passed: 47 tests passed across unit/integration targets.

Summary:
- Findings: 12
- Severity breakdown: critical 0, high 6, medium 4, low 2

## `crates/legion-agent/src/lib.rs`

### Finding 1
- Category: failure-point
- Severity: low
- Line numbers: 291-299
- Description: `DelegatedTaskSandboxOrchestrator::initialize` converts `sandbox_path` with `self.sandbox_path.to_str().unwrap()` before invoking `git worktree add`. On Unix/macOS, `PathBuf` can contain non-UTF-8 bytes; a path provided through the environment or filesystem can panic the process instead of returning an error.
- Suggested fix direction: Avoid `to_str().unwrap()` and pass `&self.sandbox_path` directly to `Command::arg`, or convert with `to_str().ok_or_else(...)` and return an `io::Error` with a clear message.

### Finding 2
- Category: failure-point
- Severity: high
- Line numbers: 361-388
- Description: The copy-based sandbox fallback walks the workspace and calls `std::fs::copy` for every non-directory entry. Symlinks are not detected or rejected; `std::fs::copy` follows symlinks, so a workspace symlink pointing outside the repository can copy external files into the delegated sandbox. This breaks isolation and can leak files that were never intended to be part of the delegated task scope.
- Suggested fix direction: Use `symlink_metadata` and explicitly handle symlinks. Either preserve safe in-repo symlinks after resolving and validating their targets, or skip/reject symlinks whose canonical target is outside the workspace root.

### Finding 3
- Category: bug
- Severity: high
- Line numbers: 422-469, 519-524
- Description: `validate_containment` normalizes a candidate path for the containment check, but `generate_proposal` later derives the proposal path from the original `input.target_path`. A path like `<sandbox>/src/../generated.txt` validates as contained, then can be emitted as `src/../generated.txt` in the proposal payload. Downstream apply code may interpret the `..` segments differently, and the emitted `CanonicalPath` is not actually canonical.
- Suggested fix direction: Return the normalized/canonicalized contained path from the containment helper and use that normalized path for `strip_prefix` and proposal payload construction. Reject proposal-relative paths containing `ParentDir`, root, prefix, or other non-normal components.

### Finding 4
- Category: bug
- Severity: high
- Line numbers: 514-565
- Description: `DelegatedTaskProposalGenerator::generate_proposal` is documented as comparing sandbox state with `HEAD`, but it never reads `HEAD`, never diffs the target, and always emits a `ProposalPayload::CreateFile` with no version/fingerprint preconditions. Existing-file modifications are represented as create-file proposals, and stale sandbox state can produce proposals with no concurrency guard.
- Suggested fix direction: Detect whether the target exists in the base checkout and generate the appropriate edit/patch payload for modifications versus creates. Populate file/workspace preconditions and expected fingerprints from the current sandbox/base state before returning proposal metadata.

### Finding 5
- Category: failure-point
- Severity: medium
- Line numbers: 997-1058
- Description: `record_proposal_output` appends proposal outputs, evidence records, and worker results every time it is called. The generated evidence/result ids are deterministic by worker id (`legion-evidence:{worker}` and `legion-result:{worker}`), so repeated calls for the same worker create duplicate ids and duplicate worker results. Other coordinator methods are explicitly idempotent, but this one can corrupt downstream result/evidence consumers.
- Suggested fix direction: Make proposal recording idempotent per worker/proposal id, or reject duplicate records with a structured error. If multiple outputs per worker are valid, include proposal id or sequence in evidence/result ids and validate uniqueness before insertion.

### Finding 6
- Category: failure-point
- Severity: medium
- Line numbers: 1207-1268
- Description: Provider route requests use a constant cancellation token (`Uuid::from_u128(13)`) and constant event sequence (`EventSequence(13)`) for every provider-backed worker. Multiple routes in the same workflow can therefore share cancellation/audit identifiers, making targeted cancellation and audit ordering ambiguous.
- Suggested fix direction: Derive cancellation token and event sequence from stable per-worker/session data supplied by the caller, or require them in the worker/session metadata so each provider route has unique audit/cancellation identity.

## `crates/legion-agent/src/comm.rs`

No findings identified.

## `crates/legion-agent/src/dag.rs`

### Finding 7
- Category: bug
- Severity: high
- Line numbers: 82-102
- Description: `workflow_dag_from_approved_plan` creates nodes for every plan section entry and then connects them with a simple linear `windows(2)` chain. It does not preserve task-graph dependencies, independent branches, blocked nodes, or explicit task ordering from the workflow metadata. A plan with parallel tasks becomes a serial chain, which can over-constrain execution and misrepresent the approved workflow DAG.
- Suggested fix direction: Build DAG nodes and edges from task graph dependency metadata when available, and only fall back to section-order presentation edges for non-executable requirement/design entries. Add tests for independent tasks and multi-edge dependency graphs.

## `crates/legion-agent/src/evidence.rs`

### Finding 8
- Category: failure-point
- Severity: medium
- Line numbers: 44-74, 147-156
- Description: `external_log_evidence_record` constructs evidence ids and command labels directly from `log_label`, and `record_external_log_evidence` pushes the record without running `validate_legion_evidence_record`. Empty labels, path-like labels, newlines, or other invalid characters can create invalid or colliding evidence ids; sensitive label text can also be included in the metadata summary before redaction policy validation.
- Suggested fix direction: Validate all evidence records before insertion, normalize or hash external labels before embedding them in ids, and keep raw labels out of summaries unless they have been policy-checked/redacted.

## `crates/legion-agent/src/external.rs`

### Finding 9
- Category: bug
- Severity: high
- Line numbers: 78-99
- Description: `validate_workspace_edit_conversion` requires `ProposalTargetCoverageKind::Complete`, but it never verifies that `payload.target_coverage.targets` actually match every path/file touched by `file_edits` and `file_operations`. An external producer can declare complete coverage for one target while including edits, creates, deletes, or renames for another target.
- Suggested fix direction: Derive the affected target set from `file_edits` and every `WorkspaceFileOperation`, compare it against `target_coverage.targets`, and reject missing, extra, or redacted targets when complete coverage is required.

### Finding 10
- Category: failure-point
- Severity: low
- Line numbers: 40-77
- Description: The `principal` parameter is explicitly ignored (`let _ = principal`), and the validation only checks capability equality with the payload. Empty or placeholder principals/capabilities can be accepted as long as they match each other, weakening audit attribution for externally supplied proposals.
- Suggested fix direction: Validate that `principal` and both capability fields are non-empty, well-formed identifiers. Keep the capability equality check, but also reject placeholder/empty identities before constructing the proposal envelope.

## `crates/legion-agent/src/merge_readiness.rs`

No findings identified.

## `crates/legion-agent/src/plan.rs`

### Finding 11
- Category: failure-point
- Severity: medium
- Line numbers: 43-58
- Description: `task_entries` converts each task node into a display string with id, targets, and verification requirements, but drops `depends_on`, `edge_count`, blocked-task metadata, and task state. The editable plan that users approve therefore omits the dependency information that later DAG/scheduler behavior depends on.
- Suggested fix direction: Include dependency and state metadata in task entries or add a dedicated dependency section. Validate that `TaskGraphArtifact::edge_count` and node `depends_on` data are represented in the editable plan before approval.

## `crates/legion-agent/src/scheduler.rs`

No findings identified.

## `crates/legion-agent/src/scope.rs`

### Finding 12
- Category: bug
- Severity: high
- Line numbers: 23-37
- Description: Scope enforcement delegates to `target_is_within_scope` and `forbids_path` using the raw `target_path`. Neither the target path nor forbidden path comparison is canonicalized/normalized here. Paths with `..` components or symlinks can pass a module/repo `starts_with` check while resolving outside the intended scope, and forbidden path checks can be bypassed with alternate spellings.
- Suggested fix direction: Canonicalize or securely normalize workspace root, scope target, forbidden paths, and the candidate path before comparison. Reject paths that cannot be canonicalized when mutation is involved, and resolve symlinks according to the selected workspace policy.

## `crates/legion-agent/src/tools.rs`

No findings identified.
