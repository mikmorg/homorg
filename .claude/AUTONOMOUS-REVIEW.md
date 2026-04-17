# Autonomous Code Review & Improvement Loop

Run with `/loop` to continuously improve the codebase one focused cycle at a time.

## Three-Layer Improvement Strategy

### Layer 1: Design & Implementation Smells (Priority)
**Focus:** Code correctness, DRY violations, API contracts, error handling

**Completed (Session 1 — 9 fixes):**
- ✅ Data bug: `external_codes` type normalization in item creation (prevents dedup issues)
- ✅ API contract: `Retry-After` only on transient 409s (not permanent conflicts)
- ✅ Rust DRY: Extract `_enqueue_enrichment_task` helper (16-line duplicate removed)
- ✅ Flutter DRY: Unify `ApiException`/`ApiError` exception types
- ✅ Flutter DRY: Share 401-retry in `_sendMultipartWithRetry` helper
- ✅ Svelte DRY: Extract `eventTypeToMessage()` helper (duplicate removed)
- ✅ Web DRY: Deduplicate 401-retry via `_retryWith401Handler` helper
- ✅ Flutter UX: Show taxonomy fetch errors (visible error message instead of silent fail)
- ✅ Flutter: Re-export `ApiError` from `HomorgApi` (import cleanup)

**Remaining High-Impact Smells:**
- `src/events/projector.rs` — 300-line if/else chain in `project_item_updated` (DEFERRED: needs type-safe dispatch design)
- `mobile/lib/screens/item_detail_screen.dart` — 2400-line God Widget (DEFERRED: needs tests first)
- `web/src/routes/stocker/[sessionId]/+page.svelte` — 1700-line God Component (DEFERRED: needs FSM extraction)

### Layer 2: Test Coverage Expansion (Next Priority)
**Focus:** Unit tests, integration tests, widget tests, e2e tests

**Rust Integration Tests:**
- File: `tests/stocker_session_tests.rs` (7 tests)
- Pattern: `common::setup()` helper from `tests/common/mod.rs`
- Tests: create_session, list_sessions, get_session, end_session, event_replay, item-in-session, move_item

**Flutter Unit Tests:**
- File: `mobile/test/services/homorg_api_test.dart` (10 tests)
- Pattern: `MockClient` from `package:http/testing.dart`
- Coverage: getItem, getChildren, getAncestors, resolveBarcode (3 variants), listContainerTypes, restoreItem, submitBatch

**Flutter Widget Tests:**
- File: `mobile/test/screens/browse_screen_test.dart` (6 tests)
- Coverage: breadcrumb navigation, ancestor display, container items, tapping actions

**Playwright E2E:**
- File: `web/e2e/item-detail.spec.ts` (4 tests)
- Edit: `web/e2e/browse.spec.ts` (add 2 tests for non-container items + deep breadcrumbs)

### Layer 3: Code Quality Checkpoints (Ongoing)
- Type safety: Dart null-safety, TypeScript strict mode, Rust sqlx validation
- Error handling: Visible errors to users, consistent backend error mapping
- API contracts: Centralized auth retry, smart Retry-After headers

---

## Each Cycle: Pick → Review → Improve → Verify → Commit

### 1. Pick ONE area to review

**Smell-Driven Cycles:**
- `src/commands/item_commands.rs` — Look for validation duplication, missing error handling
- `src/events/projector.rs` — Find repeated patterns in event handlers (but NOT the 300-line if/else)
- `src/api/item_routes.rs` — Check request handlers for edge cases
- `mobile/lib/screens/` — Identify missing error UI feedback, duplicated API patterns
- `web/src/lib/api/` — Look for duplication in fetch/retry logic, error handling gaps

**Test-Driven Cycles:**
- Focus on `tests/`, `mobile/test/`, `web/e2e/` directories
- Add tests for error paths, edge cases, API contract validation
- Verify Authorization headers, response parsing, offline behavior

### 2. Understand the current state
- Read the file(s) to understand purpose and pattern
- Check for obvious issues: unused vars, clippy warnings, missing tests, unclear logic
- Look at related code to understand conventions
- Scan recent commits to see if similar fixes were already done elsewhere

### 3. Make ONE targeted improvement
Pick from:

**Smell Fixes:**
- Extract a duplicated 10+ line block into a helper function
- Add missing error handling at a system boundary (network, auth, DB)
- Fix a clippy warning or fmt issue
- Remove dead code or unused imports
- Improve a comment where logic is non-obvious
- Refactor a function >40 lines to be clearer
- Simplify boolean logic or nested conditionals

**Test Additions:**
- Add test for an uncovered error case (401, 404, 422, etc.)
- Add test for missing error handling
- Add test for an edge case in API response parsing
- Add test for offline queue behavior
- Add test for breadcrumb/navigation edge cases

**API/Type Improvements:**
- Fix a type mismatch or unsafe cast
- Add explicit null-safety where implicit
- Centralize repeated error-handling patterns

### 4. Verify the change
**All Layers:**
```bash
# Rust
SQLX_OFFLINE=true cargo build --release
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

# Web
cd web && npm run check
npx vitest run --maxWorkers=1

# Flutter
PATH="/scratch/homorg-build/flutter/bin:$PATH" flutter analyze mobile/
PATH="/scratch/homorg-build/flutter/bin:$PATH" flutter test mobile/test/
```

Confirm it's a real improvement, not a sideways move.

### 5. Commit if worthwhile
- Only commit if the improvement is real and non-trivial
- Use clear commit messages: 
  - `fix: remove duplicate X logic in Y` (DRY violations)
  - `fix: handle missing error case in X` (correctness)
  - `feat: add test for X error path` (test coverage)
  - `fix: centralize X to reduce duplication` (abstraction)
  - `fix: show error UI when X fails` (UX)
  - `chore: remove dead code in X`
- Do NOT commit formatting or trivial changes

### 6. Self-assess and continue
- Note what was improved in a sentence or two
- If the codebase feels meaningfully better, continue
- If you've hit diminishing returns or are making sideways moves, pause
- Each cycle should take 10-20 minutes for smells, 15-30 for tests

## What Counts as "Improvement"

**Yes (Commit These):**
- Removing 10+ lines of duplicated code
- Adding a missing test for an error path
- Fixing a real clippy warning or type error
- Extracting a 30-line function into a 15-line helper
- Adding visible error feedback where it was silent
- Centralizing an auth/retry pattern used in 2+ places
- Handling a forgotten edge case (401, 404, null, etc.)

**No (Skip These):**
- Moving code around without changing it
- Renaming things with no business value
- Adding abstraction for hypothetical future use
- Comments that just restate what the code does
- Whitespace-only changes
- Refactoring code that already works well

## Boundaries

- **Don't refactor God-classes without tests** — ItemDetailScreen, stocker page need test coverage first
- **Don't introduce new dependencies** without asking
- **Don't change test patterns** or test infrastructure
- **Don't add features** — only improve existing code
- **Don't break master** — verify all tests pass before committing

## When to Pause

- "I've made 5+ cycles and hit diminishing returns" → Good stopping point
- "Every change I'm considering is sideways or cosmetic" → Stop
- "I'm about to refactor something large or cross-module" → Pause and ask
- "This requires changing architecture or multiple files" → Pause and ask
- "I want to refactor a God-class" → Add tests first, then tackle it in a dedicated session

---

## Last Updated
- Session 1: 2026-04-17 — 9 code smell fixes + test coverage plan documented
