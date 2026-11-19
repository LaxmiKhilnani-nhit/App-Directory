# App Directory

A portable Windows launcher plus a server-side admin portal. Staff sign in automatically with
their Windows username and see a tile grid of the applications they are entitled to. The server
only ever sends links the user is allowed to see; access comes from per-user grants and group
membership managed in the web portal.

Full design history: **[PLAN.md](./PLAN.md)**. Sections of that document that describe
PostgreSQL, passwords, bearer tokens or "no self-update" are superseded by the notes below.

---

## Repository layout

```
apps/
  api/       Rust (Axum) CSV-backed API; serves the admin SPA at /admin
  admin/     React admin portal (Vite)
  desktop/   Tauri v2 + React launcher (Windows)
packages/
  ui/        shared React components and design tokens
  types/     shared API types
  config/    shared tsconfig
infra/
  docker-compose.yml   one API service, no database
  Dockerfile.api       Rust API + admin SPA bundle
  data/                runtime CSV files (gitignored)
  backups/             archived CSV snapshots
  scripts/             backup + VM diagnostics
scripts/               icon generator, CSV migration exporter
```

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Node | ≥ 22 | Node 24 verified |
| pnpm | 12.x | `npm i -g pnpm@12`, or `corepack pnpm` |
| Rust | stable | `rustup default stable` |
| MSVC Build Tools | 2022 | "Desktop development with C++" workload — required by Tauri |
| WebView2 runtime | any | Preinstalled on Windows 11 |
| Docker | 24+ | Server stack (one container) |

## Quick start

```powershell
pnpm install
pnpm icons
pnpm typecheck
```

### API (dev, no Docker)

```powershell
# from apps/api; DATA_DIR defaults to ./data and is created on first start
cargo run
# health: http://127.0.0.1:8090/api/v1/healthz  (reports database: "csv")
# admin:  http://127.0.0.1:8090/admin
```

### Admin portal (dev)

```powershell
pnpm dev:admin      # http://localhost:5173/admin, proxies /api to 127.0.0.1:8090
```

The portal keeps a sign-in screen for administrators. The desktop client does not sign in.

### API tests

Tests use a temporary CSV directory; no database container is needed.

```powershell
cargo test --manifest-path apps/api/Cargo.toml
```

### Desktop client (dev)

```powershell
pnpm dev:desktop            # browser preview at http://localhost:1420
pnpm tauri dev              # real window (needs MSVC + WebView2)
pnpm package:desktop        # release exe at release\App Directory.exe
```

The browser preview falls back to sample data because `invoke` is unavailable outside Tauri.

## Server stack (Docker)

```powershell
docker compose -f infra/docker-compose.yml --env-file .env up -d --build
```

The API binds `0.0.0.0:8090`, including the VM's LAN address `172.168.50.50`. PostgreSQL is
gone; all state is in `infra/data/`. If port 8090 refuses connections, check
`ss -ltnp | grep 8090` on the VM (`bash infra/scripts/check-server.sh` prints everything) before
suspecting the application.

### Backups

```powershell
.\infra\scripts\backup-db.ps1       # archives infra\data into infra\backups\appdir-<stamp>.zip
.\infra\scripts\backup-db.ps1 -KeepDays 14
```

Schedule it on the VM with Task Scheduler. Restore by copying the CSV files back into
`infra/data/` and restarting the container.

### Client download from the server

Place `release\App Directory.exe` on the server in `<repo>/release/`, then recreate the API
container so the folder is mounted. Staff can download it from
`http://172.168.50.50:8090/download/app-directory.exe`; there is a **Download client** link in
the admin sidebar. No image rebuild is needed to publish a new client binary.

## Data model

All state lives in CSV files under `infra/data/` (plus `icons/<app_id>.bin` for icon bytes). The
API loads them into memory, writes through a temporary file plus atomic rename, and reloads files
edited by hand on the next request.

| File | Purpose |
|---|---|
| `users.csv` | Accounts, name, email, department (a department name), role, active flag |
| `departments.csv` | The departments people can belong to |
| `app_departments.csv` | App granted to a department |
| `groups.csv` | Manual user groups and the built-in All Users |
| `group_members.csv` | Explicit group membership |
| `apps.csv` | Tiles: name, URL, category, order, enabled/visibility flags, icon metadata |
| `app_groups.csv` / `user_apps.csv` | Group and direct-user grants |
| `settings.csv` | Runtime config (sync interval, offline grace, announcement, identity, update flag) |
| `audit.csv` | Admin changes with actor, entity and before/after JSON |
| `revision.csv` | Single revision counter for client sync |

The entitlement rule lives in `apps/api/src/domain/entitlements.rs`:

> direct person grant **union** department grant (the user's department) **union** explicit
> group grant, for active users and enabled, non-deleted apps.

There is no built-in "All Users" group: it was removed so an IT user sees only IT apps, plus any
special app granted directly to them (even if that app belongs to another department).

Departments are managed on their own screen, including their apps — add a department and tick
its apps in the same form. The user editor assigns one department from a list; renaming keeps
its people and app grants.

To move data out of the old PostgreSQL stack, run `scripts/export-to-csv.mjs` while the old API
is still running, then switch to the CSV stack.

## Authentication

There is no password, session or onboarding code in the client. On first launch it asks the
user to confirm their work email (pre-filled from the Windows account, since `whoami` is not
always the person's email name) and caches it in
`%LOCALAPPDATA%\AppDirectory\identity.txt`. Every later launch sends that email as
`X-Windows-Identity` with no prompt. **Change email** in the directory footer clears the cache
and asks again. The address is checked against the server before it is saved, so a typo is not
cached.

The server also accepts a `DOMAIN\user` identity and normalises it to `<user>@<mail_domain>`
(default `nhit.co.in`, editable in Settings). If the user exists and is active in `users.csv`,
`/client/sync` returns their apps; otherwise the client shows a "contact IT" screen.

The admin portal keeps its sign-in page, but it is for administrators only: `super_admin` and
`app_admin` accounts can log in, while a plain `user` account gets a 403 from `login` and every
`/admin/*` endpoint. The token is a formality; the desktop client does not sign in at all.

**The username is trusted as supplied.** Anyone on the LAN can claim another user's name and see
their apps. This was accepted deliberately to keep the client sign-in-free. Do not expose port
8090 outside the office network.

## Admin portal screens

| Screen | Route | Contents |
|---|---|---|
| Sign in | `/admin/login` | admin-only entry point |
| Dashboard | `/admin/dashboard` | counts and recent audit entries |
| Applications | `/admin/apps` | table, create/edit drawer, icon upload/refetch, grants |
| Departments | `/admin/departments` | department list; rename or delete (people and app grants follow) |
| Groups | `/admin/groups` | manual user groups; All Users is built in |
| Users & access | `/admin/users` | profile, department, role, direct grants, deactivate |
| CSV import | `/admin/import` | paste CSV, validate, per-row errors, commit |
| Settings | `/admin/settings` | app name, identity mapping, sync/offline, announcement, client update |
| Audit log | `/admin/audit` | filters and expandable before/after JSON |

## Desktop client

The client is a single portable `.exe` that installs nothing. It keeps its state under
`%LOCALAPPDATA%\AppDirectory`:

| Path | Contents |
|---|---|
| `config.json` | schema version, last sync time, cached revision, shortcuts flag |
| `cache.bin` | encrypted snapshot: magic header + AES-256-GCM ciphertext |
| `icons\<app_id>.bin` | cached favicons, handed to the webview as data URLs |
| `server.txt` | optional override for the server URL |
| `updates\` | staged update downloaded from the server |

- Launch renders the encrypted cache immediately; live sync and icon refresh happen in the
  background with short timeouts, so an unreachable server does not delay the tiles.
- The exe is a GUI-subsystem binary with no console attached, and every child process is
  started with `CREATE_NO_WINDOW` — no cmd window appears on the desktop.
- First launch writes desktop and Start-menu shortcuts without overwriting existing ones.
- Sync runs at launch, on the server's `sync_interval_hours` cadence, and on demand (once/minute).
- Offline: last snapshot renders with a banner until `offline_valid_until`, then a retry screen.
- Keyboard: `Ctrl+F` search, `Enter` opens the first match, `Esc` clears.

### Self-update

With `update_enabled = 1` and an `update_version` in `settings.csv`, clients **silently**
download the published exe on launch, verify its SHA-256, and apply it on the **next launch**
(the old exe is kept as `App Directory.old.exe`). No user action is needed and there is no
mid-session restart. A failed download or checksum leaves the old exe untouched and is retried
next time.

The server computes the checksum from the mounted `release\App Directory.exe`, so publishing is
just: replace that file, then set `update_enabled` and `update_version` in Settings.

## Known risk

Identity is trusted from `whoami` and transport is plain HTTP. Hardening (proxy-provided identity,
TLS, code signing) can be added later without changing the UI. Do not expose port 8090 outside
the office network.

## Icon

`pnpm icons` draws a placeholder mark and writes every size Tauri needs. Replace
`apps/desktop/src-tauri/icons/*` and the two `favicon.png` files with the real brand icon when
it is supplied.
