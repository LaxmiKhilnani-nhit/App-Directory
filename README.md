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
# from apps/api; DATABASE_URL is only needed from Phase 1 onward
cargo run
# health: http://127.0.0.1:8090/api/v1/healthz
# admin:  http://127.0.0.1:8090/admin
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

## Build status

| Phase | Scope | State |
|---|---|---|
| 0 | Workspace scaffold, compose, API skeleton, Tauri shell | ✅ done |
| 1 | Postgres schema + migrations + entitlement tests | ⬜ next |
| 2 | Auth, sessions, reset codes, audit | ⬜ |
| 3 | Admin API (apps, groups, users, grants, settings) | ⬜ |
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
