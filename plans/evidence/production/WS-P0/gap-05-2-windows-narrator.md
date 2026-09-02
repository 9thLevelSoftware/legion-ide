# GAP-05.2 — Windows Narrator transcript of a live window

**Date:** 2026-09-02  
**Wave:** 2 proof surface  
**Task:** GAP-05.2  
**SHA:** `2bbbfb392757a87a8400bec498ae703629db0b1a` (`origin/main` at capture)  
**OS:** Microsoft Windows 11 Pro 10.0.26200 (x86_64)  
**AT:** Windows Narrator 10.0.26100.8972

## What this is

A live `eframe::run_native` window (`legion-desktop --smoke`, title
`Legion IDE Smoke`) was started on this machine. Windows Narrator was started,
the window was focused, Tab moved through the top chrome, and Narrator+Ctrl+X
copied each last-spoken phrase from Narrator itself. Speech Recap opened
(`SPEECH_RECAP_WINDOW=True`).

Raw capture: [`plans/evidence/accessibility/2026-09-02-windows-narrator-transcript.txt`](../../accessibility/2026-09-02-windows-narrator-transcript.txt)

Repeatable probe: `scripts/a11y-narrator-transcript.ps1` (requires a live
window, same as `scripts/a11y-uia-walk.ps1`).

Product utterances Narrator spoke:

- `Legion IDE Smoke region, Manual, button, current,`
- `Assist, button,`
- `Delegate, button,`
- `Legion Workflows, button,`
- `Command, button,`
- `Explorer drawer, button,`
- `Bottom panel drawer, button,`
- `TERMINAL, button,`
- `PROBLEMS (0), button,`

`Copied last phrase to clipboard` is Narrator confirming the copy command used
to harvest those phrases.

## What this is not

- Not a UIA tree dump (`a11y-uia-walk.ps1` / `UIA_WALK_OK`)
- Not NVDA
- Not VoiceOver or Orca (GAP-05.3 / GAP-05.4)
- Not a ledger promotion of PR-UI-001
- Not a 3-OS screen-reader bar

The capture also recorded Narrator reading the system volume HUD
(`N Volume level`) while CapsLock chords were sent. Those lines are
environmental, not Legion controls, and are omitted from the committed
product transcript.

## Verification

```text
cargo test -p legion-desktop --test accessibility gap05_2_windows_narrator_transcript_names_at_and_live_window
```

Ledger row statuses are unchanged.
