# Homorg — Claude Handoff Document

> Last updated: 2026-04-07  
> Purpose: Allow a fresh Claude instance (on any machine) to resume work with full context.  
> Read this alongside CLAUDE.md, which covers commands, architecture, and env vars.

---

## Current State — Everything Is Committed and Pushed

`main` is fully up to date with origin. No stashed changes, no WIP branches.

```
a9090a6  feat: add Flutter camera app (mobile/) and Android build scripts
aa5bcae  feat: G5 offline queue integration — enqueue mutations on network failure
76470ec  Stocker picker: reset state on open, drop unreadable path in recents
e93b1c1  Stocker: smart container picker (recents) + container move mode
b8e1bbf  Round 8 hardening: input validation, memory leak fixes, and code cleanup
...
```

---

## What Has Been Built (Cumulative)

### Core App
Full-stack household inventory system:
- **Backend**: Rust/Axum, PostgreSQL, event sourcing (CQRS)
- **Frontend**: SvelteKit static PWA with offline support
- **Auth**: JWT HS256 (15-min access) + refresh tokens (30-day, hashed in DB)
- **Hierarchy**: PostgreSQL LTREE for container nesting
- **Offline**: Service Worker (Workbox) + IndexedDB mutation queue

### Review Rounds Completed (Rounds 3–8 + Backlog)
Every known issue from code review has been resolved. Key fixes:

| Round | Highlights |
|-------|-----------|
| R3 | CoordinateInput NaN, queue 429/401 retry, admin auth flash, aria labels, DB indexes |
| R4 | `FOR UPDATE` on image remove, barcode code-type uppercase, undo NotFound, security headers |
| R5 | Frontend test coverage: barcode-type, serial-scanner, queue, client; ISBN-10 X-digit fix |
| R6/backlog | B1 barcode type-aware JSONB query, B5 decimal precision (full stack), I3/I4 category_id |
| R7 | G6 PDF rate limiting, integration tests (R6-B), G7 rebuild stats, R4-D projector cap, R4-A session cleanup, R4-C JWT role claim removed |
| R8 | N1-N7: label_renames size limits, max_depth clamp, toast timer, sync listener cleanup, register URL fix, barcode sequence CHECK constraint, constants extraction |

### Stocker UX (Latest Feature Work)
- **Smart container picker** with Recents section (last 5 containers, persisted via `recentContainers.ts` store)
- Picker resets to recents each time it opens; scanning while picker is open intercepts and resolves as a container pick
- **Container move mode** (`⇄` toggle): scanning a container moves it into the active context instead of setting it as context; context banner turns amber; `setActiveContainer()` helper encapsulates all set-context logic

### G5 Offline Queue Integration (Last Backend-Touching Feature)
- `web/src/lib/api/client.ts`: non-GET, non-auth, non-FormData requests that hit a network error are now enqueued (via `web/src/lib/offline/queue.ts`) and throw `QueuedError`
- Stocker handles `QueuedError` gracefully (friendly log, no scan-error feedback)
- Pending-sync badge in `web/src/routes/+layout.svelte` is now a tap-to-sync button
- 6 new client tests cover all queue/no-queue branches

### Flutter Camera App (`mobile/`)
Companion Android app for photo capture:
- **Screens**: `ConnectScreen` (QR pair) → `QrScanScreen` → `SessionScreen` (camera/gallery upload)
- **Service**: `lib/services/api_service.dart` — uploads images to homorg `/api/camera/session/:id/photo`
- **APK**: built release APK at `mobile/build/app/outputs/flutter-apk/app-release.apk` (48.6 MB, gitignored)
- **Rebuild**: `bash scripts/build-android.sh` (sets all env vars inline, no shell profile changes)
- **Toolchain**: lives on `/mnt/homorg-build` LVM volume (Flutter 3.41.6, Android SDK, Gradle cache, Dart pub-cache); mount with `sudo mount /mnt/homorg-build` if not already mounted
- **LVM setup** (first time on a new machine): `sudo bash scripts/setup-build-volume.sh` — creates the volume, migrates existing tooling, sets up fstab

---

## Decimal Precision Detail (B5)

Financial fields use `rust_decimal::Decimal` end-to-end:
- Event struct `ItemCreatedData` uses `Option<Decimal>` with `decimal_compat::deserialize_opt` (accepts legacy f64 numbers from old events AND new strings)
- Request structs use `Option<Decimal>` / `Option<Option<Decimal>>`
- TypeScript sends as `string`; forms send trimmed string or null, **never `parseFloat()`**
- `diff_nullable_decimal!` macro handles Decimal diff; `diff_nullable_numeric!` handles f64 (separate macros)

---

## Remaining Known Issues (Low Priority / Accepted)

These are intentionally deferred, not forgotten:

| ID | Issue | Status |
|----|-------|--------|
| — | JWT HS256 single secret, no key rotation | Accepted for household scale |
| — | No password reset / account recovery | Accepted — single household |
| — | Default CORS wildcard in dev | Dev only, not production concern |
| — | Tag/category ops not event-sourced (no undo, no audit) | Accepted limitation |
| — | No per-user data scoping | By design (household app) |
| — | Undo of `ItemMoveReverted`/`ItemRestored` not supported | Accepted |
| — | Undo has no actor ownership check | Accepted (collaborative household) |
| — | History endpoint accessible for deleted items | Intentional audit trail |

---

## Test Suite Status

```bash
# Backend — 100 unit tests, no DB required
cargo test --all-targets

# Frontend — 187 tests across 9 files
cd web && npx vitest run --maxWorkers=1

# Integration tests — need live DB (testcontainers)
cargo test -- --ignored

# CI (mirrors GitHub Actions)
SQLX_OFFLINE=true cargo fmt --all -- --check
SQLX_OFFLINE=true cargo clippy --all-targets --all-features -- -D warnings
SQLX_OFFLINE=true cargo build --release
```

All tests pass on `main`.

---

## Key File Map

| File | Why You'd Touch It |
|------|--------------------|
| `src/models/event.rs` | All domain events — start every write-side change here |
| `src/events/projector.rs` | Read model maintenance (~37 KB, 13 event handlers) |
| `src/events/store.rs` | `EventStore::append` — writes event, calls projector in same txn |
| `src/commands/item_commands.rs` | Item create/update/delete/move business logic |
| `src/lib.rs` | `AppState` wiring — add new commands/queries here |
| `src/api/mod.rs` | Axum router + rate limiting config |
| `src/errors.rs` | Error → HTTP response mapping |
| `web/src/lib/api/client.ts` | Fetch wrapper with offline queue integration |
| `web/src/lib/offline/queue.ts` | IndexedDB mutation queue |
| `web/src/lib/coordinate-helpers.ts` | `parseCoordinate`, `computeLabelRenames` |
| `mobile/lib/services/api_service.dart` | Flutter HTTP service (camera session upload) |
| `scripts/build-android.sh` | Build Flutter APK (self-contained, all env inline) |
| `scripts/setup-build-volume.sh` | One-time LVM setup for build toolchain |

---

## Workflow Notes

- **Commit style**: `feat:`, `fix:`, `tests:`, `chore:` prefix; always append `Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>`
- **Env vars**: set inline in wrapper scripts, never in `~/.zshrc`
- **New features**: follow Command → DomainEvent → EventStore → Projector pattern; add the event to `src/models/event.rs` first
- **Adding a route**: wire in `src/api/mod.rs`, add handler in appropriate `*_routes.rs`, add command/query struct, wire into `AppState` in `src/lib.rs`
- **No speculative abstractions**: fix what's broken or build what's asked — no extra helpers, no future-proofing
