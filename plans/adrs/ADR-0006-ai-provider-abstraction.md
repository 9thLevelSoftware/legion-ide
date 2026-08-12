# ADR-0006: Define AI Provider Abstraction and BYOK Credential Boundaries

## Status
Accepted

## Context
Legion IDE must support local models (Ollama, llama.cpp) and BYOK cloud providers (OpenAI, Anthropic) without contaminating core logic with provider-specific assumptions. Credentials must never leak into logs, memory, or prompts.

## Decision
Define a provider-agnostic capability interface in `legion-ai` (chat, embeddings, structured output, streaming). Implement provider-specific adapters in `legion-ai-providers`. Route all requests through `legion-security` privacy checks before transmission. Store credentials in OS keychain via `legion-platform`.

2026-06-02 implementation note:

- `legion-ai-providers` exposes concrete local adapters for deterministic local tests, Ollama loopback, and llama.cpp loopback.
- Ollama uses the native local HTTP API and `OLLAMA_BASE_URL`, defaulting to `http://localhost:11434`.
- llama.cpp uses the server's OpenAI-compatible `/v1/chat/completions` and `/v1/embeddings` routes through `LEGION_LLAMA_CPP_BASE_URL` or `LLAMA_CPP_BASE_URL`, defaulting to `http://localhost:8080/v1`. `LEGION_LLAMA_CPP_API_KEY` or `LLAMA_CPP_API_KEY` may attach an optional local bearer token; no token is required by default.
- Generic OpenAI-compatible BYOK providers remain separate from llama.cpp and require `LEGION_OPENAI_COMPATIBLE_API_KEY` or `OPENAI_API_KEY` before network invocation.
- Provider response metadata remains metadata-only and stores endpoint fingerprints, not raw prompts, provider payloads, source text, or credential material.

## Consequences
- **Positive**: New providers require only adapter implementation, not core changes.
- **Positive**: Centralized privacy enforcement prevents accidental exfiltration.
- **Negative**: Normalizing disparate provider APIs may lose provider-specific optimizations.
- **Negative**: Streaming and cancellation semantics vary by provider; adapters must handle edge cases.
