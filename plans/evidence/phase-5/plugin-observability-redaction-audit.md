# Phase 5 Plugin Observability Redaction Audit

Plugin audit uses `EventEnvelope` with metadata-only redaction and non-zero schema, correlation id, causality id, and event sequence. The helper `plugin_event_envelope` records plugin identity as metadata and does not store source text, prompts, provider payloads, secrets, or unbounded output.

Storage validation rejects plugin records containing `source_body`, `fn main`, `raw_source`, `raw_prompt`, `provider_response`, `secret`, `api_key`, or `unbounded_output` markers.

Validated by:
- `plugin_observability_event_is_metadata_only_and_validated`
- `plugin_storage_namespace_isolation_and_quota_fail_closed`
- `dto_contracts_plugin_host_call_and_storage_schemas_are_versioned_and_metadata_only`

Status: PASS in focused runs before final workspace gates.
