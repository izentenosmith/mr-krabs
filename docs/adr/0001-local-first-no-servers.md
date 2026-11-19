# ADR-0001 · Local-first, zero infrastructure

**Status:** Accepted

## Context
Household financial data is highly sensitive; the project must cost $0 to run; the household has two Macs on the same home network.

## Decision
All data lives in a SQLite file on each device. Devices sync peer-to-peer over the LAN. No backend, no accounts, no telemetry. Optional outbound calls (AI provider, FX rates) are opt-in and off by default.

## Consequences
- ✅ Privacy by construction; works offline; no hosting cost.
- ✅ Distribution via GitHub Releases (free for public repos).
- ❌ No sync when partners are apart (mitigation later: encrypted batches in a user-chosen synced folder).
- ❌ Backups are the user's responsibility → built-in encrypted backup feature.
