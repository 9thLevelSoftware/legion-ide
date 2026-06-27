# Terminal Review

Scope: legion-terminal session management and OSC protocol handling.
Files reviewed:
- `crates/legion-terminal/src/lib.rs`
- `crates/legion-terminal/src/session.rs`
- `crates/legion-terminal/src/osc.rs`

Verification performed:
- `cargo check -p legion-terminal` passed
- `cargo test -p legion-terminal --lib -- --nocapture` passed: 14 tests passed
- Searched reviewed files for TODO/FIXME/HACK/todo!/unimplemented!: no production stubs found

## Findings by file

### `crates/legion-terminal/src/lib.rs`

#### Finding 1
- Category: bug
- Severity: high
- Line numbers: 786-792, 1267-1275
- Description: `TerminalRuntime::launch` validates `TerminalLaunchPolicyContract` fields including `cwd_policy` and `timeout_seconds`, but the runtime never enforces them when spawning the PTY. It always passes `cwd: None` to `PtyRequest`, and there is no runtime deadline/timeout stored with the session or passed to the platform boundary. A policy can therefore appear accepted in audit metadata while the actual process runs in the platform/default cwd and without the validated timeout constraint.
- Suggested fix direction: Carry an explicit, policy-approved cwd and deadline into `TerminalRuntimeLaunchRequest`/`RuntimeSession`, pass the cwd to `PtyRequest`, and enforce `timeout_seconds` either in the PTY service or in runtime polling/cleanup. If `cwd_policy` is only descriptive, rename/separate it from enforceable policy so callers cannot mistake validation for enforcement.

#### Finding 2
- Category: bug
- Severity: high
- Line numbers: 780, 797-802, 840, 965-987
- Description: `policy.output_byte_limit` is only checked to be nonzero and no greater than `config.max_output_bytes`; actual launch and poll redaction/truncation use `self.config.max_output_bytes`. A caller granted a smaller per-launch output limit can still receive/audit up to the global runtime maximum, because the accepted policy limit is not stored in `RuntimeSession` and is not used by `read_pty`, `redact_terminal_projection`, or `byte_count` calculation.
- Suggested fix direction: Store the accepted `policy.output_byte_limit` in `RuntimeSession` and use the per-session limit for launch projection, later `read_pty` calls, redaction, byte counts, and `TerminalOutputChunk.truncated`. Keep the global config as an upper bound only.

#### Finding 3
- Category: bug
- Severity: high
- Line numbers: 1301-1314
- Description: `redact_terminal_projection` performs literal substring replacement for secret markers and prefixes. This leaves credential suffixes visible for common formats: replacing only an authorization header prefix leaves the token value that follows, and replacing token prefixes or environment-variable names does not remove the rest of the secret value. Because terminal output is explicitly projected/retained as `redacted_payload`, this can leak credentials into terminal projections.
- Suggested fix direction: Replace the literal replacement list with token-aware redaction patterns that consume the full credential value through a safe delimiter, with case-insensitive handling for headers and environment assignments. Add tests for bearer-style authorization headers, environment assignments, GitHub/Slack/OpenAI token shapes, and mixed-case header names.

#### Finding 4
- Category: failure-point
- Severity: medium
- Line numbers: 346-355, 465
- Description: `DapClientRuntime::client()` and `client_mut()` panic with `expect` when no session has launched. `DapClientRuntime::step()` calls `self.client()` after only validating the provided session id, so a normal misuse path (`step` before `launch`) crashes the caller instead of returning `DapAdapterFixtureError::Denied`. This is a public runtime API and should not expose a panic for an invalid state transition.
- Suggested fix direction: Replace the panicking accessors with fallible helpers returning `DapAdapterFixtureError::Denied`, and have `step` return a structured error when no active client exists. Keep panicking helpers private to tests only if needed.

#### Finding 5
- Category: bug
- Severity: medium
- Line numbers: 947-1002, 1057-1080, 1083-1124, 1183-1201, 1283-1288
- Description: Audit sequencing is inconsistent across lifecycle operations. `input` and `resize` use the runtime-owned `RuntimeSession::next_sequence()`, but `poll_output`, `close`, and `kill` trust caller-supplied `event_sequence`, `correlation_id`, and `causality_id` from their request structs. This allows duplicate, regressing, or cross-session event identities in terminal audit records even though the runtime maintains a per-session sequence counter.
- Suggested fix direction: Generate audit event sequences consistently from `RuntimeSession::next_sequence()` for every lifecycle audit, including poll/close/kill. Treat caller correlation IDs as request metadata only after validating they belong to the active session, or explicitly document why externally supplied event identity is authoritative.

#### Finding 6
- Category: failure-point
- Severity: medium
- Line numbers: 1066-1072, 1092-1107, 1204-1229
- Description: Close/kill lifecycle operations clone the platform session id under the mutex, release the registry lock, perform the backend operation, and then remove the session in a separate lock. Because the runtime methods take `&self`, concurrent lifecycle calls can interleave: for example, an `input` can obtain the same platform id while `close` is in flight and then return a `Running` audit after the close has removed the session. Duplicate close/kill calls can also act on the same platform id and report confusing errors after backend side effects already occurred.
- Suggested fix direction: Track an in-session lifecycle state such as `Closing`/`Killing` under the registry lock before invoking the backend, or remove/mark the session atomically before irreversible close/terminate operations. Serialize lifecycle operations per session and make in-flight/closed sessions return a stable structured error.

### `crates/legion-terminal/src/session.rs`

#### Finding 7
- Category: bug
- Severity: medium
- Line numbers: 22-27
- Description: `TerminalSessionMetadata::apply_shell_projection` only updates `exit_code` when the new projection contains an exit code. If a new command boundary arrives (`PromptStart`, `CommandStart`, or `CommandOutput`) without a `CommandFinished` exit code, the metadata keeps the previous command's exit code. Consumers of `session_metadata` can therefore read a stale exit code for the currently running command.
- Suggested fix direction: Clear `exit_code` when applying a new command-start/output boundary, and only set it on `CommandFinished`/OSC 133 `D`. Add tests for a completed command followed by a new `CommandStart` projection.

### `crates/legion-terminal/src/osc.rs`

#### Finding 8
- Category: failure-point
- Severity: medium
- Line numbers: 38-70
- Description: `parse_terminal_shell_output` consumes from `ESC ]` to the end of the payload if no BEL or ST terminator is present. PTY reads are chunked, so an OSC sequence split across reads or a malformed sequence can cause all following bytes in that chunk to be dropped from `visible_output`. That can hide user-visible terminal output and also prevents recovery of the partial OSC metadata in the next chunk because parsing is stateless.
- Suggested fix direction: Preserve unterminated OSC bytes as visible output, or introduce a small stateful parser/buffer per session so split OSC sequences are completed on the next read. Add tests for `"prefix ESC]7;file://..."` without a terminator and for terminators arriving in a later chunk.

#### Finding 9
- Category: failure-point
- Severity: low
- Line numbers: 88-92
- Description: OSC 7 cwd parsing assumes a POSIX `file://host/path` shape, strips the host by taking everything after the first slash, and returns the path without URL decoding. This leaves common paths such as `My%20Project` encoded in metadata and can mangle Windows drive/UNC-style file URLs, despite the crate exposing a Windows feature path through `legion-platform/windows`.
- Suggested fix direction: Parse OSC 7 values with a URL-aware helper: validate/handle the host component, percent-decode path segments, and add platform-specific handling/tests for Windows drive letters and UNC paths.

## Summary

Total findings: 9

Severity breakdown:
- Critical: 0
- High: 3
- Medium: 5
- Low: 1
