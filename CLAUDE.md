# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Backend (Rust)
```bash
cargo build --release
cargo run                          # start server (requires DATABASE_URL env var)
cargo watch -x run                 # dev with auto-reload
cargo test --all-targets           # run all tests (unit tests pass without DB)
cargo test <test_name>             # run a single test by name
cargo test -- --ignored            # run integration tests (require live DB)
cargo fmt --all -- --check         # format check
cargo clippy --all-targets --all-features -- -D warnings  # lint
```

### Frontend (SvelteKit)
```bash
cd web
npm run dev                        # dev server with HMR (proxies /api → localhost:8080)
npm run build                      # production build
npm run check                      # svelte-check type checking
npx vitest run --maxWorkers=1      # run all frontend tests
npx vitest run --maxWorkers=1 <file>  # run a single test file
```

### Infrastructure
```bash
docker compose up -d db            # start PostgreSQL only
docker compose up -d               # start all services
python3 scripts/seed.py --username admin  # populate test data (runs against localhost:3000)
```

### CI checks (mirrors GitHub Actions)
```bash
SQLX_OFFLINE=true cargo fmt --all -- --check
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings
SQLX_OFFLINE=true cargo build --release
cargo test --all-targets
```

## Architecture

### CQRS + Event Sourcing

Every write goes through a single transaction: **Command → DomainEvent → EventStore append → Projector update → items table**. This means reads are immediately consistent after writes. The flow lives in:

- `src/commands/` — validate business rules, call `EventStore::append`
- `src/events/store.rs` — `EventStore::append` writes to `event_store` table, calls projector in same txn
- `src/events/projector.rs` — 13 event handlers that maintain the `items` denormalized read model
- `src/models/event.rs` — `DomainEvent` enum (single source of truth for all event payloads)

The `event_store` table is append-only and immutable. Undo works by appending compensating events (e.g. `ItemMoveReverted`), not by mutating history.

### Request Lifecycle

```
HTTP request → Axum router (src/api/mod.rs)
  → optional AuthUser extractor (src/auth/middleware.rs) — validates JWT
  → handler in src/api/*_routes.rs
    → src/queries/ (reads, no txn needed) OR src/commands/ (writes, transactional)
      → src/events/store.rs + src/events/projector.rs
```

`AppState` (defined in `src/lib.rs`) is cloned into every handler via Axum's `State` extractor. It holds the DB pool, event store, and all command/query structs.

### Container Hierarchy

Items are organized in containers using PostgreSQL LTREE. Each item has a `path` column (e.g. `home.kitchen.drawer`) that enables instant subtree queries. Container schemas are JSONB:

- `{ "type": "abstract", "labels": [...] }` — named slots
- `{ "type": "grid", "rows": N, "columns": M }` — positional grid
- `{ "type": "geo" }` — geographic location

Children store their coordinate as `{ "type": "abstract", "value": "label name" }`. Frontend helpers in `web/src/lib/coordinate-helpers.ts` handle parsing and label rename cascade logic.

### Authentication

- JWT HS256 access tokens (15-min TTL) + refresh tokens (30-day, stored hashed in DB)
- `AuthUser` extractor in `src/auth/middleware.rs` — returns 401 if token invalid/missing
- Roles: `admin` | `member` — checked per-route in handlers
- Setup route (`POST /api/setup`) only works when zero users exist

### Frontend

SvelteKit with static adapter (fully pre-built). Dev server proxies `/api` and `/files` to `localhost:8080`. All `.svelte` files use **Svelte 5 runes mode**:

- State: `$state()`, derived: `$derived()`, effects: `$effect()`
- Props: `$props()`, bindable: `$bindable()`
- Layout children: `{@render children()}`
- `$app/state` (not `$app/stores`) for `page`

PWA with Service Worker (Workbox): offline scanning queues to IndexedDB and syncs when reconnected. Cache strategies: taxonomy = StaleWhileRevalidate (5 min), images = CacheFirst (24 hr).

### Key Files to Know

| File | Purpose |
|------|---------|
| `src/models/event.rs` | All domain events — start here for any write-side change |
| `src/events/projector.rs` | Read model maintenance — largest file (~37 KB) |
| `src/lib.rs` | `AppState` definition + all dependency wiring |
| `src/api/mod.rs` | Router with rate limiting config |
| `src/errors.rs` | Unified error → HTTP response mapping |
| `web/src/lib/api/` | All frontend API client functions |
| `web/src/lib/offline/` | IndexedDB offline sync queue |

## Environment Variables

Required: `DATABASE_URL`, `JWT_SECRET` (64+ chars).

Notable defaults: `LISTEN_ADDR=0.0.0.0:8080`, `STORAGE_PATH=./data/images`, `BARCODE_PREFIX=HOM`, `DB_MAX_CONNECTIONS=20`. Rate limiting disabled by default (`RATE_LIMIT_RPS` unset). See `src/config.rs` for all options.

## Testing Notes

- Unit tests (`cargo test`) run without a database.
- Integration tests in `tests/` are `#[ignore]` — need a live DB, run with `-- --ignored`.
- Frontend tests use `fake-indexeddb`; use `vi.resetModules()` + `new FDBFactory()` per test to avoid state leakage between tests.
- `SQLX_OFFLINE=true` is required for CI (no DB available during build).
