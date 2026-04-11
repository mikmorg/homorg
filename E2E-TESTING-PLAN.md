# E2E Web UI Testing Plan

Headless browser testing via Playwright on SSH-only server.

## Setup

```bash
cd web
npm install -D @playwright/test
sudo npx playwright install chromium --with-deps  # installs Chromium + system libs (libgbm, libnss, etc.)
```

## Playwright Config

`web/playwright.config.ts`:
- Base URL: `http://localhost:5173`
- Browser: Chromium headless (no Xvfb needed — Playwright handles it)
- Web server: auto-start `npm run dev` before tests
- Backend: assume running on `:8080` (start separately or via `globalSetup`)

## Test Structure

```
web/e2e/
├── setup/
│   └── auth.setup.ts          # Login once, save storage state
├── auth.spec.ts               # Setup flow, login, logout, register
├── browse.spec.ts             # Browse items, navigate containers
├── stocker.spec.ts            # Start session, scan items, camera link
├── journal.spec.ts            # View events, undo, filter
├── search.spec.ts             # Full-text search, barcode resolve
├── admin.spec.ts              # User management, categories, tags
└── fixtures.ts                # Shared helpers (login, seed data check)
```

## Key Test Scenarios

### Auth
- First-time setup creates admin account
- Login with valid/invalid credentials
- Session persists across page reload (JWT refresh)
- Logout clears state

### Stocker Session
- Start session, set container context
- Scan barcode → item appears in log
- Quick-create item from unknown barcode
- Camera link generation shows QR code
- Session polling shows camera uploads
- End session

### Journal
- Events appear newest-first
- Filter by event type works
- Undo button triggers compensating event
- Load more pagination works

### Browse
- Navigate container hierarchy
- Item detail shows images, metadata
- Edit item fields
- Delete and restore item

### Search
- Text search returns results
- Barcode search resolves items

## Running

```bash
# Ensure backend is running on :8080 with seeded data
npx playwright test                    # all tests, headless
npx playwright test e2e/journal.spec   # single file
npx playwright test --headed           # with visible browser (needs X11/VNC)
npx playwright show-report             # HTML report
```

## CI

Playwright is **not** wired into CI. E2E browser tests are an on-demand tool
for interactive coding sessions — used to drive the frontend in a real browser
while iterating on features, not as a merge gate. Keep CI fast and deterministic;
reach for Playwright locally when a change needs end-to-end verification.

## Notes
- No Xvfb needed — Playwright's Chromium runs headless natively
- Screenshots on failure: `use: { screenshot: 'only-on-failure' }`
- Video recording available: `use: { video: 'retain-on-failure' }`
- Trace viewer for debugging: `use: { trace: 'retain-on-failure' }`
