# B20 — DAP dogfood green on all three platforms

Date: 2026-08-19
Task: P2.F3.T2
Supersedes the open questions in `B19-windows-adapter-diagnosis.md`

## Result

| Platform | Handshake | Launch | Step | Policy gate |
| --- | --- | --- | --- | --- |
| Ubuntu | pass | pass | pass | pass |
| macOS | pass | pass | pass | pass |
| Windows | pass | pass | pass | pass |

Windows, from the run that closed it:

```
system DAP launch dogfood: stopped reason=exception thread=1092 frames=5
system DAP launch dogfood: step reason=step frames=5
test result: ok. 1 passed; 0 failed
```

This morning the count was zero platforms.

## Two defects and one environment fault

**The launch deadlock (product).** The client sent `launch` and blocked on its
response before sending `configurationDone`. Per the DAP sequence an adapter
answers `launch` only once configuration is finished — it emits `initialized`,
waits for breakpoints and `configurationDone`, then responds. Both sides were
correct and each was waiting for the other. `launch` is now sent without
waiting, `initialized` is awaited, `configurationDone` follows, and both
responses are collected afterwards in whatever order they arrive.

**Discarded responses (product, introduced by that fix).** Waiting for
`initialized` necessarily reads the launch response on the way, and the frame
recorder kept events while dropping responses. The later wait then looked for a
frame that had already arrived. This broke Ubuntu — the one platform that
worked — and the frame trace caught it on its first run.

**A missing Python runtime (environment).** `liblldb.dll` links against
`python310.dll`, which the runner image does not carry. LLDB embeds CPython, so
`lldb-dap.exe` died during loader initialisation before reaching `main`: the
process was created and alive, stdout was closed, stderr was empty, and the
client saw `unexpected EOF in headers` — a framing error from a program that
never started. `setup-python` at 3.10 puts the library on PATH.

## What it cost, and why

Four rounds. The first three produced hypotheses that fit the evidence and were
wrong:

1. **Transport.** "This build wants `--connection` rather than stdio." Killed by
   the macOS `--help`: `--connection` is optional and stdio is the default.
2. **The deadlock.** Real, reproduced in-tree, and not the Windows fault.
3. **A probe that could not execute what it resolved.** `command -v lldb-dap`
   returned an extensionless path; `exit=127` read as "the adapter is mute" and
   meant "the probe named a file that is not there".

The symptom was identical for all of them: no frame, adapter alive, stderr
empty. That is what a deadlock looks like, and a silent refusal, and a program
that never started. Nothing distinguished them because nothing recorded the
conversation.

`LEGION_DAP_TRACE_FRAMES=1` records it, and it earned its keep twice on its
first run — finding the discarded-response regression, and showing that every
frame the adapter owed was arriving while the client timed out anyway.

The Windows answer came from walking the **transitive** dependency. The direct
dependents of `lldb-dap.exe` all resolved, including `liblldb.dll`; the absent
library was one that `liblldb.dll` itself needed.

## The fake adapter hid two real defects

It answered `launch` immediately and sent `initialized` during the handshake —
a sequence no real adapter follows. Every test against it passed while the
product hung against the real thing, twice: once for `initialized` at handshake
time (B9), once for the launch response here.

It now takes `--defer-launch-response` and `--initialized-after-launch`, both
taken from runner transcripts, and both orderings are pinned in-tree. Each new
test was checked against the pre-fix code and fails there with the same message
CI produced — three seconds locally instead of fifteen on a runner.

Modes travel as arguments rather than environment variables, because the first
version set a process-wide variable and took a sibling test down with it. Cargo
runs tests in a binary concurrently.

## Diagnostics added on the way

- stderr captured a line at a time rather than drained at EOF, so it is
  populated while the adapter is alive rather than only after it exits
- a bounded settle window on error paths, since the failure is raised before the
  reader thread is scheduled
- `<empty>` printed explicitly, because "the adapter said nothing" and "we
  failed to capture what it said" are different diagnoses
- the child's exit status attached to every frame error
- `dumpbin` dependency walking for both the adapter and its libraries, with a
  `PARSE FAILED` line, because a diagnostic that cannot parse is worth less than
  none — it looks like an answer

## Status

P2.F3.T2 done. The remaining DAP work in the backlog is elsewhere.
