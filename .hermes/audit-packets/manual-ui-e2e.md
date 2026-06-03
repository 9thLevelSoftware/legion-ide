# Audit Packet: manual-ui-e2e — Manual mode, projection UI, and deterministic IDE flows

## Project

- Name: devil-ide / Legion IDE
- Repository: https://github.com/9thlevelsoftware/devil-ide.git
- Coordinator: GPT-5.5
- Auditor: Kimi 2.6

## Objective

Inspect only the bounded scope below and report evidence-backed findings.

## Scope

Audit Manual mode exclusion, dock/panel registry, desktop projections, daily editing/search/git/terminal/debug/test workflows, and UI tests.

## Allowed Read Paths

- `AGENTS.md`
- `crates/devil-ui`
- `crates/devil-desktop`
- `crates/devil-app`
- `crates/devil-editor`
- `crates/devil-text`
- `crates/devil-project`
- `crates/devil-terminal`
- `crates/devil-security`
- `plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md`
- `plans/product-readiness-ledger.md`

## Forbidden Areas

- `.git`
- `target`
- `target-clippy-stable`

## Source Requirements / Roadmap Claims

- Phases 1-2 require mode-aware projection shell, Manual AI/network/cloud exclusion, editor/file/search/git/terminal/debug/test workflows, and proposal-mediated saves.
- Product readiness ledger currently requires working UX paths and tests before product-ready claims.

## Questions to Answer

- Are Manual/deterministic features implemented, partially implemented, stubbed, or missing?
- Do tests prove Manual mode excludes AI/cloud/worker/hosted telemetry panels?
- Can daily editing and save flows work e2e without AI?

## Evidence Requirements

For every finding include:

- file path
- line number or symbol if available
- code/test/doc evidence
- confidence: high/medium/low
- whether the finding is observed, validated, or needs reproduction

## Do Not

- Do not implement fixes.
- Do not modify files.
- Do not create branches, commits, PRs, or tasks.
- Do not audit outside the allowed read paths.
- Do not treat roadmap inference as high confidence.

## Output Format

- Scope inspected
- Files inspected
- Commands run, if any
- Feature status table
- Findings table
- Suggested validation commands
- Suggested implementation tasks, if any
- Open questions
