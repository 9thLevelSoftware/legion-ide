# Cloud Lane HTTP JSON Transport Contract — 2026-06-03

## Scope

This document captures the exact DTOs, endpoint paths, headers, and policy checks implemented for the production HTTP JSON transport in `crates/legion-remote`.

## Configuration

- `HttpLegionCloudLaneTransportConfig`
  - `base_url`: scheme + host + optional port (e.g. `http://127.0.0.1:8080`). No trailing slash required.
  - `timeout`: `Duration` for the blocking reqwest client.
  - `client_identity_label`: display-safe client name sent as `X-Legion-Client-Identity`.
  - `auth_token`: optional `(label, value)` tuple. When present, the transport sends `Authorization: {label} {value}` (e.g. `Authorization: Bearer <token>`). The `Debug` impl redacts the value to `<redacted>`.

## Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/v1/cloud/tasks` | `submit_task` |
| `GET`  | `/v1/cloud/tasks/{id}/events` | `stream_task_events` |
| `POST` | `/v1/cloud/tasks/{id}/cancel` | `cancel_task` |
| `GET`  | `/v1/cloud/tasks/{id}/proposal` | `fetch_task_proposal` |
| `GET`  | `/v1/cloud/tasks/{id}/evidence` | `fetch_task_evidence` |

- `{id}` is the `LegionCloudLaneTaskId.0` string.
- All requests include `Content-Type: application/json`.
- `X-Legion-Client-Identity` is always present when the label is non-empty.
- `Authorization` is present only when `auth_token` is `Some`.

## Error Mapping

- HTTP 4xx/5xx responses are mapped to `RemoteRuntimeError::HttpResponse { status, reason }` where `reason` is the raw response body (not logged by the transport itself).
- Network/connection errors are mapped to `RemoteRuntimeError::Transport { reason }`.
- Serde errors are mapped to `RemoteRuntimeError::Serialization { reason }`.

## Policy Checks (Client-side, Before Network)

The `LegionCloudLaneClient` performs the following validations before calling the transport, proven by integration tests:

1. `ensure_enabled` — if `runtime_enabled == false`, returns `RemoteRuntimeError::RuntimeDisabled`.
2. `validate_cloud_task_request_limits` —
   - If `budget.estimated_cost_cents > config.max_cost_cents`, returns `RemoteRuntimeError::LimitExceeded` with reason `"cloud lane estimated cost exceeds configured cost cap"`.
   - If `upload_manifest.total_upload_bytes > config.max_upload_bytes`, returns `RemoteRuntimeError::LimitExceeded` with reason `"cloud lane upload bytes exceed configured upload cap"`.
3. `validate_legion_cloud_lane_task_request` (protocol-level) —
   - Rejects nil `causality_id` with `InvalidOperation`.
   - Rejects `upload_manifest.contains_forbidden_material == true` with `InvalidOperation` containing `"cloud upload manifest contains forbidden material"`.
   - Rejects empty `upload_manifest.allowed_files` with `InvalidOperation`.

## Security

- No raw request/response bodies are logged by the transport.
- The `Debug` impl for `HttpLegionCloudLaneTransportConfig` prints `auth_token: Some(("<label>", "<redacted>"))`.
- The `Authorization` header value is never printed in test assertions or logs.

## Test Evidence

Integration tests live in `crates/legion-remote/tests/cloud_lane_http_transport.rs` and use a local `std::net::TcpListener` mock server:

- `http_transport_submit_task_sends_headers_and_body` — verifies method, path, headers, and JSON body.
- `http_transport_disabled_policy_rejects_before_network` — verifies `RuntimeDisabled` without hitting the network.
- `http_transport_forbidden_upload_rejects_before_network` — verifies forbidden material rejection before network.
- `http_transport_cost_cap_rejects_before_network` — verifies cost cap rejection before network.
- `http_transport_stream_task_events` — verifies `GET /events` and response parsing.
- `http_transport_cancel_task` — verifies `POST /cancel` and response parsing.
- `http_transport_fetch_task_proposal` — verifies `GET /proposal` and response parsing.
- `http_transport_fetch_task_evidence` — verifies `GET /evidence` and response parsing.
- `http_transport_classifies_4xx_as_http_response_error` — verifies `HttpResponse` for 404.
- `http_transport_classifies_5xx_as_http_response_error` — verifies `HttpResponse` for 503.
- `http_transport_config_debug_redacts_auth_token` — verifies `Debug` redaction.

## Validation Results

- `cargo test -p legion-remote --test cloud_lane_http_transport -- --nocapture` → **11 passed, 0 failed**
- `cargo test -p legion-remote --all-targets` → **24 passed, 0 failed** (13 unit + 11 integration)
- `cargo run -p xtask -- check-deps` → **passed**
