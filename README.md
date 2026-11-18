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
  api/       Rust (Axum) CSV-backed API, also serves the admin SPA at /admin
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
scripts/  icon generator, CSV migration exporter
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
# from apps/api; DATA_DIR defaults to ./data and is created on first start
cargo run
# health: http://127.0.0.1:8090/api/v1/healthz  (reports database: "csv")
# admin:  http://127.0.0.1:8090/admin
```

### API tests

Tests use a temporary CSV directory; no database container is needed.

```powershell
cargo test --manifest-path apps/api/Cargo.toml
```

### Desktop client

The client is a single portable `.exe` that installs nothing. It keeps its state under
`%LOCALAPPDATA%\AppDirectory`:

| Path | Contents |
|---|---|
| `config.json` | schema version, last sync time, cached revision, "shortcuts created" flag |
| `cache.bin` | encrypted snapshot: magic header + AES-256-GCM ciphertext |
| `icons\<app_id>.bin` | downloaded favicons, handed to the webview as data URLs |
| `server.txt` | optional override for the server URL (no rebuild needed) |
| `updates\` | staged update downloaded from the server |

There is **no login screen**: the current Windows username is sent to the server on every
request. The AES key for the offline cache is the only secret, held in Windows Credential
Manager (`AppDirectory` / `snapshot-key`).

- First launch writes `App Directory.lnk` to the desktop and Start menu, never overwriting an
  existing shortcut. Release builds do it automatically; debug builds only with
  `APPDIR_CREATE_SHORTCUTS=1`.
- Sync runs at launch, every `sync_interval_hours` (default 4) and on demand (one manual refresh
  per minute). When the server is unreachable the last snapshot renders with an offline banner
  until `offline_valid_until`; after that a retry screen explains the situation.
- Keyboard: `Ctrl+F` search, `Enter` opens the first match, `Esc` clears.

### Self-update

When the server sets `update_enabled = 1` and an `update_version` in `settings.csv`, the client
shows an **Update now** link. Clicking it downloads the exe from `update_url` (default
`/download/app-directory.exe`), verifies its SHA-256 against `update_sha256`, moves the running
exe aside as `App Directory.old.exe`, installs the new one, starts it and exits. Nothing is
installed and the exe stays portable. A failed checksum leaves the old exe untouched.

To publish a new client: replace `release/App Directory.exe` on the server, compute its SHA-256,
then set `update_enabled`, `update_version` and `update_sha256` via `/api/v1/admin/settings` (or
`POST /api/v1/admin/update/publish`).
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
| 7 | CSV-only backend, trusted `whoami` identity, self-update | ✅ done |
| 8 | 400-user rollout | ⬜ |

## Known risk

**Identity is trusted from `whoami`**: anyone with LAN access can claim another user's Windows
name and see that user's apps. This was accepted deliberately to keep the client sign-in-free;
hardening later (proxy-provided identity, signed device token) needs no UI change.

Transport is still **plain HTTP** while TLS is deferred, and there is no code signing, so
SmartScreen may warn on first run. Do not expose port 8090 beyond the office network.
## Icon

`pnpm icons` draws a placeholder mark and writes every size Tauri needs. Replace
`apps/desktop/src-tauri/icons/*` and the two `favicon.png` files with the real brand icon
when it is supplied.
