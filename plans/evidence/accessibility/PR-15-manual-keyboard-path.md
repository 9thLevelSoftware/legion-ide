# PR-15 Accessibility Evidence

## Probe status

| Platform | Repeatable probe | Observation |
| --- | --- | --- |
| Windows | `scripts/a11y-uia-walk.ps1` | Observed when run against a live desktop window; the committed script is the source of truth. |
| macOS | No committed OS-tree probe | Unobserved. |
| Linux | No committed OS-tree probe | Unobserved. |

Run `scripts/a11y-platform-probe.sh` to produce a machine-readable status. It
delegates Windows to the committed UIA walk and deliberately exits non-zero for
macOS/Linux rather than implying an observation that was not made.

## Manual keyboard-only path

This is the repeatable path to execute on a real desktop window. It is a
checklist, not a claimed human transcript; no screen-reader session is claimed
by this packet.

1. Open a trusted Manual workspace and focus the editor without using a mouse.
2. Use the published palette/keymap to run workspace search and keep focus in the results.
3. Invoke the published go-to-definition action from the focused editor symbol.
4. Focus a Git hunk and invoke `Git: Stage Focused Hunk` from its published key binding.
5. Open the Git commit command from the palette, enter a message, and confirm the proposal-mediated commit.
6. Launch the trusted terminal from its published keyboard action and verify the terminal receives focus.

The desktop accessibility tests cover the in-process projection and target
geometry for these surfaces. They do not substitute for a human OS-level
keyboard walk or NVDA/VoiceOver/Orca transcript.

## Acceptance boundary

PR-UI-001 remains Substrate validated until each supported OS has a committed,
repeatable OS-tree observation and a real screen-reader transcript. This packet
does not promote the ledger or claim macOS/Linux coverage.
