# Full product resume evidence — 2026-09-07

This directory records current component and integration evidence gathered while
resuming the full-product completion work. It is supporting evidence only. The
59-package and 419-requirement scope remains unchanged and unassessed as a
complete product matrix. Nothing here promotes readiness, packaged completion,
or native GUI qualification.

## Current implementation evidence

The current language path removes fabricated local rename tokens and empty
formatting, Organize Imports, and code-action proposals. Real server responses
are admitted through app-owned operations and proposal authority. Relevant
source pointers are:

- `crates/legion-app/src/language/local_proposals.rs`
- `crates/legion-app/src/language/lsp_reads.rs`
- `crates/legion-app/src/language/translate.rs`
- `crates/legion-app/src/language/translate/change_annotations.rs`
- `crates/legion-app/src/language/apply_edit_decision.rs`
- `crates/legion-app/src/language/server_apply_edits.rs`

The TypeScript path exercises an advertised server command with normalized
Windows paths, synchronization before the callback-owned proposal, and CAS
arbitration before mutation. Organize Imports and code actions retain their
advertised operation identity and review path. Workspace edits remain bounded,
proposal-mediated, and fingerprint/version checked.

Python annotated edits preserve bounded annotation metadata and reject strict
invalid indices. The retained Pyright archive is used through the explicit
local runtime configuration. Diagnostics retain file, code, range, severity,
and source metadata while projecting the message as metadata-only (`LSP error
diagnostic`); the native correction/clear/save scenario checks that privacy
boundary.

## Recorded checks

The earlier [resume-focused-r3.log](resume-focused-r3.log) count of 439 app unit tests, 36 protocol
unit tests, 115 DTO contract tests, 3 annotation DTO tests, 95 security unit
tests, and 23 security integration tests was recorded before placeholder
removal. The current [resume-language-r3.log](resume-language-r3.log) records 439 app tests and 16/16
language-tooling tests after removal and new command regression coverage. The
current `resume-app-integration-r1` records 438 app tests, app LSP composition
22, capability 3, and cross-file 6 passed; that earlier integration slice is
retained for its named checks. These are focused component checks; they are not
a workspace gate or product acceptance result.
The current [resume-native-r3.log](resume-native-r3.log) reports 4/4 Python scenarios and 6/6
TypeScript scenarios passed (real servers). [resume-desktop-r1.log](resume-desktop-r1.log) reports 243
desktop unit/rendering tests passed, and [resume-desktop-targeted-r2.log](resume-desktop-targeted-r2.log) reports
the targeted renderer API rerun 1/1 passed. [resume-clippy-r2.log](resume-clippy-r2.log) reports the
workspace all-targets clippy check with `-D warnings` exited 0. The standalone
native-r2 app result and a native harness run missing its environment are
historical context and are not current evidence headlines. The full workspace
test run was interrupted during the build at the user’s request; its durable
log is [resume-workspace-tests-r1-interrupted.log](resume-workspace-tests-r1-interrupted.log)
and it has no suite outcome.

The durable real-server evidence is retained in
`plans/evidence/full-product-resume-2026-09-07/resume-native-r3.log`. The
terminal checkpoint is [resume-terminal-checkpoint.log](resume-terminal-checkpoint.log)
(6/6, exit 0). The completed checkpoint gates are
[format](resume-format-checkpoint.log), [dependency policy](resume-check-deps-checkpoint.log),
[docs hygiene](resume-docs-checkpoint.log), [claim audit](resume-claims-checkpoint.log),
and [extract-before-modify](resume-extraction-checkpoint.log); each passed.
The advertised TypeScript command is:

```powershell
$env:LEGION_TEST_NODE_RUNTIME='C:/Users/dasbl/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe'
cargo test -p legion-app --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture
```

The command requires the retained local TypeScript and Pyright archives and
the approved Node executable. Its results classify as explicit app/real-server
integration evidence. They do not establish native GUI or full-product
qualification. Do not infer any unrun workspace test, dependency, release,
cross-platform, or phase gate from these checks; the clippy result is recorded
separately above.

## Remaining gap

Pyright supplies diagnostics and language operations but does not provide the
Python formatter or the normal Python toolchain settings surface. Formatter
provisioning/routing and explicit Python interpreter settings remain follow-up
work through the existing app-owned, proposal-mediated authority path.
