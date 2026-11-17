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

The browser preview falls back to a sample snapshot because `invoke` is unavailable outside
Tauri; add `?screen=login` or `?screen=change-password` to that URL to review the other screens.

A live end-to-end check (login, sync, icon download, encrypted cache round-trip) sits behind
`#[ignore]` because it needs the dev stack:

```powershell
$env:APPDIR_API_BASE = "http://127.0.0.1:8090"
$env:APPDIR_E2E_EMAIL = "admin@example.com"
$env:APPDIR_E2E_PASSWORD = "<value from .env>"
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --ignored
```

Add `APPDIR_E2E_KEEP=1` to leave the token and encrypted cache in `%LOCALAPPDATA%\AppDirectory`
so a packaged exe can be launched offline against them.

### Portable client (single exe)

```powershell
pnpm package:desktop
```

This runs `tauri build --no-bundle`, so the output is the bare binary rather than an installer.
It lands at `release\App Directory.exe` — one file, nothing to install. Copy it anywhere and
run it; the first launch writes `%LOCALAPPDATA%\AppDirectory`, the encrypted snapshot and the
desktop/Start-menu shortcuts. `--debug` skips LTO for a fast structural check.

### Server stack (Docker)

```powershell
Copy-Item .env.example .env    # then edit the password
docker compose -f infra/docker-compose.yml --env-file .env up -d --build
```

Compose reads `.env` from the compose file's directory, so `--env-file .env` is required when
running from the repo root (or `--env-file ../.env` from inside `infra/`).

### Backups

```powershell
.\infra\scripts\backup-db.ps1            # writes infra\backups\appdir-<timestamp>.dump
.\infra\scripts\backup-db.ps1 -KeepDays 14
```

The script takes a `pg_dump` in custom format through the running container and prunes
anything older than 30 days. PostgreSQL compresses the custom format, so no gzip step is
needed. Schedule it on the VM host with Task Scheduler:

```powershell
schtasks /create /tn "App Directory backup" /sc daily /st 01:00 /tr "powershell -ExecutionPolicy Bypass -File <repo>\infra\scripts\backup-db.ps1"
```

To restore into a scratch database (verified during the pre-rollout pass):

```powershell
docker run -d --name appdir-restore -e POSTGRES_PASSWORD=restore -e POSTGRES_USER=postgres -e POSTGRES_DB=appdir postgres:16-alpine
docker cp infra\backups\appdir-<timestamp>.dump appdir-restore:/tmp/restore.dump
docker exec -e PGPASSWORD=restore appdir-restore pg_restore -U postgres -d appdir --no-owner --no-privileges /tmp/restore.dump
```

A restored dump boots the API unchanged: `/healthz` answers `database: ok`, the seeded
settings and the administrator's password both survive, and `/admin/` is served.

The API is published on port 8090 on **all** host interfaces (`0.0.0.0:8090:8090`), including
the VM's LAN address `172.168.50.50`. PostgreSQL has **no** host port; it is reachable only
from the API container. Restrict 8090 with the host firewall if the VM has more than one network.

If a browser on another machine gets "connection refused", first confirm the container was
**recreated** after pulling a compose change (`docker compose ... up -d --force-recreate api`),
then check `ss -ltnp | grep 8090` on the VM and finally `sudo ufw status`.

### Onboarding email

SMTP is configured from **Settings → Onboarding email** in the admin portal. When enabled, a
new account's one-time onboarding code is emailed immediately; if delivery fails, the code is
still returned and shown to the administrator, so onboarding never depends on the relay. The
reset/forgot flow is unchanged — the user still sets a new password on first sign-in.

The SMTP password is stored in the server's `settings` table. It is masked in every API
response, in the audit log and in the portal, and submitting the mask means "unchanged". Treat
the mailbox password as a secret and rotate it if it has been shared in plain text.

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

## Admin API

Every route below requires a bearer token for an account that has completed its first-login
password change. `app_admin` may work only inside the categories in `admin_scope.categories`;
everything outside that scope answers `403 out_of_scope`.

| Method | Path | Who | Notes |
|---|---|---|---|
| `GET` | `/api/v1/admin/me` | admin | current user plus `isSuperAdmin` |
| `GET` | `/api/v1/admin/stats` | admin | dashboard counters and current revision |
| `GET`/`POST` | `/api/v1/admin/apps` | admin | list is scope-filtered |
| `GET`/`PATCH`/`DELETE` | `/api/v1/admin/apps/{id}` | admin | delete is a soft delete |
| `POST` | `/api/v1/admin/apps/{id}/icon` | admin | raw image body (PNG/JPEG/ICO/GIF/WebP/SVG), ≤512 KB |
| `POST` | `/api/v1/admin/apps/{id}/refetch-icon` | admin | fetches `<origin>/favicon.ico` |
| `PUT` | `/api/v1/admin/apps/{id}/groups` | admin | replaces group grants |
| `PUT` | `/api/v1/admin/apps/{id}/users` | admin | replaces direct user grants |
| `GET` | `/api/v1/admin/groups` | admin | read-only view for grant pickers |
| `POST` | `/api/v1/admin/groups` | super | |
| `GET`/`PATCH`/`DELETE` | `/api/v1/admin/groups/{id}` | super | All Users group cannot be renamed or deleted |
| `PUT` | `/api/v1/admin/groups/{id}/members` | super | refused for derived groups |
| `GET`/`POST` | `/api/v1/admin/users` | super | search, role and active filters |
| `POST` | `/api/v1/admin/users/import` | super | CSV; returns per-row errors and onboarding codes |
| `GET`/`PATCH`/`DELETE` | `/api/v1/admin/users/{id}` | super | delete deactivates and revokes sessions |
| `PUT` | `/api/v1/admin/users/{id}/apps` | super | replaces direct app grants |
| `POST` | `/api/v1/admin/users/{id}/reset-code` | super | `{purpose: "onboard"|"reset"}` |
| `GET` | `/api/v1/admin/reset-requests` | super | `?status=open|issued|dismissed` |
| `POST` | `/api/v1/admin/reset-requests/{id}/issue` | super | issues and resolves in one step |
| `POST` | `/api/v1/admin/reset-requests/{id}/dismiss` | super | |
| `GET`/`PUT` | `/api/v1/admin/settings` | super | validated keys only |
| `GET` | `/api/v1/admin/audit` | admin | filters: actor, entityType, action, from, to |
| `GET` | `/api/v1/client/sync` | user | full entitled snapshot (see PLAN.md §5.1) |
| `GET` | `/api/v1/icons/{app_id}` | user | ETag + `304`; entitled users and any admin |

Every mutation writes an `audit_log` row and bumps `sync_state.revision` in the same
transaction, so a client never observes a half-applied change. `PATCH` bodies distinguish
"field absent" (unchanged) from `null` (clear the nullable column). The last active
super admin cannot be demoted or deactivated, and nobody can deactivate their own account.

## Admin portal screens

The SPA is built into the API image (`infra/Dockerfile.api`) and served from `/admin`, so the
portal and the API share an origin and no CORS configuration is needed.

| Screen | Route | Who | Notes |
|---|---|---|---|
| Sign in / change password | `/admin/login`, `/admin/change-password` | anyone | first-login gate before any admin screen |
| Dashboard | `/admin/dashboard` | admin | scoped counters for `app_admin`; its activity feed is limited to app changes |
| Applications | `/admin/apps` | admin | search/filter table, create/edit drawer with tile preview, icon upload or refetch, group and direct-user grants |
| Groups | `/admin/groups` | super | manual membership picker, auto-by-department groups, built-in All Users is view-only |
| Users & access | `/admin/users` | super | profile, role and admin scope, direct grants, onboarding/reset codes, deactivate |
| CSV import | `/admin/import` | super | paste or upload, column mapping, client-side preview, per-row errors and one-time onboarding codes |
| Reset queue | `/admin/reset-requests` | super | issue a code or dismiss, filtered by status |
| Settings | `/admin/settings` | super | app name, sync interval, offline grace, password policy, announcement banner, SMTP onboarding email and test send |
| Audit log | `/admin/audit` | super | filters plus expandable before/after JSON |

`app_admin` accounts only see Dashboard and Apps; every other route redirects to `/admin/apps`,
matching the API's own `403 out_of_scope` answers. The client-side preview in the CSV screen is
advisory - the server validates again and commits row by row.

## Desktop client

The client is a single portable `.exe` that installs nothing and keeps its state under
`%LOCALAPPDATA%\AppDirectory`:

| Path | Contents |
|---|---|
| `config.json` | schema version, last sync time, cached revision, "shortcuts created" flag |
| `cache.bin` | encrypted snapshot: magic header + AES-256-GCM ciphertext |
| `icons\<app_id>.bin` | downloaded favicons, handed to the webview as data URLs |

- The session token and the snapshot key live in Windows Credential Manager (service
  `AppDirectory`, accounts `session` and `snapshot-key`); neither is written to a file.
- First launch writes `App Directory.lnk` to the desktop and the Start menu and never overwrites
  an existing shortcut. Release builds do this automatically; debug builds only with
  `APPDIR_CREATE_SHORTCUTS=1`.
- Signing out deletes the snapshot and icons, so a shared machine never shows the previous
  user's list offline.
- Sync runs at launch, every `sync_interval_hours` (default 4) and on demand (one manual refresh
  per minute). When the server is unreachable the last snapshot renders with an offline banner
  until `offline_valid_until`; after that the login screen reports when the last sync happened.
- Keyboard: `Ctrl+F` search, `Enter` opens the first match, `Esc` clears.
- `must_change_password` routes the user to the change-password screen before the grid.

## Pre-rollout checks (2026-11-17)

Measured against the dev stack with the packaged exe before the 400-user rollout:

| Check | Result |
|---|---|
| CSV import, 400 new users plus 10 bad rows | 400 created, 10 row errors, ~13 s |
| Users list at 400 accounts | 12 ms |
| User search | 10 ms |
| Group membership write | 16 ms |
| One group grant | ~9 ms |
| `/client/sync` for a user with 60 apps | 18.5 kB, 7-16 ms |
| Offline cold start, server stopped | window opens, `cache.bin` left untouched |

Two defects the pass turned up, both fixed: the CSV import rejected `All Users` and
auto-by-department groups as unknown names, and the grant endpoints that replace a whole
collection silently treated a mistyped field as "empty" instead of rejecting it. A 400-row
import takes roughly 13 seconds, so day one should be started from the portal and allowed to
finish.

## Build status

| Phase | Scope | State |
|---|---|---|
| 0 | Workspace scaffold, compose, API skeleton, Tauri shell | ✅ done |
| 1 | Postgres schema + migrations + entitlement tests | ✅ done |
| 2 | Auth, sessions, reset codes, audit | ✅ done |
| 3 | Admin API (apps, groups, users, grants, settings) | ✅ done |
| 4 | Admin SPA screens | ✅ done |
| 5 | Client sync, cache, shortcuts, directory UI | ✅ done |
| 6 | Packaging: portable `.exe`, first-run shortcuts | 🟡 built and smoke-tested; clean-VM check + Intune deferred |
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
