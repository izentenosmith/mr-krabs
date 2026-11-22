# Mr Krabs 🦀

[![ci](https://github.com/izentenosmith/mr-krabs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/izentenosmith/mr-krabs/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> *"I like money."* — a local-first personal & household finance app for macOS.

Mr Krabs tracks **personal** and **household** finances for a two-person household (you and your partner), runs **100% on your own devices**, syncs **peer-to-peer over your home LAN**, and costs **$0 in infrastructure** — no cloud, no SaaS, no accounts.

## What is Mr Krabs

Mr Krabs is a desktop app for managing your **own** and your **household's** money: accounts, transactions, budgets, debts and reports, in one place. It replaces the shared spreadsheet, runs entirely on your own computers, and keeps the two of you in sync without any server in between.

### Product non-negotiables

1. **Lightning fast.** It has to beat the spreadsheet: opens in under 2 seconds, scrolls 10,000 transactions without lag, and builds a monthly report in a blink ([NFR-05](docs/01-requirements.md)).
2. **Your data is yours.** It lives on your devices and never goes to the internet.
3. **100% local.** Personal and household finances, managed on your own machines; the two devices sync directly over your home network.
4. **AI is optional and never intrusive.** Off by default and never required: everything works without it. If you turn it on, it only helps where you ask (importing bank statements), and a cloud provider is used only if you add your own key.
5. **Easy to install.** One app, no accounts, no servers to set up.
6. **Easy to port.** Everything is in a single local database file you can back up and take to a new machine.
7. **$0, forever.** You run it yourself, so there is no subscription, no hosting and no cloud bill.

## Stack

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

## Why I built this

My partner and I track our money in a spreadsheet. We both have to keep it up to date, and when we edit it at the same time, keeping the copies in sync is a pain. We wanted something that is:

- **Just for us** — not a product, not a service.
- **Free** — not a single dime on subscriptions or hosting.
- **Safe from the internet** — our data stays on our own devices.
- **Easy** to use, install, move to a new machine and sync between the two of us.

We tried Notion and it didn't work out. We tried a cloud spreadsheet and it was worse. I looked at many open-source projects and subscription apps; some are genuinely interesting, but they all fall into the same trap: they live on the internet, our data ends up somewhere else, and none of them feel like ours.

So Mr Krabs is also a project we build together — shaped around our own needs and our own experience managing a household.

## Status

🏗️ **M0 — foundation ✅.** Workspace, Docker dev environment, CI, first tested core crates and the [UI design system](design-system/mr-krabs/MASTER.md) are in place, along with the libraries M1 builds on. Next: the [M1 vertical slice](docs/07-roadmap.md).

## Quick start (Docker)

```bash
docker compose up --build        # first run compiles Rust: a few minutes
# UI           → http://localhost:5173
# Core API     → http://localhost:7878/api/health
# DB browser   → http://localhost:8081 (read-only)
docker compose --profile test run --rm test-back    # Rust unit tests
docker compose --profile test run --rm test-front   # frontend unit tests
```

Desktop app (macOS, native): `pnpm install && pnpm tauri dev`. Details in [docs/08-development.md](docs/08-development.md).

## Contributing

`feature → dev → main`, PRs only; `main` accepts PRs from `dev` only. See [CONTRIBUTING.md](CONTRIBUTING.md).

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
| 08 | [Development](docs/08-development.md) | Docker compose, native setup, devserver |
| — | [ADRs](docs/adr/) | Architecture decision records |
| — | [Glossary](docs/glossary.md) | Terms (EN/ES) |

## Engineering non-negotiables

1. **No floats for money.** Integer minor units (`i64`) + ISO currency, or `rust_decimal` for rates.
2. **No logic in SQL.** No triggers, stored procedures, side-effecting UDFs, or computed views.
3. **No logic in React.** Components render DTOs returned by Rust; they never sum, convert or aggregate money.
4. **No logic in Tauri commands.** Commands deserialize, delegate to `mr-krabs-core`, serialize.
5. **No servers.** Nothing leaves the household's devices unless the user explicitly enables a cloud AI provider with their own key.

## License

[MIT](LICENSE)
