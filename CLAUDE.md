# CLAUDE.md

Guidance for AI coding agents (and humans) working in this repository.

## Project

**Mr Krabs** — local-first personal & household finance desktop app for macOS. Tauri v2 shell, Rust core, SQLite storage, React + shadcn/ui frontend, P2P LAN sync between two devices. $0 infrastructure, no servers. Default currency **CLP** (configurable; USD, EUR, UF/CLF built in). UI language es-CL first.

**Status:** M0 (foundation) is done and the M1 libraries are in place (router, i18n, forms, tables, thiserror, proptest, insta); the M1 slice itself (accounts, ledger, transactions) has not started. Follow `docs/07-roadmap.md` in order; do not start a new milestone unless asked.

## Non-negotiable architecture rules

1. **All business logic lives in Rust** (`crates/`). Validation, balances, budgets, reports, FX, import parsing and sync merge all belong there.
2. **No floats for money.** Use `mrk_money::Money` (`i64` minor units + currency). Clippy denies `f32`/`f64` and float arithmetic workspace-wide; never `#[allow]` around that.
3. **SQLite is storage only.** No `CREATE TRIGGER`, `CREATE VIEW`, procedures or UDFs (`tools/check-no-sql-logic.sh` fails CI). PK/FK/UNIQUE/NOT NULL/simple CHECK constraints are fine. Don't aggregate money in SQL; load rows and compute in Rust.
4. **Shells are thin.** `apps/desktop/src-tauri` and `apps/devserver` only deserialize, call `mr-krabs-core`, and serialize. They depend on `mr-krabs-core` only.
5. **React is presentational.** No money math, parsing or formatting in TS. Money arrives as `MoneyDto { minor: string, currency, display }` and user input goes back as text. Only `packages/api-client` may import `@tauri-apps/*` (ESLint enforces it).
6. **Migrations are forward-only**: add `crates/mrk-store/migrations/NNNN_name.sql` and register it in `MIGRATIONS`. Never edit an applied migration. Every replicated table needs `hlc`, `hlc_device`, `deleted` (see `docs/05-sync.md`).
7. **Public repo:** never commit secrets, tokens, real bank statements, spreadsheets, databases, or personal names/amounts. Test fixtures must be synthetic.
8. **Never invent how the UI looks.** Every visual decision (colors, type, spacing, layout, components, states, motion, copy tone) is defined in `design-system/mr-krabs/MASTER.md`; a page file in `design-system/mr-krabs/pages/<page>.md`, if present, overrides it. Read both before any frontend work and use only the semantic tokens in `apps/desktop/src/index.css`, no raw hex. If the design system doesn't cover something, ask or propose an addition to `MASTER.md` instead of improvising.

Decisions are recorded in `docs/adr/`. If a change contradicts an ADR, write a new ADR rather than silently diverging.

## Layout

```
crates/
  mrk-money/       Money, CurrencyCode, es-CL parse/format, allocation (only dep: thiserror)
  mrk-domain/      Pure ledger rules (balanced double entry, classify)
  mrk-store/       rusqlite Store + embedded migrations
  mrk-import/      Statement import helpers
  mrk-ai/          Optional AI provider types (off by default)
  mrk-sync/        HLC (change log/protocol later)
  mr-krabs-core/   App facade, DTOs (api.rs), command catalogue (commands.rs)
apps/
  devserver/       Dev-only axum HTTP shell (docker `back`)
  desktop/         React app (src/) + Tauri shell (src-tauri/)
packages/
  api-client/      Typed client: Tauri IPC or HTTP transport
design-system/
  mr-krabs/        UI design system: MASTER.md + pages/<page>.md overrides
docs/              Design docs 00–08, ADRs, glossary
compose.yaml       back + front + db (+ test profile)
```

## Commands

```bash
# Docker (default dev environment)
docker compose up --build                            # UI :5173, API :7878, DB browser :8081
docker compose --profile test run --rm test-back     # guardrails + fmt + clippy + Rust tests
docker compose --profile test run --rm test-front    # lint + typecheck + vitest + build

# Native
cargo fmt --all
cargo clippy --workspace --exclude mr-krabs-desktop --all-targets -- -D warnings
cargo test --workspace --exclude mr-krabs-desktop
cargo run -p mrk-devserver                           # http://127.0.0.1:7878
pnpm install && pnpm lint && pnpm typecheck && pnpm test
pnpm dev                                             # Vite on :5173 (proxies /api to :7878)
pnpm tauri dev                                       # desktop app (macOS)
```

Run fmt, clippy and the tests for anything you touch before committing.

## Adding a command (the usual vertical slice)

1. Domain logic + unit tests in the right `mrk-*` crate (pure functions where possible).
2. Method on `mr_krabs_core::App`, plus DTOs in `api.rs` (`#[serde(rename_all = "camelCase")]`).
3. Register it in `commands.rs` (`dispatch` and `COMMANDS`); the catalogue test checks every listed command dispatches.
4. Add a `#[tauri::command]` in `apps/desktop/src-tauri/src/lib.rs` and list it in `generate_handler!`.
5. Add types and a method to `packages/api-client` (`types.ts`, `createApi`); types are hand-written until tauri-specta lands in M1.
6. UI: presentational component taking DTOs/callbacks, plus a container using TanStack Query. Strings go in both `src/i18n/es-CL.ts` and `en.ts`. Add Vitest + Testing Library tests.

## Conventions

- Rust edition 2024, `rustfmt.toml` (max width 100). Errors are explicit enums deriving `thiserror::Error`; map to `ApiError { code, message }` at the core boundary.
- Rust tests: `proptest` for invariants, `insta` snapshots for wire contracts, `pretty_assertions` for diffs.
- Logging: `tracing` only; never log amounts or descriptions at `info`.
- IDs are UUIDv7 (from M1). Dates are civil `YYYY-MM-DD`; sync timestamps use HLC.
- Wire format is camelCase JSON. Error codes are SCREAMING_SNAKE_CASE.
- Frontend: TypeScript strict, `@/` alias to `apps/desktop/src`, shadcn components in `src/components/ui`, features in `src/features/<area>`, routes in `src/routes` (TanStack Router, hash history). UI copy in Spanish (es-CL) via `react-i18next` keys in `src/i18n/`, never inline strings. Forms use `react-hook-form` + `zod` for shape only; amounts are validated in Rust. Lists use TanStack Table (+ Virtual for long ones).
- `Cargo.lock` and `pnpm-lock.yaml` are committed; CI installs `--locked` / `--frozen-lockfile`, so commit lockfile changes with the manifest change.
- Tests sit next to the code (`#[cfg(test)] mod tests`, `*.test.ts(x)`).

## Git workflow

- Branches: `feat/*`, `fix/*`, `docs/*`, `chore/*` off **`dev`**, then PR into `dev` (squash). `main` only receives PRs **from `dev`** (merge commit). See `CONTRIBUTING.md`.
- `main` and `dev` are protected: no direct or force pushes. Required checks: `guardrails`, `rust`, `frontend` (+ `branch-policy` on main).
- Use Conventional Commits (`feat(money): …`, `fix: …`, `docs: …`).
- Never merge into `main` without the owner's explicit approval.
