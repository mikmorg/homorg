# Autonomous Code Review & Improvement Loop

Run with `/loop` to continuously improve the codebase one focused cycle at a time.

## Each Cycle: Pick → Review → Improve → Verify → Commit

### 1. Pick ONE area to review
- A single file or module (prefer `src/commands/`, `src/events/`, `src/api/`, or `web/src/lib/`)
- Or a specific test file that could use expansion
- Or a specific code smell: dead code, long functions, missing error handling, incomplete tests

### 2. Understand the current state
- Read the file(s) to understand purpose and pattern
- Check for obvious issues: unused vars, clippy warnings, missing tests, unclear logic
- Look at related code to understand conventions

### 3. Make ONE targeted improvement
Pick from:
- Fix a clippy warning or fmt issue
- Remove dead code or unused imports
- Add missing error handling at a system boundary
- Extract a small helper function to reduce duplication
- Add a test for an uncovered error case
- Improve a comment where logic is non-obvious
- Refactor a function >30 lines to be clearer
- Simplify boolean logic or nested conditionals
- Remove or fix incomplete TODOs

### 4. Verify the change
**Rust:**
```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

**Frontend (web/):**
```bash
npm run check
npx vitest run --maxWorkers=1
```

Confirm it's a real improvement, not a sideways move.

### 5. Commit if worthwhile
- Only commit if the improvement is real and non-trivial
- Use clear commit messages: 
  - `refactor: extract X helper`
  - `fix: handle missing Y case`
  - `test: add coverage for Z error`
  - `chore: remove dead code in X`
  - `style: simplify X logic`
- Do NOT commit formatting or trivial changes

### 6. Self-assess and continue
- Note what was improved in a sentence or two
- If the codebase feels meaningfully better, continue
- If you've hit diminishing returns or are making sideways moves, pause
- Each cycle should take 5-15 minutes

## What Counts as "Improvement"

**Yes:**
- Removing 5+ lines of unused code
- Adding a missing test for an error path
- Fixing a real clippy warning
- Extracting a 20-line function into a 10-line helper
- Adding a one-line comment that clarifies non-obvious logic

**No:**
- Moving code around without changing it
- Renaming things with no business value
- Adding abstraction for hypothetical future use
- Comments that just restate what the code does ("increment x")
- Whitespace-only changes

## Boundaries

- **Don't refactor across modules** — keep changes localized to one file or tightly coupled group
- **Don't introduce new dependencies** without asking
- **Don't change test patterns** or test infrastructure
- **Don't add features** — only improve existing code
- **Don't break master** — if you're unsure, commit to a branch and ask before pushing

## When to Pause

- "I've made 3+ cycles and everything is already pretty clean" → Good stopping point
- "Every change I'm considering is sideways or cosmetic" → Stop
- "I'm about to refactor something large or cross-module" → Pause and ask
- "This fix requires changing architecture or multiple files" → Pause and ask
