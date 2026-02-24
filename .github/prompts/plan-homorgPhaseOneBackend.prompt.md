# Homorg Phase 1 Backend — Event-Sourced Inventory Service

**TL;DR:** Build a self-hosted Rust/Axum backend daemon with PostgreSQL (LTREE + JSONB + event sourcing), JWT auth for household members, and a comprehensive REST API supporting high-velocity stocker batch ingestion and basic picker retrieval. No AI/LLM — all fields are manual entry. The architecture is fully extensible: event store, polymorphic coordinates, and modular service layers are designed to absorb Phase 2+ features (VLM classification, semantic search, NFC, reorganization) without schema rewrites.

## 1. Project Scaffold & Infrastructure

Create the Rust workspace at `/home/mikmorg/homorg/`:

```
homorg/
├── Cargo.toml                    # workspace root
├── docker-compose.yml            # PostgreSQL 16 + app
├── Dockerfile                    # multi-stage Rust build
├── .env.example                  # config template
├── migrations/                   # sqlx migrations (ordered)
│   ├── 0001_extensions.sql
│   ├── 0002_users.sql
│   ├── 0003_items.sql
│   ├── 0004_event_store.sql
│   ├── 0005_barcode_sequences.sql
│   ├── 0006_scan_sessions.sql
│   └── 0007_refresh_tokens.sql
└── src/
    ├── main.rs
    ├── config.rs
    ├── lib.rs
    ├── db/           # pool, migration runner
    ├── models/       # domain types
    ├── events/       # event types, store, projector
    ├── commands/     # write-side business logic
    ├── queries/      # read-side queries
    ├── api/          # Axum route handlers
    ├── auth/         # JWT, Argon2, middleware
    ├── storage/      # file storage abstraction (images)
    └── errors/       # unified error types
```

**Key Crate Dependencies:**
- `axum` + `tower` + `tower-http` (CORS, tracing, compression)
- `sqlx` with `postgres` + `runtime-tokio` (compile-time checked queries)
- `tokio` (async runtime)
- `serde` / `serde_json` (serialization)
- `jsonwebtoken` (JWT)
- `argon2` (password hashing)
- `uuid` (v4/v7 generation)
- `chrono` (timestamps)
- `tracing` + `tracing-subscriber` (structured logging)
- `validator` (request validation)
- `dotenvy` (env config)

**Docker Compose** defines two services:
- `db`: PostgreSQL 16 with `ltree`, `pg_trgm`, `uuid-ossp` extensions pre-loaded. Persistent volume for data.
- `app`: the Rust binary. Depends on `db`. Exposes port 8080.

## 2. Database Schema

### 2a. Extensions (`0001_extensions.sql`)
Enable `ltree`, `pg_trgm`, `uuid-ossp`. Install `pgvector` and `postgis` now but don't create dependent columns — they'll be Phase 2+ additions, and having the extensions present avoids a migration headache later.

### 2b. Users (`0002_users.sql`)
```
users
├── id              UUID PK (v4)
├── username        VARCHAR(64) UNIQUE NOT NULL
├── password_hash   VARCHAR(256) NOT NULL (Argon2id)
├── display_name    VARCHAR(128)
├── role            VARCHAR(16) NOT NULL DEFAULT 'member'   — 'admin' | 'member' | 'readonly'
├── container_id    UUID   — FK→items (this user's ephemeral "In Use" container, populated after items table exists)
├── created_at      TIMESTAMPTZ DEFAULT NOW()
└── updated_at      TIMESTAMPTZ DEFAULT NOW()
```
The first registered user is auto-promoted to `admin`. Subsequent registrations require an admin-issued invite token or admin action. `container_id` links to the user's personal ephemeral container in the item hierarchy (created during user setup as `Root.Users.{username}`).

### 2c. Unified Items Table — Read Projection (`0003_items.sql`)
This is the denormalized **read projection** rebuilt from events. Every row represents the *current* state of a physical object.

```
items
├── id                UUID PK (v4)
├── system_barcode    VARCHAR(32) UNIQUE NOT NULL  — e.g. "HOM-000001"
├── node_id           VARCHAR(16) UNIQUE NOT NULL  — immutable UUID-derived LTREE label: "n_4a8b3c1d"
│
├── ── Classification ──
├── name              VARCHAR(512)
├── description       TEXT
├── category          VARCHAR(128)
├── tags              TEXT[]                        — arbitrary tags for filtering
│
├── ── Hierarchy ──
├── is_container      BOOLEAN NOT NULL DEFAULT FALSE
├── container_path    LTREE                        — e.g. "Root.HOM_000010.HOM_000042"
├── parent_id         UUID FK→items(id)            — redundant with path, but enables FK integrity
│
├── ── Polymorphic Coordinate (location within parent) ──
├── coordinate        JSONB                        — {"type":"abstract","value":"top_shelf"}
│                                                    {"type":"grid_2d","x":4,"y":2}
│                                                    {"type":"grid_3d","x":1,"y":2,"z":3}
│
├── ── Container Properties ──
├── location_schema   JSONB                        — defines valid coordinate types for children
├── max_capacity_cc   NUMERIC                      — max volumetric capacity (cm³)
├── max_weight_grams  NUMERIC                      — max load-bearing weight
│
├── ── Physical Properties ──
├── dimensions        JSONB                        — {"width_cm":…, "height_cm":…, "depth_cm":…}
├── weight_grams      NUMERIC
│
├── ── Fungible Commodity Tracking ──
├── is_fungible       BOOLEAN NOT NULL DEFAULT FALSE
├── fungible_quantity  INTEGER
├── fungible_unit     VARCHAR(32)                  — "pencils", "screws", etc.
│
├── ── External Identifiers ──
├── external_codes    JSONB DEFAULT '[]'           — [{"type":"UPC","value":"012345678905"}, …]
│
├── ── Condition & Valuation ──
├── condition         VARCHAR(32)                  — 'new','like_new','good','fair','poor','broken'
├── acquisition_date  DATE
├── acquisition_cost  NUMERIC(12,2)
├── current_value     NUMERIC(12,2)
├── depreciation_rate NUMERIC(5,4)
├── warranty_expiry   DATE
│
├── ── Extensible Metadata ──
├── metadata          JSONB DEFAULT '{}'           — arbitrary key-value (manufacturer, model, color, …)
├── images            JSONB DEFAULT '[]'           — [{"path":"…","caption":"…","order":0}, …]
│
├── ── Search ──
├── search_vector     TSVECTOR                     — auto-maintained via trigger
│
├── ── Audit ──
├── is_deleted        BOOLEAN NOT NULL DEFAULT FALSE
├── created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
├── updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
├── created_by        UUID FK→users(id)
└── updated_by        UUID FK→users(id)
```

**Indexes:**
- GiST on `container_path` — instant subtree queries via `<@` operator
- GIN on `search_vector` — full-text search
- GIN on `name` with `gin_trgm_ops` — fuzzy typo-tolerant search
- GIN on `external_codes` — lookup by UPC/EAN/ISBN
- GIN on `metadata` — arbitrary metadata queries
- GIN on `tags` — tag-based filtering
- B-tree on `parent_id`, `system_barcode`, `category`, `condition`
- Partial index on `is_container WHERE is_container = TRUE`
- Partial index on `is_deleted WHERE is_deleted = FALSE`

**Trigger:** A `tsvector_update_trigger` auto-populates `search_vector` from `name`, `description`, `category`, and casts of `metadata` values, weighted by relevance (name=A, category=B, description=C, metadata=D).

**LTREE Node ID Strategy:** Each item has an immutable `node_id` column (e.g., `n_4a8b3c1d`) derived from the first 8 hex characters of its UUID with a `n_` prefix, ensuring LTREE label safety (`[A-Za-z_][A-Za-z0-9_]*`). The `container_path` is composed of these node IDs: `n_00000001.n_a3b4c5d6.n_f1e2d3c4`. Barcodes and names are freely mutable without affecting any path. The root node uses well-known ID `n_00000001` (from UUID `...-000001`) and the Users container uses `n_00000002`.

### 2d. Event Store (`0004_event_store.sql`)
Append-only, immutable ledger — the authoritative source of truth.

```
event_store
├── id                BIGSERIAL PK                — monotonic ordering
├── event_id          UUID NOT NULL (v4)          — globally unique event identifier
├── aggregate_id      UUID NOT NULL               — the item UUID this event pertains to
├── aggregate_type    VARCHAR(32) DEFAULT 'item'  — extensible for future aggregate types
├── event_type        VARCHAR(64) NOT NULL        — discriminator (see list below)
├── event_data        JSONB NOT NULL              — full event payload
├── metadata          JSONB DEFAULT '{}'          — correlation_id, causation_id, session_id, batch_id
├── actor_id          UUID FK→users(id)           — who caused this event
├── created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
└── sequence_number   BIGINT NOT NULL             — per-aggregate ordering (optimistic concurrency)
```

**Constraint:** `UNIQUE(aggregate_id, sequence_number)` — prevents conflicting concurrent writes to the same aggregate.

**Indexes:**
- `(aggregate_id, sequence_number)` — replay a single item's history
- `event_type` — filter by event kind
- `created_at` — time-range queries, audit log pagination
- `actor_id` — "what did this user do?"
- `(metadata->>'session_id')` — correlate events from a stocker session

**Phase 1 Event Types:**

| Event Type | Payload (event_data) |
|---|---|
| `ItemCreated` | Full initial item state snapshot |
| `ItemUpdated` | `{field: "name", old: "...", new: "..."}` per changed field |
| `ItemMoved` | `{from_container_id, to_container_id, from_path, to_path, coordinate}` |
| `ItemMoveReverted` | `{original_event_id, from_container_id, to_container_id}` |
| `ItemDeleted` | `{reason}` |
| `ItemRestored` | `{from_event_id}` |
| `ItemImageAdded` | `{path, caption, order}` |
| `ItemImageRemoved` | `{path}` |
| `ItemExternalCodeAdded` | `{type, value}` |
| `ItemExternalCodeRemoved` | `{type, value}` |
| `ItemQuantityAdjusted` | `{old_qty, new_qty, reason}` |
| `ContainerSchemaUpdated` | `{old_schema, new_schema}` |
| `BarcodeGenerated` | `{barcode, assigned_to}` (or null if pre-printed) |

### 2e. Barcode Sequences (`0005_barcode_sequences.sql`)
```
barcode_sequences
├── prefix       VARCHAR(8) PK    — "HOM"
└── next_value   BIGINT DEFAULT 1
```
Atomic `UPDATE ... RETURNING` to generate sequences. Supports batch generation (increment by N, return range). The prefix is configurable per deployment.

### 2f. Scan Sessions (`0006_scan_sessions.sql`)
```
scan_sessions
├── id                    UUID PK
├── user_id               UUID FK→users NOT NULL
├── active_container_id   UUID FK→items
├── started_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
├── ended_at              TIMESTAMPTZ
├── items_scanned         INTEGER DEFAULT 0
├── items_created         INTEGER DEFAULT 0
└── items_moved           INTEGER DEFAULT 0
```

### 2g. Refresh Tokens (`0007_refresh_tokens.sql`)
```
refresh_tokens
├── id            UUID PK
├── user_id       UUID FK→users ON DELETE CASCADE
├── token_hash    VARCHAR(256) NOT NULL   — SHA-256 of the refresh token
├── device_name   VARCHAR(128)            — "Android Scanner", "Web Browser"
├── expires_at    TIMESTAMPTZ NOT NULL
└── created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
```

### 2h. Seed Data
After migrations, an initialization routine creates:
1. The `Root` container item (`HOM-ROOT` / `Root`) as the top-level LTREE root.
2. A `Users` container (`HOM-USERS` / `Root.Users`) as the parent for ephemeral user containers.

## 3. Event Sourcing Engine (`src/events/`)

### 3a. Event Type System (`events/types.rs`)
Define a Rust enum `DomainEvent` with serde-tagged variants for each event type. Each variant carries a strongly-typed payload struct. The enum serializes to/from the `event_type` + `event_data` columns.

### 3b. Event Store (`events/store.rs`)
- `append(aggregate_id, event: DomainEvent, actor_id, metadata) → Result<StoredEvent>`
  - Computes `sequence_number` as `MAX(sequence_number) + 1` for the aggregate (within a transaction).
  - Serializes and inserts. Returns the full stored event including `id` and `created_at`.
- `get_events(aggregate_id, from_sequence?) → Vec<StoredEvent>` — replay.
- `get_events_by_session(session_id) → Vec<StoredEvent>` — for batch undo.
- `get_events_paginated(filters, cursor, limit)` — admin audit log.

### 3c. Projector (`events/projector.rs`)
A synchronous in-process projector (NOT eventual consistency — we want immediate read-after-write in Phase 1). After each event is appended, the projector updates the `items` read projection table within the same database transaction. This guarantees:
- Event store and read projection are never out of sync.
- If projection fails, the event is not committed (transactional outbox pattern without the outbox).

Each event type maps to a projection handler:
- `ItemCreated` → `INSERT INTO items`
- `ItemUpdated` → `UPDATE items SET ... WHERE id = $1`
- `ItemMoved` → Update `parent_id`, `container_path`, `coordinate` for the item; cascade `container_path` update to all descendants using `UPDATE items SET container_path = new_prefix || subpath(container_path, nlevel(old_prefix)) WHERE container_path <@ old_path`
- `ItemDeleted` → `UPDATE items SET is_deleted = TRUE`
- etc.

A `rebuild_all_projections()` function replays the entire event store from `sequence_number = 0` to reconstruct the items table. This is an admin-only disaster recovery tool.

## 4. Command Handlers (`src/commands/`)

Each command validates business rules, then delegates to the event store + projector.

### 4a. Item Commands (`commands/item_commands.rs`)
- **CreateItem:** Validates barcode uniqueness, parent container exists and `is_container = true`, coordinate conforms to parent's `location_schema` (if defined). Derives immutable `node_id` from item UUID via `uuid_to_node_id()`, computes `container_path` by appending to parent's path. Appends `ItemCreated` event.
- **UpdateItem:** Accepts a partial update payload. Diffs against current state, generates `ItemUpdated` event with old/new values for each changed field.
- **MoveItem:** Validates destination is a container. Computes new `container_path`. If the moved item `is_container`, validates no circular reference (destination is not a descendant of the item). Appends `ItemMoved` event. Projector cascades path update to descendants.
- **DeleteItem:** If item `is_container` and has children, either refuse or recursively soft-delete (configurable). Appends `ItemDeleted`.
- **RestoreItem:** Un-delete. Validates parent still exists. Appends `ItemRestored`.
- **AddImage / RemoveImage:** Manage the `images` JSONB array.
- **AddExternalCode / RemoveExternalCode:** Manage `external_codes` JSONB.
- **AdjustQuantity:** For fungible containers. Validates `is_fungible = true`.

### 4b. Undo Commands (`commands/undo_commands.rs`)
- **UndoEvent:** Takes an `event_id`. Reads the original event. Generates the inverse compensating event (e.g., `ItemMoved` from A→B becomes `ItemMoveReverted` moving B→A). The compensating event's `metadata.causation_id` points to the original.
- **UndoBatch:** Takes a list of event IDs (or a session_id + time range). Processes undo in reverse chronological order to maintain consistency.

### 4c. Barcode Commands (`commands/barcode_commands.rs`)
- **GenerateBarcode:** Atomically increments `barcode_sequences`, returns formatted barcode string (`HOM-{zero_padded_value}`).
- **GenerateBatch:** Increments by N, returns the range.
- **ResolveBarcode:** Given an arbitrary scanned string, determines if it starts with the magic prefix (→ system barcode, look up in items), or is a commercial code (→ return `{type: "external", code_type: "UPC", value: "..."}` for the client to handle).

## 5. Query Handlers (`src/queries/`)

All queries read exclusively from the `items` projection table.

### 5a. Item Queries
- **GetById:** Full item detail, including computed `ancestors` path (by splitting `container_path` into labels and resolving each to an item name).
- **GetByBarcode:** Resolve `system_barcode` or search `external_codes` JSONB.
- **GetHistory:** `SELECT * FROM event_store WHERE aggregate_id = $1 ORDER BY sequence_number` with pagination.

### 5b. Container Queries
- **GetChildren:** `SELECT * FROM items WHERE parent_id = $1 AND is_deleted = FALSE` with sorting/pagination.
- **GetDescendants:** `SELECT * FROM items WHERE container_path <@ $1 AND is_deleted = FALSE` — LTREE subtree query.
- **GetAncestors:** Split the item's `container_path` into labels, resolve each to its item record. Returns ordered breadcrumb array.
- **GetContainerStats:** Count of children, sum of weights, sum of volumes, capacity utilization percentage.

### 5c. Search Queries
- **FullTextSearch:** `WHERE search_vector @@ plainto_tsquery($1)` with `ts_rank` scoring.
- **FuzzySearch:** `WHERE name % $1` using `pg_trgm` similarity, with `similarity()` threshold. Falls back to `ILIKE %$1%` for short queries.
- **PathSearch:** `WHERE container_path ~ $1` using LTREE `lquery` syntax. E.g., `Root.*.Kitchen.*` finds everything in any Kitchen.
- **FilteredSearch:** Combines text search with structured filters: `category`, `condition`, `tags`, `is_container`, `parent_id`, `acquisition_date` range, `current_value` range. Returns paginated results sorted by relevance or specified field.

## 6. REST API Layer (`src/api/`)

All endpoints are prefixed with `/api/v1`. Request/response bodies are JSON. Pagination uses cursor-based `?cursor=&limit=` parameters. Errors return a consistent `{"error": {"code": "...", "message": "..."}}` shape.

### 6a. Auth (`/api/v1/auth/`)
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/auth/setup` | None | First-time setup: create admin account. Fails if any user exists. |
| POST | `/auth/login` | None | Returns `{access_token, refresh_token, expires_in}`. Access tokens: 15min. Refresh: 30 days. |
| POST | `/auth/refresh` | Refresh token in body | Rotate refresh token, return new access token. |
| POST | `/auth/logout` | JWT | Revoke the refresh token. |
| GET | `/auth/me` | JWT | Current user profile. |
| POST | `/auth/invite` | JWT (admin) | Generate a single-use invite code for new member registration. |
| POST | `/auth/register` | Invite code | Register with valid invite code. |

### 6b. Items (`/api/v1/items/`)
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/items` | JWT | Create item. Body includes barcode (or auto-generate), parent_id, metadata fields. |
| GET | `/items/{id}` | JWT | Full item detail with ancestors breadcrumb. |
| PUT | `/items/{id}` | JWT (member+) | Partial update of metadata fields. |
| DELETE | `/items/{id}` | JWT (member+) | Soft-delete. |
| POST | `/items/{id}/restore` | JWT (member+) | Un-delete. |
| POST | `/items/{id}/move` | JWT (member+) | Move to new container. Body: `{container_id, coordinate?}`. |
| GET | `/items/{id}/history` | JWT | Paginated event log. |
| POST | `/items/{id}/images` | JWT | Upload image (multipart) or attach URL. |
| DELETE | `/items/{id}/images/{idx}` | JWT | Remove image. |
| POST | `/items/{id}/external-codes` | JWT | Add a UPC/EAN/ISBN. |
| DELETE | `/items/{id}/external-codes/{type}/{value}` | JWT | Remove external code. |
| POST | `/items/{id}/quantity` | JWT | Adjust fungible quantity. Body: `{new_quantity, reason}`. |

### 6c. Containers (`/api/v1/containers/`)
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/containers/{id}/children` | JWT | Direct children, paginated, sortable. |
| GET | `/containers/{id}/descendants` | JWT | Full subtree via LTREE (with optional `max_depth`). |
| GET | `/containers/{id}/ancestors` | JWT | Breadcrumb path to Root. |
| GET | `/containers/{id}/stats` | JWT | Child count, weight sum, volume util %. |
| PUT | `/containers/{id}/schema` | JWT (member+) | Update `location_schema` for coordinate validation. |

### 6d. Barcodes (`/api/v1/barcodes/`)
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/barcodes/generate` | JWT | Generate 1 new system barcode. Returns `{barcode: "HOM-000042"}`. |
| POST | `/barcodes/generate-batch` | JWT | Body: `{count: N}`. Returns array of barcodes. |
| GET | `/barcodes/resolve/{code}` | JWT | Returns `{type: "system", item_id: "..."}` or `{type: "external", code_type: "UPC"}` or `{type: "unknown"}`. |

### 6e. Stocker (`/api/v1/stocker/`)
The critical high-throughput endpoint set for the thin client.

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/stocker/sessions` | JWT | Start a scan session. Returns `{session_id}`. |
| POST | `/stocker/sessions/{id}/batch` | JWT | Submit batch of scan events (see payload below). |
| PUT | `/stocker/sessions/{id}/end` | JWT | Close the session, finalize stats. |
| GET | `/stocker/sessions` | JWT | List user's sessions with stats. |
| GET | `/stocker/sessions/{id}` | JWT | Session detail + associated events. |

**Batch Payload:**
```json
{
  "events": [
    {
      "type": "set_context",
      "barcode": "HOM-000042",
      "scanned_at": "2026-02-23T10:00:00Z"
    },
    {
      "type": "move_item",
      "barcode": "HOM-000001",
      "coordinate": {"type": "abstract", "value": "top_shelf"},
      "scanned_at": "2026-02-23T10:00:01Z"
    },
    {
      "type": "create_and_place",
      "barcode": "HOM-000099",
      "name": "Red Power Drill",
      "category": "Tools",
      "condition": "good",
      "metadata": {},
      "scanned_at": "2026-02-23T10:00:02Z"
    }
  ]
}
```

**Batch Response:**
```json
{
  "processed": 3,
  "results": [
    {"index": 0, "status": "ok", "context_set": "HOM-000042"},
    {"index": 1, "status": "ok", "event_id": "uuid..."},
    {"index": 2, "status": "ok", "event_id": "uuid...", "item_id": "uuid...", "needs_details": true}
  ],
  "errors": []
}
```

The entire batch is processed in a single database transaction. If any event fails validation, the response includes the error at that index but continues processing remaining events (partial success model — configurable to all-or-nothing via `?atomic=true` query param).

### 6f. Search (`/api/v1/search/`)
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/search` | JWT | `?q=` for text, `?path=` for LTREE lquery, `?category=`, `?condition=`, `?container_id=`, `?tags=`, `?min_value=`, `?max_value=`. Combined full-text + trigram + filters. |

### 6g. Undo (`/api/v1/undo/`)
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/undo/event/{event_id}` | JWT (member+) | Undo a single event. Returns the compensating event. |
| POST | `/undo/batch` | JWT (member+) | Body: `{event_ids: [...]}` or `{session_id: "...", from_timestamp: "..."}`. Undoes in reverse order. |

### 6h. Users (`/api/v1/users/`)
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/users` | JWT (admin) | List all household members. |
| GET | `/users/{id}` | JWT | User detail (own profile or admin). |
| PUT | `/users/{id}` | JWT | Update profile (own or admin). |
| PUT | `/users/{id}/role` | JWT (admin) | Change role. |
| DELETE | `/users/{id}` | JWT (admin) | Deactivate user. |

### 6i. System (`/api/v1/system/`)
| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | None | Liveness check. Returns DB connectivity status. |
| GET | `/stats` | JWT | Total items, containers, events, items by category/condition. |
| POST | `/admin/rebuild-projections` | JWT (admin) | Replay event store, rebuild items table. Long-running — returns 202 Accepted. |
| GET | `/admin/rebuild-projections/status` | JWT (admin) | Progress of rebuild. |

## 7. Authentication & Authorization (`src/auth/`)

- **Password hashing:** `argon2id` with recommended OWASP parameters.
- **JWT:** Access tokens (15 min TTL) carry `{sub: user_id, role, iat, exp}`. Signed with HS256 using a configurable secret from env. Refresh tokens are opaque random strings; only their SHA-256 hash is stored in the database.
- **Middleware:** An Axum `FromRequestParts` extractor (`AuthUser`) that validates the JWT `Authorization: Bearer` header on every protected route. Extracts `user_id` and `role` into request extensions.
- **Role enforcement:** A `RequireRole` tower layer that checks `AuthUser.role` against a minimum threshold (`readonly < member < admin`).

## 8. Image Storage (`src/storage/`)

Define a `StorageBackend` trait with methods `upload(key, bytes) → url`, `delete(key)`, `get_url(key) → url`. Phase 1 implements `LocalFilesystemBackend` — stores files under a configurable directory (e.g., `/data/images/{item_id}/{uuid}.{ext}`), serves them via a static file handler on `/files/`. The trait abstraction allows drop-in replacement with an S3-compatible backend in Phase 2.

## 9. Configuration (`src/config.rs`)

All settings via environment variables (loaded from `.env`):

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | — | PostgreSQL connection string |
| `JWT_SECRET` | — | HMAC signing key |
| `JWT_ACCESS_TTL_SECS` | 900 | Access token lifetime |
| `JWT_REFRESH_TTL_DAYS` | 30 | Refresh token lifetime |
| `LISTEN_ADDR` | `0.0.0.0:8080` | HTTP bind address |
| `BARCODE_PREFIX` | `HOM` | Magic prefix for system barcodes |
| `BARCODE_PAD_WIDTH` | 6 | Zero-padding width (HOM-000001) |
| `STORAGE_PATH` | `/data/images` | Local image storage root |
| `MAX_BATCH_SIZE` | 500 | Max events per stocker batch |
| `LOG_LEVEL` | `info` | Tracing filter |
| `RUST_LOG` | `homorg=debug` | Fine-grained log control |

## 10. Error Handling (`src/errors/`)

A unified `AppError` enum implementing Axum's `IntoResponse`. Variants:
- `NotFound(String)`
- `Conflict(String)` — barcode already exists, circular containment, etc.
- `ValidationError(Vec<FieldError>)`
- `Unauthorized` / `Forbidden`
- `DatabaseError(sqlx::Error)` — logged, returned as 500
- `StorageError(String)`

All errors serialize to `{"error": {"code": "ITEM_NOT_FOUND", "message": "...", "details": [...]}}` with appropriate HTTP status codes.

## 11. Extensibility Hooks for Phase 2+

The following are **not implemented** in Phase 1 but the schema and architecture explicitly accommodate them:

- **pgvector column on items:** Add `embedding VECTOR(1536)` for semantic search when the LLM pipeline arrives. The `search_vector` TSVECTOR provides adequate search until then.
- **PostGIS column on items:** Add `geolocation GEOGRAPHY(POINT, 4326)` when geo-tracking is needed. The JSONB `coordinate` field handles abstract/grid locations now.
- **AI classification queue:** The `ItemCreated` event with `needs_details: true` flag (in metadata) is already the hook. A future worker subscribes to events with this flag and triggers VLM classification.
- **NFC integration:** Uses the same `POST /items/{id}/move` endpoint. No backend changes needed — NFC is purely a client-side scan trigger.
- **Reorganization engine:** Reads from `items` projection and `event_store` for affinity analysis. Writes reorganization suggestions to a new `tasks` table (Phase 2 migration).

## 12. Verification

1. **Database:** Run `sqlx migrate run` and validate all tables, indexes, and triggers are created. Query `pg_catalog` to confirm extensions are loaded.
2. **Auth flow:** `curl` the `/auth/setup` → `/auth/login` → `/auth/me` chain. Verify JWT validation rejects expired/malformed tokens.
3. **Stocker workflow:** POST a batch with `set_context` + `move_item` + `create_and_place` events. Verify: (a) events appear in `event_store`, (b) `items` projection reflects current state, (c) `container_path` LTREE values are correct.
4. **Hierarchy:** Create a 5-level deep container chain. Verify `GET /containers/{id}/descendants` returns the full subtree. Move a mid-level container and verify all descendant paths are updated.
5. **Undo:** Move 10 items into a container. Undo the batch. Verify all items return to their original containers with correct paths.
6. **Search:** Insert items with varied names. Verify full-text search returns ranked results. Verify trigram search handles typos. Verify LTREE path queries narrow scope correctly.
7. **Concurrency:** Run 4 parallel stocker batch submissions targeting the same container. Verify no sequence number conflicts and all events are correctly recorded.

## 13. Key Decisions

- **Rust/Axum** over alternatives: matches performance needs of event-sourced system, strong type safety prevents schema drift
- **Synchronous projection** (same transaction as event append) over eventual consistency: simplicity for Phase 1, eliminates read-after-write issues. Can migrate to async workers later if needed.
- **Cursor-based pagination** over offset: correct semantics when the underlying data is append-only (event store) or mutable (items being moved)
- **Partial-success batch** as default over atomic: better UX for stocker workflow — don't discard 499 valid scans because 1 barcode was bad
- **Invite-code registration** over open registration: prevents unauthorized access in a household deployment without requiring complex admin UI
- **LTREE label derived from system barcode** over UUID-in-path: human-readable paths, debuggable, consistent with physical label
