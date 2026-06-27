# Core Protocol Review

Scope reviewed:
- crates/legion-protocol/src/lib.rs
- crates/legion-protocol/src/capability.rs
- crates/legion-protocol/src/manifest.rs
- crates/legion-protocol/src/plan.rs
- crates/legion-protocol/src/risk.rs
- crates/legion-protocol/src/scope.rs
- crates/legion-protocol/src/tools.rs

Verification run:
- `cargo test -p legion-protocol --quiet` passed: 21 + 1 + 113 + 2 + 1 tests, 0 failures.

## crates/legion-protocol/src/lib.rs

### Finding 1
- Category: failure-point
- Severity: low
- Line numbers: 194-197
- Description: `TimestampMillis::now()` silently maps `SystemTime` values before `UNIX_EPOCH` to `0` and casts `Duration::as_millis()` from `u128` to `u64`. A system clock anomaly before the epoch becomes indistinguishable from a real epoch timestamp, and a far-future/overflowing duration would wrap during the cast.
- Suggested fix direction: Return a `Result<TimestampMillis, _>` for clock errors, or saturate explicitly with a diagnostic path. Use `u64::try_from(d.as_millis()).unwrap_or(u64::MAX)` or equivalent checked conversion instead of a direct cast.

### Finding 2
- Category: failure-point
- Severity: medium
- Line numbers: 9483-9488
- Description: Delegated runtime protected-path enforcement checks `target.contains(protected)` on display/path labels. Substring matching is not path-aware: it can flag unrelated paths and can also miss intended protected paths when separators, canonicalization, case, symlinks, or glob-style protected patterns differ from the raw label representation.
- Suggested fix direction: Represent protected paths/patterns as canonical paths or compiled globs and compare with `Path`-aware boundary checks. Normalize targets before evaluation and keep a conservative fallback for non-path labels.

## crates/legion-protocol/src/capability.rs

### Finding 3
- Category: failure-point
- Severity: low
- Line numbers: 44-49
- Description: `AssistedAiCapabilityMatrix::has_explicit_declaration()` uses `String::is_empty()` instead of trimming. Whitespace-only `provider_id`, `provider_label`, `context_length_label`, or `cost_usage_label` therefore count as an explicit declaration, which can make malformed provider metadata appear usable to routing or UI surfaces.
- Suggested fix direction: Check `trim().is_empty()` for user/display/provider identifier fields and consider validating `schema_version`, availability, and redaction posture in a dedicated `validate()` method.

## crates/legion-protocol/src/manifest.rs

### Finding 4
- Category: failure-point
- Severity: medium
- Line numbers: 88-125
- Description: `ContextManifestAssembly::into_record()` emits a `ContextManifestRecord` without validating required contract fields or derived consistency. Empty `manifest_id`, zero `schema_version`, whitespace identifiers, inconsistent `omitted_item_count`, and stale/freshness flags that disagree with the flattened items can all be serialized as trusted manifest records.
- Suggested fix direction: Add a validation step or change the helper to return `Result<ContextManifestRecord, AssistedAiContractError>`. Validate manifest id, schema version, redaction hints, item/permission schema versions, and recompute or verify derived counts/risk flags.

## crates/legion-protocol/src/plan.rs

### Finding 5
- Category: bug
- Severity: medium
- Line numbers: 334-335, 420-447
- Description: The artifact contract documents that `sections` are "always ordered requirements → design → tasks", but `EditablePlanArtifact::validate()` only checks presence and duplicates. A plan with all three sections in the wrong order passes validation, allowing downstream UI/diff code to receive a contract-valid artifact that violates the documented ordering invariant.
- Suggested fix direction: During validation, compare each section's index to `section.kind.order()` or sort only in constructors and reject externally deserialized artifacts whose section order differs from the canonical order.

## crates/legion-protocol/src/risk.rs

### Finding 6
- Category: bug
- Severity: high
- Line numbers: 131-136
- Description: `RiskAssessment::is_allow()` returns true when `findings` is empty because `Iterator::all()` is vacuously true. If a risk engine fails to emit findings, omits all rules, or deserializes an empty assessment, approval gating can treat the proposal as allowed even though no deterministic rules actually ran.
- Suggested fix direction: Fail closed by requiring at least one finding and preferably one finding per `RiskRuleId::all()`. Return false when findings are empty or when any canonical rule id is missing.

## crates/legion-protocol/src/scope.rs

### Finding 7
- Category: failure-point
- Severity: medium
- Line numbers: 77-109
- Description: `DelegatedTaskScope::target_is_within_scope()` validates the workspace/module/file boundary but does not consult `forbidden_paths`. Because both methods live on the same scope object, callers can easily treat `target_is_within_scope()` as the complete authorization predicate and accidentally allow a target that is inside the selected module/repo but explicitly forbidden.
- Suggested fix direction: Add an all-in-one predicate such as `allows_target_path()` that checks workspace boundary, target kind, and forbidden paths together, or make `target_is_within_scope()` call `forbids_path()` before returning true.

## crates/legion-protocol/src/tools.rs

### Finding 8
- Category: failure-point
- Severity: medium
- Line numbers: 95-103, 127-140
- Description: `LegionToolCallFeedbackKind::UnknownTool` exists, but `LegionToolCallFeedback` requires `tool: LegionToolKind`. An actually unknown tool name cannot be represented without mapping it to a known enum variant, which loses the invalid attempted name and can mislead model feedback/retry handling.
- Suggested fix direction: Add an `attempted_tool_name: Option<String>` field or change the feedback tool field to an enum that can carry either `Known(LegionToolKind)` or `Unknown(String)`.

### Finding 9
- Category: bug
- Severity: medium
- Line numbers: 304-379
- Description: `validate_tool_schema_definition()` validates that required fields match `tool.kind.required_fields()`, but it does not verify that `tool.tool_name` equals `tool.kind.tool_name()` or that `description_label` equals the canonical label. A schema definition with kind `Read` and name `terminal-command` can pass validation, corrupting registry identity and routing.
- Suggested fix direction: Validate canonical name/label consistency against `tool.kind` and reject mismatches. Also consider checking that every property referenced by `required` has a schema compatible with the tool kind.

### Finding 10
- Category: failure-point
- Severity: low
- Line numbers: 208-218, 259-273
- Description: The JSON schemas for `read` and `edit-as-proposal` constrain `start_line` and `end_line` independently to be >= 1, but they do not enforce `start_line <= end_line`. A syntactically valid tool call can request an inverted range, pushing the error to later runtime handling.
- Suggested fix direction: Add runtime validation after JSON Schema validation or encode a stricter schema if the validator supports cross-field constraints. Return structured `InvalidArguments` feedback when an inverted range is provided.
