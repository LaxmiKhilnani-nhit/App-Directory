# App Directory

A portable Windows launcher plus a server-side admin portal. Staff sign in and see a tile
grid of the in-house applications they are entitled to; the server only ever sends links the
user is allowed to see. Access comes from per-user grants and group membership, all managed
from the web portal — no client rebuild for content changes.

Full design, data model, API contract and phases: **[PLAN.md](./PLAN.md)**.

---

## Repository layout

```
apps/
  api/       Rust (Axum + SQLx) API, also serves the admin SPA at /admin
  admin/     React admin portal (Vite)
  desktop/   Tauri v2 + React launcher (Windows)
packages/
  ui/        shared React components and design tokens
  types/     shared API types
  config/    shared tsconfig
infra/       docker-compose, Dockerfile.api, backups
scripts/     placeholder icon generator
```

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Node | ≥ 22 | Node 24 verified |
| pnpm | 12.x | `npm i -g pnpm@12`, or `corepack pnpm` |
| Rust | stable | `rustup default stable` |
| MSVC Build Tools | 2022 | "Desktop development with C++" workload — required by Tauri |
| WebView2 runtime | any | Preinstalled on Windows 11 |
| Docker | 24+ | For the server stack |

## Quick start

```powershell
pnpm install
pnpm icons          # regenerate placeholder icons (already committed)
pnpm typecheck      # tsc across all packages
```

### Admin portal (dev)

```powershell
pnpm dev:admin      # http://localhost:5173/admin, proxies /api to 127.0.0.1:8090
```

### API (dev, no Docker)

```powershell
# from apps/api; DATABASE_URL is optional — without it the API starts with static + health
# endpoints only, so frontend work is unblocked
cargo run
# health: http://127.0.0.1:8090/api/v1/healthz  (includes database status)
# admin:  http://127.0.0.1:8090/admin
```

### API tests

Integration tests need a real PostgreSQL instance; `#[sqlx::test]` creates and drops a
throw-away database per test.

```powershell
docker run -d --name appdir-test-db -p 127.0.0.1:55432:5432 `
  -e POSTGRES_PASSWORD=test postgres:16-alpine

$env:DATABASE_URL = "postgres://postgres:test@127.0.0.1:55432/postgres"
cargo test --manifest-path apps/api/Cargo.toml
```

### Desktop client (dev)

```powershell
pnpm dev:desktop            # browser preview at http://localhost:1420
pnpm tauri dev              # real window (needs MSVC + WebView2)
```

### Server stack (Docker)

```powershell
Copy-Item .env.example .env    # then edit the password
docker compose -f infra/docker-compose.yml up -d --build
```

The API is published on `API_BIND_IP:8090` (default `127.0.0.1` for local testing; use the
VM's LAN IP — `172.168.50.50` — in production). PostgreSQL has **no** host port; it is
reachable only from the API container.

## Configuration

Runtime settings for the client are baked at build time in
`apps/desktop/src-tauri/src/config.rs` (`DEFAULT_API_BASE_URL`). Set `APPDIR_API_BASE` to
override during local development.

Server environment variables are documented in `.env.example`. Secrets live in `.env`, which
is gitignored.

## Data model

Schema lives in `apps/api/migrations/0001_init.sql` and is applied automatically on startup.

| Table | Purpose |
|---|---|
| `users` | Accounts, role (`user` / `app_admin` / `super_admin`), department, lockout counters |
| `groups` | Manual groups, auto-by-department groups, and the single built-in **All Users** group |
| `group_members` | Explicit group membership |
| `apps` | Tiles: name, URL, category, order, enabled flag, icon bytes |
| `app_groups` | App granted to a group |
| `user_apps` | App granted directly to one user |
| `sessions` | Opaque bearer tokens (hashed), expiry, revocation |
| `reset_codes` / `reset_requests` | Admin-issued reset codes and the queue of "forgot password" requests |
| `settings` | Runtime config (sync interval, offline grace, announcement, lockout policy) |
| `audit_log` | Every admin change with actor, entity and before/after |
| `sync_state` | Single-row revision counter for client sync |

Effective access is defined once, in `apps/api/src/domain/entitlements.rs`:

> direct grant **∪** grants via any group the user belongs to (explicit membership,
> department match, or the All Users group), for active users and enabled, non-deleted apps.

`tests/entitlements.rs` covers each branch of that rule against a real PostgreSQL instance.

## Authentication

Auth is a bearer-token scheme, implemented in `apps/api/src/domain/auth.rs` and exposed by
`apps/api/src/http/auth.rs`. There is no AD/LDAP: accounts are created by admins.

| Method | Path | Notes |
|---|---|---|
| `POST` | `/api/v1/auth/login` | `{email, password}` → `{token, expiresAt, user, mustChangePassword}` |
| `GET` | `/api/v1/auth/me` | revalidates a token and returns the current user |
| `POST` | `/api/v1/auth/logout` | revokes the session used for the call |
| `POST` | `/api/v1/auth/change-password` | `{currentPassword, newPassword}`; other sessions are revoked |
| `POST` | `/api/v1/auth/forgot` | always `202`; queues an admin-visible reset request |
| `POST` | `/api/v1/auth/reset` | `{email, code, newPassword}`; consumes a single-use admin code |

Details that matter in practice:

- Tokens are 32 random bytes (hex). Only the SHA-256 hash is stored in `sessions`, so a
  database copy cannot be replayed as a live session. TTL is `SESSION_TTL_DAYS` (default 30).
- Passwords use Argon2id. Unknown emails still pay one hash worth of time, so login latency
  does not reveal whether an account exists.
- Failed logins increment `users.failed_attempts`; at `login_lockout_attempts` (default 5) the
  account is locked for `login_lockout_minutes` (default 15). The lock is disclosed as `423`
  because telling a user to contact IT is more useful than hiding it.
- Reset codes are 8 characters from an unambiguous alphabet, shown as `XXXX-XXXX`, hashed at
  rest, valid for 72 hours and single use. Input is case-, dash- and space-insensitive.
- Forgot-password never enumerates: known and unknown addresses both return `202`.
- `change-password` keeps the calling session alive and revokes every other one; a reset
  revokes all sessions for that user.
- Per-IP sliding-window rate limits guard login, forgot and reset. Account lockout is the
  durable defence; the limiter is in-memory and resets with the process.
- `audit_log` records login, logout, password change and password reset. Failed sign-ins are
  logged with `tracing` only, so a brute-force run cannot flood the admin-visible table.

All failures use the uniform envelope: `{"error": {"code": "…", "message": "…"}}`.

## Build status

| Phase | Scope | State |
|---|---|---|
| 0 | Workspace scaffold, compose, API skeleton, Tauri shell | ✅ done |
| 1 | Postgres schema + migrations + entitlement tests | ✅ done |
| 2 | Auth, sessions, reset codes, audit | ✅ done |
| 3 | Admin API (apps, groups, users, grants, settings) | ⬜ next |
| 4 | Admin SPA screens | ⬜ |
| 5 | Client sync, cache, shortcuts, directory UI | ⬜ |
| 6 | Packaging, clean-VM test, Intune | ⬜ |
| 7 | 400-user rollout | ⬜ |

## Known risk (temporary)

Transport is **plain HTTP** until the SSL rollout (see PLAN.md §8 and §11). Passwords cross
the LAN in cleartext. Mitigations in place: LAN-only bind, login lockout/rate limits,
Argon2id hashing, no password reuse across systems. Do not expose the API beyond the office
network until TLS is enabled.

## Icon

`pnpm icons` draws a placeholder mark and writes every size Tauri needs. Replace
`apps/desktop/src-tauri/icons/*` and the two `favicon.png` files with the real brand icon
when it is supplied.
