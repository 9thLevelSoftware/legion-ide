# LSP & Remote Review

Scope reviewed:
- `crates/legion-lsp/src/lib.rs`
- `crates/legion-lsp/src/bin/mock_lsp_server.rs`
- `crates/legion-remote/src/lib.rs`
- `crates/legion-remote-transport/src/lib.rs`

Verification performed:
- `cargo check -p legion-lsp -p legion-remote -p legion-remote-transport --all-targets` passed.
- `cargo clippy -p legion-lsp -p legion-remote -p legion-remote-transport --all-targets -- -D warnings` passed.
- `cargo test -p legion-lsp -p legion-remote -p legion-remote-transport --all-targets` passed.

Summary:
- Findings: 15
- Severity breakdown: critical 0, high 4, medium 8, low 3
- Stub markers: no `TODO`, `FIXME`, `HACK`, `todo!()`, or `unimplemented!()` were found in the reviewed production file bodies.

## `crates/legion-lsp/src/lib.rs`

### Finding 1
- Category: bug
- Severity: high
- Line numbers: 2435-2468
- Description: `LspStdioSession::read_until_correlated_response` discards every response whose JSON-RPC id is not the one currently being waited for. If callers send multiple requests and the server answers out of order, the earlier response can be read and skipped while waiting for a later request. The skipped response is never correlated, buffered, or removed from the pending table, so a later `read_response_for` for that request can block until EOF and the pending request remains stranded.
- Suggested fix direction: Correlate every response as it arrives and store completed responses by request id until the caller asks for them, or make the stdio session enforce one in-flight request at a time. Do not silently skip response frames with known pending ids.

### Finding 2
- Category: failure-point
- Severity: high
- Line numbers: 2366-2372, 2435-2447, 2607-2678
- Description: The stdio read path is fully blocking and does not apply the `LspPendingRequest.timeout_ms` budget. `resolve_timeout` exists on `LspClient`, but `read_response_for` and `request` never use it; a wedged or slow language server can hang the caller indefinitely while reading headers or payloads.
- Suggested fix direction: Add deadline-aware reads around `read_response_for`, preferably with an async/nonblocking read pump or a watchdog thread. On timeout, call `resolve_timeout`, emit a cancellation notification if appropriate, and return a timeout result instead of blocking forever.

### Finding 3
- Category: failure-point
- Severity: low
- Line numbers: 830-851
- Description: JSON-RPC ids are advanced with `saturating_add(1)`. Once `next_json_rpc_id` reaches `u64::MAX`, every subsequent request reuses the same id, overwriting entries in `pending_by_json_rpc_id` and `json_rpc_id_by_request_id` and corrupting request correlation.
- Suggested fix direction: Use `checked_add` and return an explicit exhaustion error, or wrap only after confirming there are no pending requests for the candidate id.

### Finding 4
- Category: bug
- Severity: low
- Line numbers: 1103-1110
- Description: Completion scores cast `index` to `u16` before scaling. For very large completion lists, indices beyond `u16::MAX` wrap before `saturating_mul`, so later completions can regain high scores instead of staying at the saturated floor.
- Suggested fix direction: Do the score calculation in `usize` or `u32`, clamp at zero, and only then cast to `u16`.

## `crates/legion-lsp/src/bin/mock_lsp_server.rs`

### Finding 5
- Category: failure-point
- Severity: medium
- Line numbers: 291-303
- Description: The mock server parses `Content-Length` and immediately allocates `vec![0u8; length]` with no maximum payload bound. A malformed or hostile test client can force excessive memory allocation and kill the mock process.
- Suggested fix direction: Reuse the production framer limit or add a small mock-specific maximum before allocating the payload buffer.

### Finding 6
- Category: bug
- Severity: low
- Line numbers: 100-107, 220-227
- Description: Unknown methods always produce an error response even when the incoming message is a JSON-RPC notification with no id. That serializes as `"id": null`, which violates the JSON-RPC/LSP notification rule that notifications must not receive responses. The same pattern can affect other handled methods if they arrive as notifications.
- Suggested fix direction: If `id` is `None`, process only true notifications such as `exit`/`$/cancelRequest` and otherwise ignore the message or log it without writing a response.

## `crates/legion-remote/src/lib.rs`

### Finding 7
- Category: bug
- Severity: high
- Line numbers: 791-817, 994-1000, 1205-1213, 1267-1275
- Description: The runtime deduplicates and audits using the envelope `operation_id`, but payload descriptors only check that their own operation ids are non-zero. Filesystem and process payloads can therefore carry an `operation_id` that differs from the envelope id; duplicate detection, causality, audit records, and mutation text can all refer to a different operation than the payload being applied.
- Suggested fix direction: Validate that every payload type with an embedded operation id matches `envelope.operation_id` before dispatching. Apply the same session/principal/correlation consistency checks at the envelope-to-payload boundary.

### Finding 8
- Category: bug
- Severity: high
- Line numbers: 1177-1192
- Description: `validate_mutation_gate` accepts any granted capability decision and any non-empty principal through `has_required_write_guards`. It does not require the capability to be `remote.fs.write`, does not require the principal to match the active session/envelope principal, and does not bind precondition correlation/causality to the envelope. A granted decision for a different capability could authorize remote filesystem mutation.
- Suggested fix direction: Reuse `validate_capability(&preconditions.capability_decision, "remote.fs.write")`, compare `preconditions.principal_id` with the session/envelope principal, and bind precondition correlation/causality to the operation envelope.

### Finding 9
- Category: bug
- Severity: medium
- Line numbers: 1089-1096
- Description: `create_file` stores content containing the proposal id (`remote-created-by-proposal:{id}`) but records the fingerprint of the constant string `remote-created-by-proposal`. The returned snapshot fingerprint does not describe the stored content, which can make later write preconditions stale or allow mismatched content to appear valid.
- Suggested fix direction: Compute the fingerprint from the actual `content` string after it is built, exactly as `seed_file` and `write_file` do.

### Finding 10
- Category: failure-point
- Severity: medium
- Line numbers: 1035-1064, 1089-1096, 1166-1173
- Description: Mutating operations preserve or reuse the caller's precondition snapshot id instead of issuing a new post-mutation snapshot id. Writes set `entry.snapshot_id = preconditions.snapshot_id`, creates copy the precondition snapshot, and renames carry the old entry unchanged to the destination. Snapshot ids therefore do not reliably identify unique remote states after mutation.
- Suggested fix direction: Allocate or derive a new snapshot id for each accepted mutation, update the entry to that post-mutation snapshot, and return that new snapshot in the outcome.

### Finding 11
- Category: failure-point
- Severity: medium
- Line numbers: 1670-1679
- Description: `common_headers` silently omits the `Authorization` or `X-Legion-Client-Identity` header when `HeaderValue::from_str` fails. A malformed configured token or identity label turns into an unauthenticated/misattributed request rather than a local configuration error.
- Suggested fix direction: Make header construction fallible and return `RemoteRuntimeError::InvalidOperation` or `Transport` when configured headers are invalid.

### Finding 12
- Category: failure-point
- Severity: medium
- Line numbers: 1749, 1765, 1775, 1785
- Description: Cloud task ids are interpolated directly into URL path segments. `validate_cloud_task_id` only rejects empty ids, so ids containing `/`, `?`, `#`, or percent-encoded path separators can alter the requested endpoint path when the HTTP transport is used.
- Suggested fix direction: Percent-encode task ids as path segments before formatting URLs, and tighten task-id validation to reject path/control characters.

## `crates/legion-remote-transport/src/lib.rs`

### Finding 13
- Category: failure-point
- Severity: medium
- Line numbers: 584-587, 731-735, 905-911
- Description: `replay_window_size` only bounds `accepted_sequences`; `seen_operations` and `inflight_operations` keep growing for every accepted operation until acked or process lifetime ends. Long-lived sessions can accumulate unbounded operation ids, and the reported replay window does not reflect the memory retained for duplicate detection.
- Suggested fix direction: Bound duplicate/replay tracking to the configured replay window, evict old operation ids when their sequence leaves the window, and keep inflight state separate from historical replay state.

### Finding 14
- Category: bug
- Severity: medium
- Line numbers: 761-779
- Description: `checkpoint` accepts a checkpoint for any seen `last_operation_id` without checking that `checkpoint.event_sequence` is at or behind the current `last_sequence` or consistent with the accepted replay window. A future or stale checkpoint can become the resume anchor and later produce misleading replay metadata.
- Suggested fix direction: Require checkpoint sequence/order to be within the accepted replay window and no greater than the current highest accepted sequence before saving `last_checkpoint`.

### Finding 15
- Category: failure-point
- Severity: low
- Line numbers: 279-282
- Description: `connect_inner` calculates `Instant::now() + Duration::from_millis(attempt.timeout_ms)`. Extremely large timeout values can overflow the `Instant` addition and panic before the connection path can return a structured `RemoteTransportCarrierError`.
- Suggested fix direction: Use `Instant::now().checked_add(...)` and reject unsupported timeout budgets with `InvalidPolicy` instead of panicking.
