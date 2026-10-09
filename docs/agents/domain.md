# Domain Docs

## Layout

Use one shared domain context:

- `GLOSSARY.md` at the repository root.
- `plans/adrs/` for architecture decisions, preserving the existing
  ADR location and numbering.

## Before exploring

Read `AGENTS.md` and `docs/INDEX.md`, then the root glossary if present
and ADRs relevant to the work.

Read `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md` before crossing
authority boundaries. Check current ledgers and ADR status rather than
treating historical decisions as current product-readiness evidence.

If the glossary is absent, proceed silently. Do not create an empty
glossary or suggest creating one upfront. Domain-modeling work can
introduce it when terminology is resolved.

## Vocabulary

Use glossary-defined terms in issues, proposals, hypotheses, and tests.
If a needed concept is missing, reconsider the term or note the gap for
domain-modeling work.

## ADR conflicts

Explicitly identify any proposed conflict with an existing ADR and
explain why reopening the decision is warranted. Do not silently
override it.
