# ADR-0006 · AI is optional and only proposes

**Status:** Accepted

## Context
Bank statements (*estados de cuenta*) come in many PDF layouts. LLMs read them well but can hallucinate rows or amounts, and cloud providers would receive sensitive data.

## Decision
- AI extraction is disabled by default and compiled behind a cargo feature.
- Preferred provider is a local model (Ollama/LM Studio, OpenAI-compatible API on localhost); cloud providers only with the user's own key (stored in Keychain) and per-import consent.
- AI returns strict JSON validated by schema; amounts as strings parsed by Rust; totals must reconcile with statement totals; results land in staging for human review. AI never writes to the ledger.

## Consequences
- ✅ $0 path exists (local model); privacy preserved by default; errors are caught by deterministic checks.
- ❌ Local models need a capable Mac (Apple Silicon recommended) and a one-time model download by the user.
