# AI Stack Review

Scope reviewed:
- `crates/legion-ai/src/lib.rs`
- `crates/legion-ai/src/classifier.rs`
- `crates/legion-ai/src/manifest.rs`
- `crates/legion-ai/src/redaction.rs`
- `crates/legion-ai/src/streaming.rs`
- `crates/legion-ai/src/telemetry.rs`
- `crates/legion-ai-providers/src/lib.rs`
- `crates/legion-ai-providers/src/capabilities.rs`

Verification:
- Ran `cargo check -p legion-ai -p legion-ai-providers` from `~/legion-ide`; it completed successfully.

Summary:
- Findings count: 12
- Severity breakdown: critical 0, high 4, medium 6, low 2

## `crates/legion-ai/src/lib.rs`

### Finding 1
- Category: bug
- Severity: medium
- Line numbers: 679-705
- Description: `ProviderRouter::route_completion` invokes the selected provider, but the successful route response reports `provider_id` and `model_label` from the original request without checking that the provider response came from the same provider/model. A misconfigured or malicious adapter could return a response for another backend while policy, audit labels, and UI metadata still claim the requested provider was used.
- Suggested fix direction: Validate `completion.provider == request.provider_id` and `completion.model == request.model_label` (or explicitly record both requested and actual provider/model) before returning a completed route. Refuse or error on mismatch.

### Finding 2
- Category: failure-point
- Severity: low
- Line numbers: 756-764
- Description: `allowed_route_decision()` hard-codes `schema_version: 1` while the containing route response uses `request.schema_version`. If the protocol schema version changes, nested route-decision metadata can be stale even when the top-level response preserves the caller's version.
- Suggested fix direction: Pass the request schema version into `allowed_route_decision(schema_version)` and use it consistently in the nested `AssistedAiRouteDecision`.

### Finding 3
- Category: failure-point
- Severity: medium
- Line numbers: 451-480, 512-522
- Description: The deterministic inline predictor can produce an `Available` inline-prediction result with empty ghost text when `max_prediction_bytes` is zero or too small for the generated prefix. The result still reports `line_count` as 1, so consumers may try to display or accept an empty suggestion as if it were useful.
- Suggested fix direction: Treat zero-length generated text as a refusal/no-suggestion state, or validate `max_prediction_bytes` before building an `Available` result. Add an explicit test for `max_prediction_bytes == 0`.

## `crates/legion-ai/src/classifier.rs`

No findings.

## `crates/legion-ai/src/manifest.rs`

No findings.

## `crates/legion-ai/src/redaction.rs`

### Finding 4
- Category: bug
- Severity: high
- Line numbers: 24-40, 51-56
- Description: The redaction pass replaces marker strings and token prefixes, but not the complete sensitive values. For example, API-key assignments and authorization headers can be transformed so that the field name or token prefix is redacted while the rest of the secret remains in `redacted_text`. This is especially risky because callers use the returned text as model-bound or telemetry-safe output.
- Suggested fix direction: Replace token-aware spans rather than marker substrings. Use regexes or scanner-provided byte ranges that cover the full assignment/header/token value, and add tests with realistic non-placeholder secrets to verify the entire value is removed.

### Finding 5
- Category: failure-point
- Severity: medium
- Line numbers: 21-40
- Description: Sensitive-marker detection lowercases the payload in `scan_payload_for_sensitive_markers`, but the subsequent replacement list is mostly case-sensitive. Mixed-case forms can be flagged as requiring redaction while not actually being removed from `redacted_text`.
- Suggested fix direction: Make redaction itself case-insensitive, or have the scanner return exact spans to replace. Ensure tests cover uppercase, lowercase, and mixed-case variants of API-key and authorization markers.

## `crates/legion-ai/src/streaming.rs`

### Finding 6
- Category: bug
- Severity: medium
- Line numbers: 36-61
- Description: `flush_code` drops fenced code blocks when `code_lines` is empty, both for complete and incomplete fences. An assistant response containing an intentionally empty code block, such as a placeholder fence with a language label, is silently omitted from the segment stream.
- Suggested fix direction: Emit a `MarkdownStreamSegment::CodeBlock` even when the code body is empty, preserving the language and `complete` flag. Add tests for empty complete and incomplete fences.

## `crates/legion-ai/src/telemetry.rs`

### Finding 7
- Category: failure-point
- Severity: low
- Line numbers: 23-24, 57, 69-78
- Description: Validation failures and spool-record construction errors are collapsed into `None`, the same value used when telemetry is blocked by policy or consent. That makes malformed lifecycle/result records indistinguishable from intentional consent gating and can hide integration regressions.
- Suggested fix direction: Consider returning `Result<Option<HostedTelemetrySpoolRecord>, _>` for the lower-level helper, or at least log/label validation failures separately from consent-policy blocks.

## `crates/legion-ai-providers/src/lib.rs`

### Finding 8
- Category: failure-point
- Severity: high
- Line numbers: 333-340, 1230-1241, 1267-1278, 2256-2259
- Description: All blocking HTTP transports create default `reqwest::blocking::Client` instances without request/connect timeouts. A hung provider endpoint or MCP HTTP server can block the calling thread indefinitely, which is dangerous for IDE/UI paths and for background agents that expect bounded provider failures.
- Suggested fix direction: Build shared clients with explicit connect and total request timeouts, surface timeout errors as provider/transport failures, and add tests using an injected transport or delayed server behavior.

### Finding 9
- Category: bug
- Severity: medium
- Line numbers: 958-972
- Description: `OpenAiCompatibleProvider::complete` serializes `max_tokens` and `temperature` directly inside `json!`, so absent options are sent as JSON `null`. Some OpenAI-compatible endpoints reject nullable optional fields rather than treating them as omitted.
- Suggested fix direction: Build the payload incrementally and only include `max_tokens` and `temperature` when the request options are `Some`.

### Finding 10
- Category: bug
- Severity: high
- Line numbers: 1230-1239, 1267-1275, 1309-1320, 1355-1365
- Description: The Anthropic transport sends whatever was configured as `ANTHROPIC_API_KEY` or `ANTHROPIC_AUTH_TOKEN` using a bearer authorization header. Anthropic API keys normally require the `x-api-key` header, while bearer authorization is only appropriate for auth-token style credentials. As written, the common `ANTHROPIC_API_KEY` path can fail even when a valid key is configured.
- Suggested fix direction: Track which credential source was used and set `x-api-key` for API keys, and a bearer authorization header only for auth tokens. Update tests to assert the header selected for each credential type.

### Finding 11
- Category: failure-point
- Severity: high
- Line numbers: 619-625
- Description: `OpenAiResponsesProvider::request_payload` defaults `openai.responses.store` to `true`, causing hosted Responses API calls to opt into provider-side response storage unless callers remember to override metadata. This is a risky default for an IDE AI stack that otherwise emphasizes metadata-only routing and explicit consent.
- Suggested fix direction: Default `store` to `false` and require an explicit opt-in metadata flag for provider-side storage. Document the retention behavior in provider setup guidance.

### Finding 12
- Category: failure-point
- Severity: medium
- Line numbers: 2172-2195, 2203-2216
- Description: `StdioMcpTransport::send_on_session` loops on `read_line` until it sees the expected response id, with no timeout, cancellation path, or maximum number of unrelated messages. A silent, wedged, or notification-only MCP server can hang the caller forever while the session mutex remains held.
- Suggested fix direction: Add a per-request timeout/deadline and return a transport error when it expires. Consider handling JSON-RPC error responses and notifications separately, and avoid holding the mutex across unbounded blocking reads.

## `crates/legion-ai-providers/src/capabilities.rs`

No findings.
