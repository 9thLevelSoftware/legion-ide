# B10 — What the Windows adapter is actually doing

**Date:** 2026-08-18
**Task:** P2.F3.T2
**Depends on:** `B9-system-adapter-dogfood.md`

## Where this started

Windows dogfood failed with one line and no way in:

```
initialize handshake failed against C:\Program Files\LLVM\bin\lldb-dap.exe:
DAP protocol error: malformed DAP frame: unexpected EOF in headers
```

That message is compatible with almost every hypothesis: the adapter died, the
adapter is a stub, the binary is wrong, the handshake is wrong, the framing is
wrong. Two rounds of diagnostic work were needed before it said anything
falsifiable.

## Round 1 — the capture that captured nothing

Stderr was piped and drained with `read_to_string`, which returns at EOF. EOF is
when the adapter exits, and every error that wants stderr is raised while it is
still running. The capture was therefore empty on precisely the paths it was
built for. Fixed by reading a line at a time into a bounded sink.

The next Windows run printed the same bare message. The drain was fixed and the
*timing* was not: the failure reaches `unexpected EOF in headers` in 20ms, so
the error was formatted before the reader thread had been scheduled. Fixed with
a bounded 250ms settle window on error paths.

An empty capture now prints `<empty>` rather than being omitted, because "the
adapter said nothing" and "we failed to capture what it said" are different
diagnoses and an absent clause cannot tell them apart.

## Round 2 — the answer, and it is not what was expected

```
malformed DAP frame: unexpected EOF in headers; adapter still running; adapter stderr: <empty>
```

Three facts, all now established rather than assumed:

1. **The adapter is alive.** `try_wait` reports no exit status. Every
   "the binary is broken / a DLL is missing / it crashed on startup" hypothesis
   is dead — those all produce an exited process, and on Windows a missing DLL
   would show as `0xc0000135`.
2. **It wrote nothing to stderr.** Not a usage message, not an error. It is not
   complaining; it is not a stub printing help text.
3. **Its stdout returned EOF while reading headers.** A live process whose
   stdout is closed.

A live process that closes stdout without a word is not a crash and not a
protocol disagreement. It is a process that does not intend to talk over stdio.

## What that points at, and what it does not

The natural reading is transport: this build of `lldb-dap.exe` is not serving
DAP on stdin/stdout for the way we invoke it. LLVM's `lldb-dap` grew a
`--connection` option, and a binary expecting a connection URI has no reason to
speak on stdout.

That is a **hypothesis, not a finding.** Nothing here has yet run the binary
directly to see what it does with no arguments, what `--version` reports, or
what `--help` lists. The three facts above are what the evidence supports; the
transport explanation is what it suggests.

Also unexplained: the same failure mode does not appear on Ubuntu, where the
handshake, launch, step and policy gate all now pass — the first fully green
dogfood run on any platform. macOS advances past the handshake and fails at
`launch`, which is a different problem and is not addressed here.

## Next step, now taken

`legion-dap-dogfood.yml` gains a report-only "Interrogate the resolved adapter"
step, run before the handshake so its output is present whether the handshake
passes or not. It records `--version`, `--help`, and what the binary does when
run with no arguments and stdin closed. If it wants a connection URI rather than
stdio, that is where it says so — or exits without a word, which is itself the
answer.

Two details, both there to keep a diagnostic from becoming a second problem:

- `continue-on-error` and `|| true` throughout. A step that can fail the run it
  is diagnosing is not a tool.
- The five-second cap is a backgrounded killer rather than `timeout(1)`, which
  is GNU coreutils and absent on the macOS runner, and on Windows names a
  different program entirely. Hanging one platform until the 60-minute job cap
  would be worse than no diagnostic at all.

### Round 3 — the probe was wrong before the adapter was

Its first run reported, on Windows:

```
--- resolved: /c/Program Files/LLVM/bin/lldb-dap ---
--- --version ---
--- --help ---
exit=127
--- stdout ---
--- stderr ---
```

Nothing from `--version`, nothing from `--help`, exit 127, empty streams. Read
quickly, that says the adapter is mute in every mode and the transport theory is
confirmed.

It says no such thing. Exit 127 is the shell reporting that it could not execute
the file, and the resolved path has no `.exe` — `command -v lldb-dap` on the
Windows runner returns an extensionless path. The Rust side spawns
`lldb-dap.exe` from that same directory and gets a **live process**. A probe that
contradicts a working spawn is the probe being wrong.

This is the same shape as the soft-skip that held this task open for two days: a
tool that fails for its own reasons and reports something that looks like a
finding about its target. The step now tries `.exe` first and prints an explicit
`PROBE BROKEN` line when it cannot execute what it resolved, so the next reader
cannot mistake one for the other.

The transport hypothesis is therefore still **untested**. Nothing has yet run
this binary successfully outside the handshake.

## Status

P2.F3.T2 stays in-progress. Ubuntu is green. Windows is now diagnosed rather
than merely failing, which is the difference between a blocked task and an
unreadable one. macOS `launch` is untouched.
