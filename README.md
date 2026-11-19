# Mr Krabs 🦀

> *"I like money."* — a local-first personal & household finance app for macOS.

Mr Krabs tracks **personal** and **household** finances for a two-person household (you and your partner), runs **100% on your own devices**, syncs **peer-to-peer over your home LAN**, and costs **$0 in infrastructure** — no cloud, no SaaS, no accounts.

| | |
|---|---|
| Platform | macOS first (Apple Silicon + Intel), structured for Windows/Linux portability |
| Shell | Tauri v2 (thin OS wrapper only) |
| Core | Rust workspace (`mr-krabs-core`) — all domain logic, money math, import, sync |
| Storage | SQLite — storage & querying only (no triggers, no business logic in SQL) |
| UI | React + TypeScript + shadcn/ui — presentation only |
| Sync | mDNS discovery, explicit pairing, encrypted P2P, HLC change log |
| Default currency | **CLP** (configurable; multi-currency supported) |
| AI | **Optional**, off by default — assists importing bank *estados de cuenta* |

## Status

📐 **Design phase.** This repository currently contains the design documents. Code lands following the [roadmap](docs/07-roadmap.md), starting with a vertical slice.

## Documentation

| # | Document | What it answers |
|---|---|---|
| 00 | [Vision & scope](docs/00-vision.md) | What are we building, for whom, and what's out of scope? |
| 01 | [Requirements](docs/01-requirements.md) | Functional & non-functional requirements, feature catalogue |
| 02 | [Architecture](docs/02-architecture.md) | Layers, boundaries, repo layout, diagrams |
| 03 | [Data model](docs/03-data-model.md) | Rust domain model + SQLite schema |
| 04 | [Command boundary](docs/04-command-boundary.md) | Typed React ⇄ Rust interface |
| 05 | [LAN sync](docs/05-sync.md) | Discovery, pairing, encryption, HLC, conflict resolution |
| 06 | [Statement ingestion & AI](docs/06-statement-ingestion.md) | CSV/XLSX/PDF import, optional AI extraction |
| 07 | [Roadmap](docs/07-roadmap.md) | Step-by-step implementation plan |
| — | [ADRs](docs/adr/) | Architecture decision records |
| — | [Glossary](docs/glossary.md) | Terms (EN/ES) |

## Non-negotiables

1. **No floats for money.** Integer minor units (`i64`) + ISO currency, or `rust_decimal` for rates.
2. **No logic in SQL.** No triggers, stored procedures, side-effecting UDFs, or computed views.
3. **No logic in React.** Components render DTOs returned by Rust; they never sum, convert or aggregate money.
4. **No logic in Tauri commands.** Commands deserialize, delegate to `mr-krabs-core`, serialize.
5. **No servers.** Nothing leaves the household's devices unless the user explicitly enables a cloud AI provider with their own key.
