# Homorg

**Event-sourced personal inventory management system** — track every physical object in your home with barcode-labeled containers, hierarchical organization, and high-velocity batch scanning workflows.

Homorg is a self-hosted Rust backend daemon designed for households that want to know *exactly* where everything is. Scan a container barcode, scan the items going into it, and move on. The system maintains a complete audit trail via event sourcing, supports undo at any granularity, and is architected to absorb future phases (AI classification, semantic search, NFC, reorganization engine) without schema rewrites.

## Key Features

- **Event-sourced architecture** — append-only event store is the single source of truth; the items table is a materialized read projection rebuilt from events in the same transaction
- **LTREE container hierarchy** — instant subtree queries, ancestor breadcrumbs, and cascading path updates when containers are moved
- **Stocker batch API** — submit hundreds of scan events in a single request; partial-success or atomic mode; session tracking with stats
- **Code 128 barcode system** — auto-generated `HOM-NNNNNN` barcodes with atomic sequence generation and batch pre-printing support
- **Combined search** — PostgreSQL full-text search (tsvector), trigram fuzzy matching (pg_trgm), LTREE path patterns, and structured filters in one query
- **JWT auth with household model** — admin/member/readonly roles, invite-code registration, refresh token rotation
- **Complete undo** — reverse any event, batch of events, or entire scan session via compensating events
- **Extensible by design** — JSONB metadata/coordinates/external codes, polymorphic location schemas, pgvector + PostGIS extensions pre-installed for Phase 2+

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     REST API (Axum)                      │
│  auth · items · containers · barcodes · stocker · search │
├──────────┬──────────────────────┬────────────────────────┤
│ Commands │      Queries         │   Auth Middleware       │
│ (write)  │      (read)          │   (JWT + roles)         │
├──────────┴──────────────────────┴────────────────────────┤
│              Event Store + Projector                      │
│     append event → project to items table (same tx)       │
├──────────────────────────────────────────────────────────┤
│                   PostgreSQL 16                           │
│         ltree · pg_trgm · uuid-ossp · pgvector            │
└──────────────────────────────────────────────────────────┘
```

**CQRS pattern:** Commands validate business rules → append a `DomainEvent` to the event store → the Projector updates the `items` read projection — all within a single database transaction for immediate read-after-write consistency.

## Project Structure

```
homorg/
├── Cargo.toml                  # Dependencies & build config
├── Dockerfile                  # Multi-stage production build
├── docker-compose.yml          # PostgreSQL 16 + app services
├── .env.example                # Environment variable template
│
├── migrations/
│   ├── 0001_extensions.sql     # ltree, pg_trgm, uuid-ossp, pgvector
│   ├── 0002_users.sql          # Users + invite tokens
│   ├── 0003_items.sql          # Items projection (indexes, triggers, seed data)
│   ├── 0004_event_store.sql    # Append-only event ledger (immutability enforced)
│   ├── 0005_barcode_sequences.sql  # Atomic barcode generation
│   ├── 0006_scan_sessions.sql  # Stocker session tracking
│   └── 0007_refresh_tokens.sql # JWT refresh token storage
│
├── docker/
│   └── init-extensions.sql     # Pre-creates PG extensions on first boot
│
└── src/
    ├── main.rs                 # Tokio entrypoint, server bootstrap
    ├── lib.rs                  # AppState definition, module declarations
    ├── config.rs               # Environment-based configuration
    ├── db.rs                   # Connection pool & migration runner
    ├── errors.rs               # Unified error types → HTTP responses
    ├── storage.rs              # StorageBackend trait + local filesystem impl
    │
    ├── models/                 # Domain types & request/response structs
    │   ├── item.rs             # Item, ItemSummary, ItemDetail, CRUD requests
    │   ├── user.rs             # User, auth requests, invite tokens
    │   ├── event.rs            # DomainEvent enum (13 variants) + payloads
    │   ├── barcode.rs          # BarcodeResolution, GeneratedBarcode
    │   └── session.rs          # ScanSession, StockerBatchEvent, batch types
    │
    ├── events/                 # Event sourcing engine
    │   ├── store.rs            # Append-only EventStore (transactional)
    │   └── projector.rs        # Synchronous projector → items table
    │
    ├── auth/                   # Authentication & authorization
    │   ├── password.rs         # Argon2id hashing & verification
    │   ├── jwt.rs              # JWT creation, validation, refresh tokens
    │   └── middleware.rs       # AuthUser extractor (FromRequestParts)
    │
    ├── commands/               # Write-side business logic
    │   ├── item_commands.rs    # Create, update, move, delete, restore, images, codes, quantity
    │   ├── undo_commands.rs    # Single/batch/session undo via compensating events
    │   └── barcode_commands.rs # Generate, batch generate, resolve barcodes
    │
    ├── queries/                # Read-side query handlers
    │   ├── item_queries.rs     # By ID, by barcode, event history
    │   ├── container_queries.rs # Children, descendants (LTREE), ancestors, stats
    │   └── search_queries.rs   # Combined full-text + trigram + path + filters
    │
    └── api/                    # Axum route handlers (REST endpoints)
        ├── mod.rs              # Router builder, /api/v1 nesting
        ├── auth_routes.rs      # Setup, login, refresh, logout, invite, register
        ├── item_routes.rs      # CRUD + move/restore/images/codes/quantity
        ├── container_routes.rs # Children, descendants, ancestors, stats, schema
        ├── barcode_routes.rs   # Generate, batch, resolve
        ├── stocker_routes.rs   # Sessions + batch scan processing
        ├── search_routes.rs    # Combined search endpoint
        ├── undo_routes.rs      # Single & batch undo
        ├── user_routes.rs      # User management + role changes
        └── system_routes.rs    # Health, stats, admin rebuild
```

## Getting Started

### Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose
- [Rust](https://rustup.rs/) 1.75+ (for local development)

### Quick Start (Docker)

```bash
# Clone and configure
cp .env.example .env
# IMPORTANT: Change JWT_SECRET to a random 64+ character string
vim .env

# Start everything
docker compose up -d

# The API is available at http://localhost:8080
# Health check:
curl http://localhost:8080/api/v1/health
```

### Local Development

```bash
# Start only the database
docker compose up -d db

# Configure environment
cp .env.example .env
# Edit .env — the defaults work for local dev with the Docker database

# Run migrations and start the server
source "$HOME/.cargo/env"  # if needed
cargo run

# Or with auto-reload (install cargo-watch first)
cargo watch -x run
```

### First-Time Setup

After the server is running, create the admin account:

```bash
curl -X POST http://localhost:8080/api/v1/auth/setup \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "your_secure_password", "display_name": "Admin"}'
```

This returns an access token and refresh token. Use the access token for subsequent requests:

```bash
export TOKEN="<access_token from setup response>"

# Check your profile
curl http://localhost:8080/api/v1/auth/me \
  -H "Authorization: Bearer $TOKEN"
```

## API Reference

All endpoints are prefixed with `/api/v1`. Authenticated endpoints require `Authorization: Bearer <token>`.

### Auth (`/auth`)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/auth/setup` | None | First-time admin account creation |
| POST | `/auth/login` | None | Authenticate → access + refresh tokens |
| POST | `/auth/refresh` | Body | Rotate refresh token, get new access token |
| POST | `/auth/logout` | JWT | Revoke refresh token |
| GET | `/auth/me` | JWT | Current user profile |
| POST | `/auth/invite` | Admin | Generate single-use invite code |
| POST | `/auth/register` | Invite | Register new member with invite code |

### Items (`/items`)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/items` | Member+ | Create item (barcode auto-generated if omitted) |
| GET | `/items/{id}` | JWT | Full detail with ancestor breadcrumbs |
| PUT | `/items/{id}` | Member+ | Partial metadata update |
| DELETE | `/items/{id}` | Member+ | Soft-delete |
| POST | `/items/{id}/restore` | Member+ | Un-delete |
| POST | `/items/{id}/move` | Member+ | Move to different container |
| GET | `/items/{id}/history` | JWT | Paginated event log |
| POST | `/items/{id}/images` | Member+ | Upload image (multipart) |
| DELETE | `/items/{id}/images/{idx}` | Member+ | Remove image |
| POST | `/items/{id}/external-codes` | Member+ | Add UPC/EAN/ISBN |
| DELETE | `/items/{id}/external-codes/{type}/{value}` | Member+ | Remove external code |
| POST | `/items/{id}/quantity` | Member+ | Adjust fungible quantity |

### Containers (`/containers`)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/containers/{id}/children` | JWT | Direct children (paginated, sortable) |
| GET | `/containers/{id}/descendants` | JWT | Full subtree via LTREE (`?max_depth=`) |
| GET | `/containers/{id}/ancestors` | JWT | Breadcrumb path to Root |
| GET | `/containers/{id}/stats` | JWT | Child count, weight, volume utilization |
| PUT | `/containers/{id}/schema` | Member+ | Update coordinate validation schema |

### Barcodes (`/barcodes`)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/barcodes/generate` | JWT | Generate 1 system barcode |
| POST | `/barcodes/generate-batch` | JWT | Generate N barcodes (`{count: N}`) |
| GET | `/barcodes/resolve/{code}` | JWT | System item / external code / unknown |

### Stocker (`/stocker`)

High-throughput batch scanning workflow.

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/stocker/sessions` | JWT | Start scan session |
| GET | `/stocker/sessions` | JWT | List user's sessions |
| GET | `/stocker/sessions/{id}` | JWT | Session detail |
| POST | `/stocker/sessions/{id}/batch` | JWT | Submit batch scan events |
| PUT | `/stocker/sessions/{id}/end` | JWT | Close session |

**Batch payload example:**

```json
{
  "events": [
    {"type": "set_context", "barcode": "HOM-000042", "scanned_at": "2026-02-23T10:00:00Z"},
    {"type": "move_item", "barcode": "HOM-000001", "coordinate": {"type": "abstract", "value": "top_shelf"}, "scanned_at": "2026-02-23T10:00:01Z"},
    {"type": "create_and_place", "barcode": "HOM-000099", "name": "Red Power Drill", "category": "Tools", "scanned_at": "2026-02-23T10:00:02Z"}
  ]
}
```

Use `?atomic=true` to fail the entire batch on any error (default: partial success).

### Search (`/search`)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/search` | JWT | Combined search with query params below |

**Query parameters:** `q` (text), `path` (LTREE lquery), `category`, `condition`, `container_id` (subtree), `tags` (comma-separated), `is_container`, `min_value`, `max_value`, `cursor`, `limit`

### Undo (`/undo`)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/undo/event/{event_id}` | Member+ | Undo single event |
| POST | `/undo/batch` | Member+ | Undo by `event_ids` array or `session_id` |

### Users (`/users`)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/users` | Admin | List all members |
| GET | `/users/{id}` | JWT* | User detail (*own profile or admin) |
| PUT | `/users/{id}` | JWT* | Update profile (*own or admin) |
| PUT | `/users/{id}/role` | Admin | Change role |
| DELETE | `/users/{id}` | Admin | Deactivate user |

### System

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | None | Liveness + DB connectivity |
| GET | `/stats` | JWT | Item/container/event counts, breakdowns |
| POST | `/admin/rebuild-projections` | Admin | Replay event store → rebuild items (202) |

## Configuration

All settings via environment variables (or `.env` file):

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | — | PostgreSQL connection string (required) |
| `JWT_SECRET` | — | HMAC-SHA256 signing key (required, 64+ chars) |
| `JWT_ACCESS_TTL_SECS` | `900` | Access token lifetime (15 min) |
| `JWT_REFRESH_TTL_DAYS` | `30` | Refresh token lifetime |
| `LISTEN_ADDR` | `0.0.0.0:8080` | HTTP bind address |
| `BARCODE_PREFIX` | `HOM` | System barcode prefix |
| `BARCODE_PAD_WIDTH` | `6` | Zero-padding width → `HOM-000001` |
| `STORAGE_PATH` | `./data/images` | Local image storage directory |
| `MAX_BATCH_SIZE` | `500` | Max events per stocker batch |
| `RUST_LOG` | `homorg=debug` | Tracing filter directive |

## Database Schema

PostgreSQL 16 with extensions: `ltree`, `pg_trgm`, `uuid-ossp`, `pgvector`.

**Core tables:**

- **`items`** — Denormalized read projection with LTREE paths, JSONB metadata/coordinates/external codes, tsvector search, comprehensive indexes (GiST, GIN, B-tree, partial)
- **`event_store`** — Append-only immutable ledger (UPDATE/DELETE blocked by trigger), per-aggregate sequence numbers for optimistic concurrency
- **`users`** — Household members with roles and personal ephemeral containers
- **`scan_sessions`** — Stocker session tracking with scan/create/move counters
- **`barcode_sequences`** — Atomic sequence generator for system barcodes
- **`refresh_tokens`** — SHA-256 hashed refresh tokens with expiry
- **`invite_tokens`** — Single-use registration codes

**13 domain event types:** `ItemCreated`, `ItemUpdated`, `ItemMoved`, `ItemMoveReverted`, `ItemDeleted`, `ItemRestored`, `ItemImageAdded`, `ItemImageRemoved`, `ItemExternalCodeAdded`, `ItemExternalCodeRemoved`, `ItemQuantityAdjusted`, `ContainerSchemaUpdated`, `BarcodeGenerated`

## Stocker Workflow

The primary use case — rapidly cataloging items into containers:

1. **Print barcode labels** — `POST /barcodes/generate-batch` to pre-generate labels
2. **Label your containers** — stick Code 128 labels on shelves, boxes, drawers
3. **Start a session** — `POST /stocker/sessions`
4. **Scan & stock** — submit batches of `set_context` (scan container) + `move_item` / `create_and_place` events
5. **End session** — `PUT /stocker/sessions/{id}/end`

Items flagged `needs_details: true` in batch responses can be enriched later with full metadata via `PUT /items/{id}`.

## Extensibility (Phase 2+)

The architecture is designed to absorb future features without migration headaches:

| Feature | Hook |
|---------|------|
| AI/VLM classification | `ItemCreated` events with `needs_details` metadata flag → future worker subscribes |
| Semantic search | `pgvector` extension pre-installed; add `embedding VECTOR(1536)` column to items |
| Geo-tracking | `PostGIS` ready; add `GEOGRAPHY(POINT, 4326)` column |
| NFC scanning | Uses existing `POST /items/{id}/move` — NFC is purely client-side |
| Reorganization engine | Reads from `items` + `event_store` for affinity analysis |
| S3 image storage | Swap `LocalStorage` for S3 backend via `StorageBackend` trait |

## Tech Stack

| Component | Choice |
|-----------|--------|
| Language | Rust 2021 edition |
| Web framework | Axum 0.8 + Tower middleware |
| Database | PostgreSQL 16 (pgvector image) |
| Query layer | sqlx 0.8 (compile-time checked) |
| Auth | JWT (HS256) + Argon2id + refresh token rotation |
| Serialization | serde + serde_json |
| Async runtime | Tokio |
| Logging | tracing + tracing-subscriber |
| Containerization | Docker multi-stage build |

## License

MIT
