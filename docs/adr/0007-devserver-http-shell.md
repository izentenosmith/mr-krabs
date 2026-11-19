# ADR-0007 · Dev-only HTTP shell for Docker development

**Status:** Accepted

## Context
Development should run via `docker compose` (back, front, db). A Tauri/macOS WebView cannot run inside a Linux container, and SQLite is an embedded library rather than a database server.

## Decision
- Add `apps/devserver`, an axum binary that — like the Tauri shell — only deserializes, calls `mr_krabs_core::commands::dispatch`, and serializes. It is never shipped.
- `@mr-krabs/api-client` has two transports behind one typed API: Tauri IPC (desktop) and HTTP `POST /api/invoke/{command}` (browser). It auto-detects Tauri at runtime.
- The `db` compose service is a read-only `sqlite-web` browser over the shared data volume.
- The command catalogue lives in the core (`commands::COMMANDS`), so both shells expose identical behavior; a core test asserts every listed command is dispatchable.

## Consequences
- ✅ Full-stack development and testing in Docker on any OS; UI iterates in a normal browser.
- ✅ Shells stay thin; business logic is still only in Rust.
- ❌ Two shells to keep in sync (mitigated by the shared catalogue; from M1, `tauri-specta` generates TS types for both).
- ❌ OS integrations (Keychain, file dialogs) are Tauri-only; the devserver uses dev fallbacks.
