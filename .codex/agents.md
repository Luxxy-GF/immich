# Immich Rust Backend Migration

## What We're Doing

We are migrating the **Immich** photo management backend from **TypeScript/NestJS** to a high-performance **Rust/Axum** stack. The new server lives in `/root/immich/server-rs/`. The original NestJS server in `/root/immich/server/` is kept as reference only — all new work targets the Rust codebase.

## Architecture

| Layer | Tech |
|---|---|
| HTTP Framework | Axum 0.7 |
| Database | PostgreSQL 14 via `sqlx` 0.7 (async, compile-time checked) |
| Async Runtime | Tokio |
| Auth | bcrypt password hashing, SHA-256 session tokens, cookie-based sessions |
| Background Jobs | Native `tokio::sync::mpsc` worker (replaces Redis/BullMQ) |
| Vector Support | `tensorchord/pgvecto-rs` Docker image (required for Immich's AI/search features) |

## Key Files

```
server-rs/
├── Cargo.toml
├── .env                          # DB_URL, PORT
├── migrations/
│   └── 0001_baseline.sql         # Full schema dump from legacy TS migrations
└── src/
    ├── main.rs                   # Axum app bootstrap, router registration, AppState
    ├── config.rs                 # Env config loader
    ├── error.rs                  # AppError enum (BadRequest, Unauthorized, InternalServerError)
    ├── crypto.rs                 # SHA-256 hashing, random token generation
    ├── models.rs                 # sqlx::FromRow structs: User, Session, Album, Asset
    ├── jobs.rs                   # Background job queue (tokio mpsc)
    ├── dtos/
    │   ├── mod.rs
    │   ├── auth.rs               # LoginCredentialDto, LoginResponseDto
    │   ├── album.rs              # CreateAlbumDto, UpdateAlbumDto, GetAlbumsDto
    │   └── asset.rs              # AssetBulkDeleteDto, UpdateAssetDto, etc.
    ├── controllers/
    │   ├── mod.rs
    │   ├── auth.rs               # POST /api/auth/login, GET /api/auth/oauth/config
    │   ├── server.rs             # GET /api/server/config, /features, /media-types, /theme
    │   ├── album.rs              # CRUD /api/albums
    │   ├── asset.rs              # CRUD /api/assets, multipart upload
    │   └── user.rs               # GET /api/users/me, GET /api/users/me/preferences
    └── middleware/
        └── auth.rs               # AuthDto extractor (cookie/bearer/header token → session lookup → user)
```

## Database

- **Container**: `immich-postgres` running `tensorchord/pgvecto-rs:pg14-v0.2.0`
- **Port**: 5432
- **Credentials**: `postgres:postgres`
- **Database**: `immich`
- **Schema**: Initialized by running legacy TypeScript Kysely migrations, then captured via `pg_dump --schema-only` into `migrations/0001_baseline.sql`. The Rust server now owns the schema going forward.
- **Table names are singular** (e.g. `"user"`, `"session"`, `"album"`, `"asset"`) — NOT plural.
- **UUIDs**: Postgres stores IDs as native `UUID` type. Our Rust structs use `String`. All SELECT queries must cast: `"id"::text as "id"`. All INSERT/WHERE clauses binding a String to a UUID column must cast: `$1::uuid`.

## Auth Flow

1. `POST /api/auth/login` — receives `{email, password}`, verifies bcrypt hash, generates random token, hashes it with SHA-256, inserts into `"session"` table, returns JSON + Set-Cookie headers (`immich_access_token`, `immich_auth_type`, `immich_is_authenticated`).
2. Subsequent requests — `AuthDto` extractor in middleware reads the cookie/bearer token, SHA-256 hashes it, looks up `"session"`, fetches `"user"`, and injects into handler.

## Web Frontend

- Lives in `/root/immich/web/` (SvelteKit)
- Vite proxy configured in `vite.config.ts` to forward `/api` → `http://127.0.0.1:3002`
- `.env` in web dir: `PUBLIC_IMMICH_SERVER_URL=http://127.0.0.1:3002`
- Run with `pnpm run dev` (port 3000)

## Current Status

### Done
- [x] Core Axum server with AppState, DB pool, tracing
- [x] Auth: login, session creation, cookie handshake, middleware guard
- [x] Server config/features/media-types/theme endpoints
- [x] Album CRUD
- [x] Asset CRUD + multipart upload with SHA1 checksumming
- [x] Background job worker (tokio mpsc)
- [x] Database schema baseline migration
- [x] Admin user provisioned (`me@luxxy.xyz` / `luxxy24`)
- [x] Web frontend proxying to Rust backend
- [x] Set-Cookie headers on login response

### In Progress / TODO
- [ ] `GET /api/users/me` — controller exists but had a compile error (unused import in user.rs needs fix)
- [ ] `GET /api/users/me/preferences` — stubbed, needs real user_metadata table lookup
- [ ] `POST /api/auth/validateToken` — needs implementation
- [ ] `POST /api/auth/logout` — needs implementation
- [ ] Notification/timeline/search endpoints — not yet started
- [ ] Asset serving (thumbnail/original file streaming) — not yet started
- [ ] Machine learning integration endpoints — not yet started
- [ ] System config CRUD (admin panel) — not yet started
- [ ] Cleanup: fix all `cargo check` warnings (unused imports/variables)

## How to Run

```bash
# 1. Start database
docker start immich-postgres
# Or if container doesn't exist:
# docker run -d --name immich-postgres -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=immich -p 5432:5432 tensorchord/pgvecto-rs:pg14-v0.2.0

# 2. Start Rust backend
cd /root/immich/server-rs
cargo run
# Listens on 127.0.0.1:3002

# 3. Start web frontend
cd /root/immich/web
pnpm run dev
# Listens on localhost:3000
```

## Critical Gotchas

1. **UUID ↔ String**: Every SQL query touching UUID columns MUST cast (`::text` on SELECT, `::uuid` on INSERT/WHERE bind). Without this, sqlx panics at runtime with "mismatched types".
2. **Singular table names**: The schema uses `"user"` not `"users"`, `"album"` not `"albums"`, `"asset"` not `"assets"`.
3. **Set-Cookie is mandatory**: The SvelteKit frontend relies on `immich_access_token` cookie for session persistence. Returning only JSON from login will cause the UI to spin forever.
4. **pgvecto-rs required**: Standard Postgres images lack the vector extension Immich needs. Always use `tensorchord/pgvecto-rs`.
