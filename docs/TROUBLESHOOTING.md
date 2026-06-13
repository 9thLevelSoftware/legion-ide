# Legion Troubleshooting and Diagnostics

Use this page when a smoke test, packaging run, release gate, or projected workflow fails.
The goal is to capture enough metadata to reproduce the failure without pasting raw private data into the issue.

## Fast triage checklist

1. Re-run the exact command from `docs/OPERATOR_RUNBOOK.md` or the issue template.
2. Save the full command line, working directory, and exit code.
3. Capture the generated evidence file for the phase you are working in.
4. Include the matching session-state and diagnostics-export files from `target/`.
5. If packaging failed, include the package directory contents and manifest.

## Common support artifacts

### GUI smoke artifacts

- Phase 6 session state: `target/gui-phase6-session.json`
- Phase 6 diagnostics export: `target/gui-phase6-diagnostics.md`
- Phase 7 session state: `target/gui-phase7-session.json`
- Phase 7 diagnostics export: `target/gui-phase7-diagnostics.md`
- Phase 8 session state: `target/gui-phase8-session.json`
- Phase 8 diagnostics export: `target/gui-phase8-diagnostics.md`

### Windows package artifacts

When `scripts/package-windows.ps1` runs without `-DryRun`, it should emit:

- package executable: `target/gui-phase6-package/legion-desktop.exe`
- package manifest: `target/gui-phase6-package/legion-desktop-package-manifest.txt`

If you override `-OutDir`, record the alternate directory in the report.

## What to include in a bug report

Use `.github/ISSUE_TEMPLATE/bug_report.md` and attach or reference:

- the exact command that failed;
- the OS and profile you were using;
- the evidence file path that should have been updated;
- the session-state and diagnostics-export files, if present;
- the package manifest, if the problem is packaging-related;
- the expected versus actual behavior in one or two sentences.

## When to escalate

Escalate when the failure involves one of these:

- missing or malformed diagnostics export output;
- package artifacts not being written to the expected directory;
- a release gate that passes locally but fails in CI;
- support evidence that would require a policy decision rather than a simple code or docs fix.
