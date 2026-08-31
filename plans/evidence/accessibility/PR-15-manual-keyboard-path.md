# PR-15 Accessibility Evidence

## Probe status

| Platform | Repeatable probe | Observation |
| --- | --- | --- |
| Windows | `scripts/a11y-uia-walk.ps1` | Observed when run against a live desktop window; the committed script is the source of truth. |
| macOS | No committed OS-tree probe | Unobserved. |
| Linux | No committed OS-tree probe | Unobserved. |

Run `scripts/a11y-platform-probe.sh` to produce a machine-readable status. It
executes the committed Windows UIA walk (using `LEGION_A11Y_PROCESS` or the
default `legion-desktop`) and deliberately exits non-zero for macOS/Linux rather
than implying an observation that was not made.

## Manual keyboard-only path

This is a repeatable shell-command and projection checklist for a real desktop
window. It is not a renderer-backed keyboard path or a claimed human transcript;
no screen-reader session is claimed by this packet.

1. Open a trusted Manual workspace and focus the editor without using a mouse.
2. Use the available shell command `:search-workspace <query>` and inspect the resulting search projection. A published palette/keymap route for workspace search is pending.
3. Use the available shell command `:definition <byte-offset>` from the focused editor symbol. A published palette/keymap route for go-to-definition is pending.
4. Use `:git-nav-next-hunk` or `:git-nav-prev-hunk` to select a hunk, then the available shell command `:git-stage-hunk <hunk-id>`. `Git: Stage Focused Hunk` is not a published palette/keymap route in this commit and remains pending.
5. `Git: Commit Staged Changes` is registered as a palette command. Its complete keyboard-only invocation and proposal confirmation are pending a renderer reachability regression.
6. Use the available shell command `:term-launch <command>` to launch the trusted terminal. A published keyboard action and renderer-backed focus check are pending.

The desktop accessibility tests cover only in-process projection and target
geometry. They do not verify shell-command dispatch, renderer palette/keymap
reachability, a human OS-level keyboard walk, or an NVDA/VoiceOver/Orca
transcript.

## Acceptance boundary

PR-UI-001 remains Substrate validated until each supported OS has a committed,
repeatable OS-tree observation and a real screen-reader transcript. This packet
does not promote the ledger or claim macOS/Linux coverage.
