# Issue tracker: Local Markdown

Issues and specs live in local Markdown files under `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`.
- Spec: `.scratch/<feature-slug>/spec.md`.
- One implementation ticket per file:
  `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`.
- Record triage state in a `Status:` line near the top.
  Use the vocabulary in `docs/agents/triage-labels.md`.
- Append conversation history under `## Comments`.
- Resolve bare ticket numbers within the relevant feature directory;
  ask for the feature if the number is ambiguous.

## Publishing and reading

When a skill says "publish to the issue tracker", create the appropriate
local spec or ticket file, creating directories as needed.

When a skill says "fetch the relevant ticket", read its local file.

## Wayfinding operations

- Map: `.scratch/<effort>/map.md`, containing Notes,
  Decisions-so-far, and Fog.
- Child: `.scratch/<effort>/issues/<NN>-<slug>.md`.
- Record `Type:` as research, prototype, grilling, or task.
- For wayfinding tickets, `Status: claimed` and `Status: resolved`
  are workflow states in addition to the triage vocabulary.
- Record dependencies as `Blocked by: NN, NN`.
  A ticket is unblocked when every listed ticket is resolved.
- Frontier: choose the first ticket by number that is unresolved,
  unblocked, and unclaimed.
- Claim: save `Status: claimed` before beginning work.
- Resolve: append the result under `## Answer`, set `Status: resolved`,
  and add a summary and ticket link to the map's Decisions-so-far.
