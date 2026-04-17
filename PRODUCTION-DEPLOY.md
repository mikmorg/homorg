# Production Deployment Plan

## Context

Currently everything runs on the dev VM (docker-compose with `app` + `db` containers). For production we need: Rust backend, SvelteKit frontend (HTTP only; TLS at external reverse proxy), PostgreSQL with pgvector, and the enricher daemon which shells out to `claude` CLI.

The Claude CLI is the reason full containerization is painful — it needs Node.js, an interactive OAuth login, and persistent `~/.claude/` auth state. Putting it in a container adds complexity with no upside on a single VM.

**Recommendation: single VM, systemd services, Docker only for PostgreSQL.**

## Target Architecture

```
[External Reverse Proxy (HTTPS termination)]
        |
        v  HTTP
[Production VM  (libvirt on home2)]
  nginx (:80)
    ├── /              → static files from /opt/homorg/web/
    ├── /api/*         → proxy_pass http://127.0.0.1:8080
    ├── /files/*       → proxy_pass http://127.0.0.1:8080
    └── /downloads/*   → proxy_pass http://127.0.0.1:8080
  systemd: homorg-api         → /opt/homorg/homorg  (Rust binary, :8080)
  systemd: homorg-enricher    → /opt/homorg/enricher (polls DB, calls claude CLI)
  docker:  postgres:16-pgvector  (:5432, localhost only)
  native:  claude CLI (npm -g, logged in as homorg user)
```

## Why Not Full Docker-Compose

| Concern | Docker-compose | Systemd + Docker DB |
|---------|---------------|---------------------|
| Claude CLI auth | Mount `~/.claude/`, install Node.js in image, re-login on rebuild | Native install, `claude login` once, done |
| Frontend serving | Need nginx sidecar container anyway | nginx native, trivial |
| Rebuild cycle | Full image rebuild (~4 min) for any code change | `rsync` binary + `systemctl restart` (~2 sec) |
| Debugging | `docker logs`, exec into container | `journalctl -u homorg-api`, native tools |
| DB | Same either way (pgvector Docker image) | Same |

## Files to Create

All under a new `deploy/` directory in the repo:

### 1. `deploy/docker-compose.prod.yml` — PostgreSQL only
- `pgvector/pgvector:pg16` container
- Named volume for data persistence
- Bind to `127.0.0.1:5432` (localhost only, no external exposure)
- Init script for extensions (ltree, pg_trgm, uuid-ossp, vector)
- Health check

### 2. `deploy/homorg-api.service` — systemd unit for the backend
- `ExecStart=/opt/homorg/homorg`
- `EnvironmentFile=/opt/homorg/.env`
- `User=homorg`, `Restart=on-failure`
- `After=docker.service` (DB must be up)

### 3. `deploy/homorg-enricher.service` — systemd unit for the enricher daemon
- `ExecStart=/opt/homorg/enricher`
- `EnvironmentFile=/opt/homorg/.env`
- `User=homorg`, `Restart=on-failure`
- `After=homorg-api.service`
- Needs `HOME=/home/homorg` so claude CLI finds `~/.claude/` auth

### 4. `deploy/nginx-homorg.conf` — nginx site config
- Listen :80
- `location /` serves `/opt/homorg/web/` (SvelteKit static build)
- `try_files $uri $uri/ /index.html` (SPA fallback)
- `location /api/`, `/files/`, `/downloads/` proxy to `127.0.0.1:8080`
- Cache headers for static assets (`/_app/immutable/` → long cache, `sw.js` → no-cache)

### 5. `deploy/env.prod.example` — template env file
- All vars from `src/config.rs` with production-appropriate defaults
- `DATABASE_URL=postgres://homorg:<password>@127.0.0.1:5432/homorg`
- `JWT_SECRET=<generate-with-openssl-rand>`
- `STORAGE_PATH=/opt/homorg/data/images`
- `ENRICHMENT_ENABLED=true`
- `CLAUDE_CLI_PATH=/usr/local/bin/claude` (or wherever npm -g puts it)
- `LOG_FORMAT=json`

### 6. `deploy/setup.sh` — one-time VM provisioning script
- Create `homorg` system user
- Install Docker, nginx, Node.js (for claude CLI only)
- `npm install -g @anthropic-ai/claude-code`
- Create `/opt/homorg/` directory structure
- Copy systemd units, enable services
- Start PostgreSQL container, run initial migration
- Remind operator to: `sudo -u homorg claude login`

### 7. `deploy/deploy.sh` — repeatable deploy script (run from dev VM)
- Build backend: `SQLX_OFFLINE=true cargo build --release`
- Build frontend: `cd web && npm run build`
- `rsync` binary + frontend build + migrations to prod VM
- `ssh prod 'sudo systemctl restart homorg-api homorg-enricher'`

## Build & Deploy Workflow

Build happens on the **dev VM** (where Rust toolchain + Node.js already exist). Prod VM stays lean — no compilers, no npm for the app, just runtimes.

```
[Dev VM]                          [Prod VM]
cargo build --release      ──rsync──>  /opt/homorg/homorg
cd web && npm run build    ──rsync──>  /opt/homorg/web/
                                       systemctl restart homorg-api
                                       systemctl restart homorg-enricher
```

## Directory Layout on Prod VM

```
/opt/homorg/
├── homorg              # API server binary
├── enricher            # enricher daemon binary
├── .env                # environment variables
├── web/                # SvelteKit static build (index.html, _app/, etc.)
├── data/
│   └── images/         # uploaded item images
├── downloads/          # APK/release files
└── migrations/         # SQL migrations (run by binary on startup)
```

## Verification

After deployment:
1. `sudo docker compose -f /opt/homorg/docker-compose.prod.yml ps` — DB healthy
2. `systemctl status homorg-api` — active, no errors in `journalctl -u homorg-api`
3. `systemctl status homorg-enricher` — active, polling
4. `curl http://localhost:8080/api/taxonomy` — backend responds
5. `curl http://localhost/` — nginx serves SvelteKit app
6. Hit the external reverse proxy URL — full HTTPS flow works end-to-end
7. Generate a label PDF from admin UI — verifies TeX + backend
8. Submit an enrichment task — verifies claude CLI + enricher daemon
