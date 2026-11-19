# ADR-0004 · Noise XX + SPAKE2 pairing for LAN sync

**Status:** Accepted

## Context
Two devices need mutual authentication and encryption on a home LAN without a CA, servers, or accounts.

## Decision
- Pair once with a short-lived 6-digit code via SPAKE2 (password-authenticated key exchange), exchanging long-term X25519 public keys.
- All sessions use Noise `XX_25519_ChaChaPoly_BLAKE2s` (`snow`) with pinned peer keys. Discovery via mDNS (`mdns-sd`).

## Alternatives
- TLS with self-signed certs + pinning: workable but heavier (cert generation/rotation, rustls config) for no extra benefit.
- libp2p: powerful but large dependency surface for a two-peer LAN use case.

## Consequences
- ✅ Small, auditable crypto surface; no certificates; MITM resistant from first contact.
- ❌ Custom framing/protocol code → fuzz tests required.
