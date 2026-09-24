# Bounded local process stdin prerequisite

Status: prerequisite specification for S2-03 Python formatter work. This is an
internal implementation contract. It does not claim that a Python formatter,
Python toolchain settings, or a complete Python workflow is implemented or
accepted.

Date: 2026-09-08.

## Purpose and scope

The Python language server currently does not provide document formatting. A
secondary formatter therefore needs an app-owned, explicitly configured local
executable that receives the current in-memory buffer snapshot on stdin and
returns formatted text on stdout. The result will later be admitted as a
reviewable proposal through the existing app proposal authority.

This specification is limited to the process transport prerequisite:

- Extend the existing `ProcessRequest.stdin` input in
  `legion-platform::BoundedProcessRequest` without changing the public
  `BoundedProcessRequest::new` signature.
- Keep source bytes in memory. Do not spool source text to a temporary file,
  invoke a shell wrapper, or use a shell command line.
- Keep the configured executable explicit and policy-approved. This transport
  does not add PATH discovery or an implicit formatter grant.
- Preserve the current bounded stdout/stderr, timeout, cancellation, process
  group/job cleanup, and fail-closed behavior.

The fixed service input limit is `MAX_BOUNDED_STDIN_BYTES = 8 * 1024 * 1024`
(8 MiB). This is a process-service safety limit and is separate from the
existing 5 MiB editor text snapshot cache budget. A caller may be unable to
format an 8 MiB input even when the editor can stream or otherwise display it;
that outcome is an explicit structured rejection.

## Existing contract being extended

`ProcessService::execute_bounded(&BoundedProcessRequest)` already promises a
finite timeout, live cancellation, and bounded stdout/stderr retention. The
request contains a nested `ProcessRequest`, output limits, timeout, and an
`Arc<AtomicBool>` cancellation flag. `ProcessRequest.stdin` is currently
rejected by both native bounded implementations even though the field exists.
The extension admits an optional stdin byte vector subject to the fixed 8 MiB
limit; `None` retains the current no-stdin behavior.

The public request constructor and all existing fields remain unchanged:

```rust
BoundedProcessRequest::new(
    ProcessRequest {
        command,
        args,
        cwd,
        env,
        stdin: Some(snapshot_bytes),
        timeout: None,
        cancelled: false,
    },
    max_stdout_bytes,
    max_stderr_bytes,
    timeout,
    cancellation,
)
```

The implementation must reject an input larger than
`MAX_BOUNDED_STDIN_BYTES` before spawning the child. No child, pipe, or policy
side effect may exist after this rejection.

## Native execution rules

The native supervisor owns the stdin writer and performs incremental writes in
chunks no larger than 64 KiB. There is no writer thread and therefore no
writer-thread join that can block shutdown. The same supervisor loop checks
cancellation, timeout, and stdout/stderr overflow on every iteration. Any
termination path first stops/reaps the child using the existing process-group
or Windows job-object cleanup and then joins the existing output reader
threads.

On Unix, the child stdin pipe is configured nonblocking with the existing
`nix`/`fcntl` mechanism. The supervisor writes the next bounded chunk when the
pipe accepts bytes and retains the returned offset for partial writes. A
would-block result yields back to the supervisor loop so cancellation and the
deadline remain observable. Once every input byte has been delivered, the
parent closes `ChildStdin`; this includes `Some(Vec::new())`, which must still
close stdin so a formatter waiting for EOF can finish.

On Windows, create the anonymous stdin pipe with `CreatePipe`. The child read
handle is the only stdin handle placed in the inherited-handle whitelist. Keep
the parent write handle non-inheritable. Before `CreateProcessW`, set the
parent write handle to `PIPE_NOWAIT` with `SetNamedPipeHandleState`; Microsoft
documents that anonymous pipe handles can be used with this API and that
`PIPE_NOWAIT` makes synchronous `ReadFile`/`WriteFile` return immediately when
the operation cannot proceed. This is synchronous polling, not overlapped
asynchronous I/O. The supervisor calls `WriteFile`, advances by the returned
partial byte count, and returns to its cancellation/timeout/overflow checks
when no progress is available. The writer handle is closed after all bytes are
delivered, including empty `Some` input.

References:

- [SetNamedPipeHandleState](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-setnamedpipehandlestate)
- [CreatePipe](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createpipe)

The current Windows bounded output pipe/job setup, inherited-handle whitelist,
and `TerminateProcess`/job cleanup remain authoritative. Windows setup must
also account for the third (stdin) pipe: if creation, inheritance-list setup,
process creation, job creation, job configuration, or handle assignment fails,
all stdin/stdout/stderr handles and attribute-list storage must be closed or
released through the existing cleanup path. Setup failure after the child is
created must kill and reap the child before returning a structured error. An
unexpected write failure or early pipe close must never be reported as
successful formatter output; clean up first, then return the structured
platform error.

## Failure and result semantics

- `None` stdin behaves exactly as today.
- `Some` input over 8 MiB is rejected before spawn.
- Empty `Some(Vec::new())` is valid and means immediate EOF after spawn.
- Cancellation returns `PlatformError::Cancelled` after cleanup.
- Deadline expiry returns `PlatformError::Timeout` after cleanup.
- Output overflow retains the existing `ProcessOutputLimit` result and cleanup.
- Invalid UTF-8, spawn failure, pipe setup failure, write failure, early close,
  and cleanup failure remain structured errors; none can produce a successful
  `ProcessResult`.
- If the child exits normally while `input_offset < input.len()`, the result is
  still a structured write/pipe failure even when the exit code is zero. A
  zero exit code cannot turn undelivered stdin into successful formatter input.
- A successful result contains the child exit code and bounded UTF-8 stdout and
  stderr. The app must additionally require the expected successful exit code
  before treating stdout as formatter output.

## Acceptance tests for this prerequisite

Add focused tests to `crates/legion-platform/tests/bounded_process.rs` and
retain all existing bounded process tests:

1. A real test child reads stdin and echoes exact Unicode bytes to stdout. The
   input must exceed the pipe capacity. The child emits simultaneous bounded
   stderr/output so neither direction can deadlock; the echo result alone is
   insufficient to prove nonblocking input writes.
2. `Some(Vec::new())` reaches the child as EOF and returns successfully.
3. A real child signals that it started and then deliberately does not read
   stdin. Supply input larger than pipe capacity, cancel while the parent is
   back-pressured, and assert a bounded `Err(PlatformError::Cancelled { .. })`
   result plus child cleanup.
4. Repeat the started/non-reading, over-pipe-capacity case with no cancellation
   and assert a bounded `Err(PlatformError::Timeout { .. })` result plus child
   cleanup.
5. A child exits before consuming all input; the caller receives a structured
   write/pipe error, never `Ok`, including when the child exit code is zero.
6. Input over `MAX_BOUNDED_STDIN_BYTES` is rejected before spawn; the child
   readiness marker must not appear.
7. Existing descendant cleanup, stdout/stderr overflow, prelaunch
   cancellation, timeout, invalid UTF-8, and finite-timeout tests remain green.

These are real host process tests. Linux and macOS behavior is assessed only
where the corresponding host execution is available; an unavailable host
implementation is recorded as blocked/unassessed, never passed by substitution
or hidden skip.

## Implementation boundary and handoff

Implementation is confined to `crates/legion-platform/src/lib.rs` and
`crates/legion-platform/tests/bounded_process.rs`, with an optional narrow
platform helper module if the existing file becomes unwieldy. No new
dependency is required.

After this prerequisite is implemented and verified, app-owned Python
formatter orchestration may use the bounded service with a captured buffer
snapshot, operation cancellation, and snapshot-staleness checks. Formatter
configuration, capability approval, stdout-to-proposal conversion, and native
Python acceptance remain separate work and are not completed by this document.
