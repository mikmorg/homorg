# Security Roadmap (Deferred)

Items identified during enterprise readiness review. To be addressed in a future phase.

## Authentication & Authorization

| # | Finding | File(s) | Priority |
|---|---------|---------|----------|
| S1 | No account lockout after failed logins | `src/api/auth_routes.rs` | HIGH |
| S2 | No security event audit log | `src/api/auth_routes.rs` | HIGH |
| S3 | No MFA/TOTP support | auth module | MEDIUM |
| S4 | HS256 JWT — single symmetric key, no rotation | `src/auth/jwt.rs` | MEDIUM |
| S5 | No password reset / account recovery | auth module | HIGH |
| S6 | No maximum concurrent sessions per user | `src/queries/token_queries.rs` | LOW |
| S7 | No API key auth for service-to-service | auth module | LOW |
| S8 | CORS defaults to wildcard `*` | `src/config.rs` | HIGH |
| S9 | Rate limiting disabled by default | `src/config.rs` | HIGH |
| S10 | `/files` served without authentication | `src/api/mod.rs` | MEDIUM |

## Data Protection & Privacy

| # | Finding | File(s) | Priority |
|---|---------|---------|----------|
| D1 | No encryption at rest for stored images | `src/storage.rs` | MEDIUM |
| D2 | No database encryption at rest config | `docker-compose.yml` | MEDIUM |
| D3 | JWT secret in plain env var (no `_FILE` support) | `src/config.rs` | HIGH |
| D4 | No data export capability (GDPR Art. 20) | — | MEDIUM |
| D5 | No "right to erasure" (GDPR Art. 17) | — | MEDIUM |
| D6 | No backup automation | — | HIGH |
| D7 | Event store grows unbounded | `src/events/store.rs` | LOW |
| D8 | No data classification labels | — | LOW |

## Frontend Security

| # | Finding | Priority |
|---|---------|----------|
| F1 | No Content-Security-Policy header | HIGH |
| F6 | No end-to-end type safety (manual TS types) | MEDIUM |

## Mobile Security

| # | Finding | Priority |
|---|---------|----------|
| M1 | No certificate pinning | MEDIUM |
| M2 | Token stored in SharedPreferences (plaintext) | HIGH |
| M3 | No crash reporting | MEDIUM |
| M4 | No code obfuscation | LOW |
| M5 | No minimum TLS version enforcement | MEDIUM |

## Interview Prompts (for when this work begins)

See `/home/mikmorg/.claude/plans/elegant-hugging-cookie.md` sections 1.1 and 1.2 for detailed guided development prompts covering: account lockout, audit logging, MFA/TOTP, JWT key rotation, password reset, file access control, encryption at rest, secrets management, GDPR compliance, and backup strategy.
