# ADR-0006: Define AI Provider Abstraction and BYOK Credential Boundaries

## Status
Accepted

## Context
Devil IDE must support local models (Ollama, llama.cpp) and BYOK cloud providers (OpenAI, Anthropic) without contaminating core logic with provider-specific assumptions. Credentials must never leak into logs, memory, or prompts.

## Decision
Define a provider-agnostic capability interface in `devil-ai` (chat, embeddings, structured output, streaming). Implement provider-specific adapters in `devil-ai-providers`. Route all requests through `devil-security` privacy checks before transmission. Store credentials in OS keychain via `devil-platform`.

## Consequences
- **Positive**: New providers require only adapter implementation, not core changes.
- **Positive**: Centralized privacy enforcement prevents accidental exfiltration.
- **Negative**: Normalizing disparate provider APIs may lose provider-specific optimizations.
- **Negative**: Streaming and cancellation semantics vary by provider; adapters must handle edge cases.
