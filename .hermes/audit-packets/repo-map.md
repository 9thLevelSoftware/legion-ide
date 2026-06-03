# Audit Packet: repo-map — Repository map and gate audit

## Project

- Name: devil-ide / Legion IDE
- Repository: https://github.com/9thlevelsoftware/devil-ide.git
- Coordinator: GPT-5.5
- Auditor: Kimi 2.6

## Objective

Inspect only the bounded scope below and report evidence-backed findings.

## Scope

Identify workspace crates, scripts, docs, tests, CI workflows, entrypoints, and baseline command health.

## Allowed Read Paths

- `AGENTS.md`
- `Cargo.toml`
- `crates`
- `xtask`
- `scripts`
- `docs`
- `plans`
- `.github/workflows`
- `deny.toml`
- `rust-toolchain.toml`

## Forbidden Areas

- `.git`
- `target`
- `target-clippy-stable`
- `node_modules`
- `vendor`

## Source Requirements / Roadmap Claims

- Verify the repo matches the Legion plan baseline and CI gates are executable.
- Do not modify files.

## Questions to Answer

- What crates, binaries, tests, and scripts exist?
- What gates are required locally and in CI?
- Are there obvious generated/binary artifacts committed that affect repo hygiene?

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
