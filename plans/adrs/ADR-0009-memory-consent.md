# ADR-0009: Define Memory Consent, Storage, Retention, and Retrieval Policy

## Status
Accepted

## Context
Long-term memory is a differentiating feature but a high-trust surface. It must be strictly opt-in, inspectable, and bounded to prevent accidental leakage of sensitive project or personal information into AI context.

## Decision
Three memory tiers: Session (temporary), Project (local, task-linked, enabled by default for non-sensitive artifacts), and Long-Term (cross-project, strictly opt-in). Long-term memory requires explicit enablement, per-repository controls, candidate review before storage, and granular deletion. Embeddings for memory are generated locally by default. Cloud embedding generation for memory requires separate explicit consent.

## Consequences
- **Positive**: Respects privacy boundaries and user consent as a product differentiator.
- **Positive**: Local-first default minimizes exfiltration risk.
- **Negative**: Memory retrieval adds latency to AI context assembly; requires ranking and deduplication.
- **Negative**: UX for reviewing memory candidates must be lightweight or users will disable the feature.
