# ADR-0008: Define Local Tracker Schema and Event Retention Policy

## Status
Accepted

## Context
The tracker is the canonical local workflow state. It must link tasks, AI runs, approvals, and code changes durably without becoming a generic issue tracker or leaking secrets.

## Decision
Store tracker data in local SQLite per repository namespace. Schema includes Task, Plan, Work Session, AI Run, Decision, Code Link, Change Link, and Approval entities. Prompt/response bodies are subject to retention settings. Compact context manifests are always retained even when full prompts are discarded.

## Consequences
- **Positive**: Durable, queryable, and locally owned workflow history.
- **Positive**: Context manifests enable Privacy Inspector and replay without storing full prompts.
- **Negative**: SQLite schema migrations must be managed carefully as the domain evolves.
- **Negative**: Retention policies require explicit user controls to avoid unbounded storage growth.
