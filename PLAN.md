# App Directory — Implementation Plan

A portable Windows launcher plus a server-side admin portal that gives ~400 staff a single
tile grid of the in-house applications they are entitled to, with access controlled by the
server, group and per-user grants, and an offline-capable client.

---

## 1. Product summary

| | |
|---|---|
| **Product name** | App Directory |
| **Client** | Tauri v2 + React + TypeScript, Windows x64 (fleet is Windows 11) |
| **Admin portal** | React SPA, served by the same Rust API at `/admin` |
| **API** | Rust (Axum + SQLx) |
| **Database** | PostgreSQL 16 (Docker, internal network only) |
| **Hosting** | Docker Compose on one Linux VM, `http://172.168.50.50:8090` |
| **Transport** | Plain HTTP for now (see §11 — accepted risk, TLS later) |
| **Users / apps** | ~400 users, 10–15 apps |
| **Distribution** | Single `.exe`, Intune (email delivery is a fallback with caveats) |

### Goals

1. A portable exe users can run without installation or admin rights, which creates its own
   desktop and Start-menu shortcuts on first run.
2. Tiles of authorised in-house apps; clicking opens the URL in the default browser.
3. Server-enforced entitlement filtering: a user never receives a URL they are not entitled to.
4. Admin-managed apps/groups/users/grants via a web portal — no client rebuild for content changes.
5. Client refreshes its list on launch, periodically, and on demand; works offline from an
   encrypted local snapshot.
6. Track and audit all admin changes.

### Explicitly out of scope for v1

- Single sign-on / passing the user's credentials to downstream apps (seam left for later).
- Client self-update (content updates are server-side; the exe is rebuilt rarely).
- Uploaded custom icons — favicon auto-fetch only (upload is designed in, UI deferred).
- Click/usage analytics.
- TLS, code signing, MSIX packaging.
- Multi-tenancy — one organisation, one directory.

---

## 2. Decisions log

| # | Decision | Rationale |
|---|---|---|
| D1 | "Portable" applies to the client only; all state is server-side | An exe cannot be the source of truth for access control |
| D2 | App-managed accounts (no AD/LDAP) | No directory available to bind against |
| D3 | Enforced filtering server-side | Only entitled URLs are ever transmitted |
| D4 | Favicon fetched server-side and cached | Works offline; no client-side scraping |
| D5 | Admin portal served by the API process | No Caddy, no extra web server, no CORS |
| D6 | Favicon + no Caddy + plain HTTP | Per client instruction; revisit with SSL rollout |
| D7 | Sync = full snapshot per call (~5 KB) plus revision counter | 15 apps makes delta sync pointless complexity |
| D8 | SMTP onboarding email (supersedes the no-relay decision) → admin-issued one-time reset codes | No relay available on the network |
| D9 | Snapshot encrypted locally; token held in Windows Credential Manager | Copying `%LOCALAPPDATA%` must not leak links |
| D10 | Built-in `All Users` group | Implements "grant to everyone" without a special case |
| D11 | No "copy link" button in the client | Undermines the directory's purpose |
| D12 | Trust the Windows username from `whoami`; no password, token or enrolment | Explicitly accepted to keep the client sign-in-free; anyone on the LAN can impersonate (see §11) |
| D13 | CSV files instead of PostgreSQL | Simpler deployment and backup; no database service in compose |
| D14 | Portable self-update when `update_enabled = 1`, verified by SHA-256 | Keeps the single-exe model while allowing server-published versions |
| D15 | Departments are first-class; apps are granted to departments, groups and people | Replaces "auto-by-department groups" as the way to grant to a whole department |
| D16 | No built-in All Users group; admin portal restricted to admin roles | A user sees only their department, their groups and direct grants; staff cannot reach admin screens |

The sections below describe the original design. Where they mention SQLx, PostgreSQL, bearer
tokens, passwords, onboarding codes or "no self-update", D12–D14 now supersede them. The
implementation is documented in README.md.

---

## 3. Repository layout

```
app-directory/
├─ PLAN.md
├─ README.md
├─ package.json                  # pnpm workspaces root
├─ pnpm-workspace.yaml
├─ .env.example                  # compose + API env template
├─ apps/
│  ├─ api/                       # Rust, Axum + SQLx
│  │  ├─ Cargo.toml
│  │  ├─ migrations/             # sqlx migrations (0001_init.sql, …)
│  │  └─ src/
│  │     ├─ main.rs
│  │     ├─ config.rs
│  │     ├─ error.rs             # AppError -> JSON error envelope
│  │     ├─ auth/                # login, sessions, reset codes, password policy
│  │     ├─ domain/              # apps, groups, users, entitlements, settings
│  │     ├─ admin/               # admin-only handlers, roles/scope checks
│  │     ├─ client/              # /client/sync, /icons
│  │     ├─ favicon.rs           # fetch + normalise + store icons
│  │     ├─ audit.rs
│  │     └─ static_assets.rs     # ServeDir for admin SPA + SPA fallback
│  ├─ admin/                     # React SPA (Vite)
│  │  └─ src/
│  │     ├─ pages/               # dashboard, apps, groups, users, settings, audit, login
│  │     ├─ api/                 # typed fetch client
│  │     └─ components/
│  └─ desktop/                   # Tauri v2 app
│     ├─ src-tauri/
│     │  ├─ Cargo.toml
│     │  ├─ tauri.conf.json
│     │  ├─ icons/               # placeholder icon until supplied
│     │  └─ src/
│     │     ├─ main.rs
│     │     ├─ api_client.rs     # reqwest, all server calls happen here
│     │     ├─ cache.rs          # AES-GCM snapshot, DPAPI-equivalent key via keyring
│     │     ├─ shortcuts.rs      # IShellLink desktop + Start menu creation
│     │     ├─ config.rs         # baked API base URL
│     │     └─ commands.rs       # Tauri commands exposed to React
│     └─ src/                    # React UI
│        ├─ pages/               # login, directory
│        └─ components/
├─ packages/
│  ├─ ui/                        # shared React components (Tile, Button, Table, Badge…)
│  ├─ types/                     # TS types generated from Rust via ts-rs
│  └─ config/                    # shared tsconfig / eslint / prettier
└─ infra/
   ├─ docker-compose.yml
   ├─ Dockerfile.api
   └─ backups/                   # pg_dump output volume
```

### Toolchain prerequisites (current machine state)

- Node 24 — present ✅
- Docker 29 — present ✅
- **Rust toolchain — missing**, install via `rustup` (stable) + MSVC build tools
- **pnpm — missing**, enable via `corepack enable pnpm`
- Tauri CLI (`cargo install tauri-cli` or dev-dependency)

---

## 4. Data model

All tables in schema `public`, `id` as `uuid` unless noted, `created_at`/`updated_at`
`timestamptz not null default now()`.

### `users`

| column | type | notes |
|---|---|---|
| id | uuid pk | |
| email | citext unique not null | login identifier |
| name | text not null | display name |
| department | text null | drives auto-groups |
| password_hash | text null | null until first login sets a password |
| must_change_password | bool not null default true | forced on first login / after reset |
| role | text not null default 'user' | `user` \| `app_admin` \| `super_admin` |
| admin_scope | jsonb null | for `app_admin`: `{ "categories": [], "group_ids": [] }` |
| is_active | bool not null default true | deactivation = hard deny on next sync |
| failed_attempts | int not null default 0 | |
| locked_until | timestamptz null | |
| last_login_at | timestamptz null | |

### `groups`

| column | type | notes |
|---|---|---|
| id | uuid pk | |
| name | text unique not null | |
| description | text null | |
| auto_department | text null | when set, membership is derived from `users.department` |
| is_all_users | bool not null default false | built-in, membership implicit |

### `group_members`

`group_id` → `groups.id` (cascade), `user_id` → `users.id` (cascade), PK `(group_id, user_id)`.
Only used for manual groups (`auto_department is null and not is_all_users`).

### `apps`

| column | type | notes |
|---|---|---|
| id | uuid pk | |
| name | text not null | tile label |
| url | text not null | `http://172.168.50.50:8080/…` |
| description | text null | shown on hover / secondary line |
| category | text null | free text, admin-managed |
| color | text null | `#RRGGBB`, used for letter-tile fallback |
| sort_order | int not null default 100 | admin-defined ordering |
| is_enabled | bool not null default true | hidden from everyone when false |
| offline_visible | bool not null default true | false = never written to the client snapshot |
| icon_data | bytea null | normalised PNG (256×256) |
| icon_mime | text null | |
| icon_source | text not null default 'none' | `favicon` \| `upload` \| `none` |
| icon_fetched_at | timestamptz null | |
| icon_etag | text null | for client cache validation |
| deleted_at | timestamptz null | soft delete, keeps audit history |

### `app_groups` / `user_apps`

`(app_id, group_id)` and `(app_id, user_id)` join tables, both cascade on delete.

### `sessions`

`id` uuid pk, `user_id`, `token_hash` text unique (SHA-256 of a 32-byte random token),
`created_at`, `expires_at` (30 days), `last_seen_at`, `revoked_at`, `client_info` text.

### `reset_codes`

`id` uuid pk, `user_id`, `code_hash` text, `purpose` text (`onboard` \| `reset`),
`expires_at` (24 h), `used_at`, `created_by` uuid → `users.id`.

### `reset_requests`

Created when a user clicks "Forgot password" — the admin queue.
`id`, `user_id`, `created_at`, `resolved_at`, `resolved_by`, `status` (`open` \| `issued` \| `dismissed`).

### `settings`

`key` text pk, `value` jsonb, `updated_at`, `updated_by`. Single source for tunables (§9.5).

### `audit_log`

`id` bigserial pk, `actor_user_id`, `action` text, `entity_type` text, `entity_id` uuid,
`before` jsonb, `after` jsonb, `ip` inet, `created_at`. Append-only.

### `sync_state`

Single row (`id = 1`), `revision` bigint not null default 1. Bumped by any change that
affects what a client sees (app create/update/delete/disable, grant change, group membership
change, user deactivation, settings change affecting the snapshot).

### Effective access query (the core rule)

```sql
-- enabled apps for user $1 = direct grants ∪ group grants ∪ All Users
select a.*
from apps a
where a.deleted_at is null
  and a.is_enabled
  and (
    exists (select 1 from user_apps ua where ua.app_id = a.id and ua.user_id = $1)
    or exists (
      select 1
      from app_groups ag
      join groups g on g.id = ag.group_id
      where ag.app_id = a.id
        and (
          g.is_all_users
          or g.auto_department is not null
             and g.auto_department = (select department from users where id = $1)
          or exists (select 1 from group_members gm
                     where gm.group_id = g.id and gm.user_id = $1)
        )
    )
  )
order by a.sort_order, a.name;
```

This query is the single implementation of access control; it lives in one module
(`domain::entitlements`) and is unit-tested against fixture data.

---

## 5. API contract (v1)

Base path `/api/v1`. All responses JSON. Errors use a uniform envelope:
`{ "error": { "code": "invalid_credentials", "message": "…" } }`.
Auth is `Authorization: Bearer <token>`.

### Public / client

| Method | Path | Notes |
|---|---|---|
| `GET` | `/healthz` | liveness for compose/uptime checks |
| `POST` | `/auth/login` | `{email, password}` → `{token, expires_at, user, must_change_password}` |
| `POST` | `/auth/change-password` | auth, `{current_password, new_password}`; revokes other sessions |
| `POST` | `/auth/logout` | auth; revokes current session |
| `POST` | `/auth/forgot` | `{email}` → `202` always; queues a `reset_request` (no user enumeration) |
| `POST` | `/auth/reset` | `{email, code, new_password}` → consumes code, clears lock |
| `GET` | `/client/sync?cursor=<revision>` | auth; full snapshot for the caller (§5.1) |
| `GET` | `/icons/:app_id` | PNG; `ETag` + `If-None-Match` → `304` |

#### 5.1 Sync payload

```json
{
  "revision": 42,
  "server_time": "2026-11-16T10:00:00Z",
  "offline_valid_until": "2026-11-23T10:00:00Z",
  "snapshot_days": 7,
  "announcement": { "text": "Payroll app moves to a new URL on Monday", "level": "info" },
  "user": { "name": "…", "department": "Finance", "must_change_password": false },
  "apps": [
    {
      "id": "…",
      "name": "Payroll",
      "url": "http://172.168.50.50:8080/payroll",
      "description": "Monthly payroll processing",
      "category": "Finance",
      "color": "#2F6FED",
      "sort_order": 10,
      "offline_visible": true,
      "icon_url": "/api/v1/icons/…",
      "icon_etag": "w/\"abc123\"",
      "updated_at": "2026-11-10T09:00:00Z"
    }
  ]
}
```

`cursor` is accepted for forward compatibility but the payload is always full — the client
diffs locally by `id` + `updated_at`. `removed` apps simply disappear from the array and the
client drops them (and their cached icons) on save.

### Admin (role-gated: `super_admin`, or `app_admin` within `admin_scope`)

| Area | Endpoints |
|---|---|
| Session | `GET /admin/me` |
| Dashboard | `GET /admin/stats` (users, apps, groups, syncs today, open reset requests) |
| Apps | `GET/POST /admin/apps`, `GET/PATCH/DELETE /admin/apps/:id`, `POST /admin/apps/:id/icon` (upload), `POST /admin/apps/:id/refetch-icon`, `PUT /admin/apps/:id/groups`, `PUT /admin/apps/:id/users` |
| Groups | `GET/POST /admin/groups`, `PATCH/DELETE /admin/groups/:id`, `PUT /admin/groups/:id/members` |
| Users | `GET/POST /admin/users`, `PATCH /admin/users/:id`, `POST /admin/users/import` (CSV), `POST /admin/users/:id/reset-code`, `PUT /admin/users/:id/apps` (bulk set), `DELETE /admin/users/:id` (soft deactivate) |
| Reset queue | `GET /admin/reset-requests`, `POST /admin/reset-requests/:id/issue`, `POST /admin/reset-requests/:id/dismiss` |
| Settings | `GET/PUT /admin/settings` |
| Audit | `GET /admin/audit?actor=&entity_type=&from=&to=` |

Every mutating admin endpoint writes an `audit_log` row and bumps `sync_state.revision`.

### Static hosting

`ServeDir` on `/assets`, SPA fallback to `index.html` for `/admin/*`. One container, one port,
no CORS configuration required.

---

## 6. Client behaviour

### 6.1 Local files

```
%LOCALAPPDATA%\AppDirectory\
├─ config.json      # schema version, last cursor, last sync time, shortcut-created flag
├─ cache.bin        # AES-256-GCM encrypted snapshot (apps + metadata)
├─ icons\<app_id>.png
└─ logs\            # rotating; no URLs or tokens in logs
```

- Encryption key: 32 random bytes stored in **Windows Credential Manager** (target
  `AppDirectory/snapshot-key`) via the `keyring` crate; regenerate after cache wipe.
- Session token: also in Credential Manager (`AppDirectory/session`), never on disk in plain
  text, never passed into the webview.
- Snapshot holds no `offline_visible = false` apps.

### 6.2 Launch sequence

1. Enforce single instance (named mutex / Tauri single-instance plugin); focus the existing window.
2. First run only: create desktop shortcut (`%USERPROFILE%\Desktop\App Directory.lnk`) and
   Start-menu entry (`%APPDATA%\Microsoft\Windows\Start Menu\Programs\App Directory.lnk`),
   `IconLocation` = exe path index 0. Never overwrite an existing shortcut.
3. Read token from Credential Manager.
   - No token → login screen.
   - Token → attempt `/client/sync`.
     - **Success** → validate revision, re-download changed icons only (ETag), persist
       encrypted snapshot, render grid.
     - **Network failure** → load snapshot if `now < offline_valid_until`; render read-only with
       an offline banner (tiles with `offline_visible = false` are hidden, not clickable).
       If expired → login screen with "Offline — last synced …" message.
     - **401** → clear token, login screen (keep snapshot for later re-login, still gated).
4. Start background sync timer (default 4 h, server-driven).

### 6.3 Sync cadence

| Trigger | Interval |
|---|---|
| Launch | always |
| Background while window open | `settings.sync_interval_hours` (default 4) |
| Manual refresh button | user-initiated, rate-limited to 1/minute |
| Network reconnect | event-driven |
| Snapshot offline validity | `settings.offline_grace_days` (default 7) |

Offline revocation is fundamentally limited: a disconnected client cannot learn it was
revoked. Revocation therefore takes effect at the **next successful sync** (launch, ≤4 h while
running, or reconnect). Per-app `offline_visible = false` is the tool for links that must never
sit on a disk. This limitation is documented in the README and surfaced to the admin in the UI
copy.

### 6.4 UI

**Login** — app name, email, password, "Forgot password" link, inline errors, server
reachability indicator, offline notice when the server is unreachable.

**Directory** — header (title, search, sync status, user menu), announcement banner (if any),
category chips, responsive tile grid, empty state ("No apps assigned yet — contact IT"),
footer ("Last synced 12 minutes ago" + refresh).

Tiles: icon (favicon) or letter tile coloured by `apps.color`; name; description on hover.
Click → `open::that(url)` in the default browser (Rust side). No copy-link control.
Keyboard: `Ctrl+F` focuses search, `Enter` opens the first match, `Esc` clears.
Search filters on name + description + category, client-side over the cached list.

`must_change_password = true` routes the user to a change-password screen before the grid.

---

## 7. Admin portal

SPA at `http://172.168.50.50:8090/admin`, same design tokens as the client via `packages/ui`.

| Screen | Contents |
|---|---|
| **Login** | email + password |
| **Dashboard** | counts (users/apps/groups), open reset requests, recent audit entries, sync revision |
| **Apps** | table (icon, name, category, URL, enabled, groups, users, sort); create/edit drawer with live tile preview, "Refetch icon" and "Upload icon"; entitlements tab (group checkboxes + user search); soft delete |
| **Groups** | list with type badge (manual / auto-department / All Users); create/edit; member picker with search; read-only for `All Users` |
| **Users** | table with search + filters (department, group, role, active); create user (returns one-time onboarding code); edit; role assignment with scope; grant matrix (checkbox per app) with "apply group suggestions"; issue reset code; deactivate |
| **CSV import** | upload → column mapping → validated preview (valid / error rows listed) → commit; generates onboarding codes for new users, downloadable once as a table |
| **Reset requests** | queue of "Forgot password" clicks; "Issue code" shows a one-time code (copy button), expiry timestamp, resolves the row |
| **Settings** | app display name, sync interval hours, offline grace days, announcement text/level/active window |
| **Audit** | filterable append-only log (actor, action, entity, before/after, time) |

Role handling: `super_admin` sees everything; `app_admin` sees only apps in scoped categories
and may manage grants for scoped groups. Navigation hides what the role cannot reach.

---

## 8. Security posture

Because TLS is deferred (D6), the following mitigations are mandatory in v1:

- API binds to the VM's LAN address only (`API_BIND_IP`), never a public interface.
- `postgres` has no host port mapping; reachable only from the `api` container.
- Passwords: Argon2id (default params), minimum 12 characters, checked against a small
  common-password deny list, no forced rotation.
- Login: 5 failures → 15-minute lock; per-IP and per-email rate limits
  (`tower-governor` + a DB-backed attempt counter); identical error for unknown email vs wrong
  password; constant-ish response time.
- Sessions: 32-byte random opaque tokens, only SHA-256 hashes stored; 30-day expiry, sliding
  `last_seen_at`; revoked on logout, password change, deactivation, or admin "sign out
  everywhere".
- Reset codes: 6 groups of digits/letters, hashed at rest, 24 h expiry, single use,
  rate-limited, invalidates existing sessions on use.
- Admin endpoints: role check + scope check + audit on every mutation.
- Client: token and snapshot key in Credential Manager; snapshot AES-GCM; logs never contain
  URLs, tokens, or app names.
- Secrets (`POSTGRES_PASSWORD`, bootstrap admin password) in `.env`, `.env` gitignored,
  `.env.example` committed.
- `README` carries an explicit "known risk: plaintext transport, accepted temporarily" note
  with the migration steps for TLS later (baked base URL + cert trust).

---

## 9. Operational specifics

### 9.1 Compose topology

```yaml
services:
  db:    # postgres:16-alpine, no ports, volume pgdata, healthcheck
  api:   # build infra/Dockerfile.api, depends_on db healthy,
         # ports: "${API_BIND_IP:-172.168.50.50}:8090:8090"
         # volumes: ./backups:/backups
```

### 9.2 Environment variables

| Var | Purpose |
|---|---|
| `DATABASE_URL` | postgres connection string (compose-internal) |
| `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` | db init |
| `API_BIND_IP` | LAN address the port binds to (default `172.168.50.50`) |
| `BOOTSTRAP_ADMIN_EMAIL` / `BOOTSTRAP_ADMIN_PASSWORD` | seeded once on first start |
| `APP_NAME` | display name default (`App Directory`) |
| `SESSION_TTL_DAYS` | default 30 |
| `RUST_LOG` | `info` |

### 9.3 Bootstrap

On container start: run `sqlx migrate run`; if no `super_admin` exists, create one from
`BOOTSTRAP_ADMIN_*` with `must_change_password = true`; ensure the `All Users` group exists;
ensure `settings` has default rows.

### 9.4 Backups

Nightly `pg_dump` from a cron sidecar / host cron into `infra/backups/` (gzip, 30-day
retention). Admin portal also offers CSV export of apps, groups, users and grants.

### 9.5 Settings keys (admin-editable, DB-backed)

| Key | Default | Effect |
|---|---|---|
| `app_name` | `App Directory` | client header + admin title |
| `sync_interval_hours` | `4` | client background sync |
| `offline_grace_days` | `7` | snapshot validity |
| `announcement` | `null` | `{text, level, active_from, active_to}` banner |
| `password_min_length` | `12` | reset/change validation |
| `login_lockout_attempts` / `login_lockout_minutes` | `5` / `15` | |

### 9.6 CSV import format

```csv
name,email,department,groups
Rahul Sharma,rahul@example.com,Finance,Finance;Reporting
```

Groups semicolon-separated; unknown groups are reported as errors (no silent creation);
duplicate emails reported; valid rows imported, new users get `must_change_password = true`
and an onboarding code shown once, or emailed to the user when SMTP is configured.

---

## 10. Build phases

Each phase ends with something demonstrable. Estimated effort assumes one developer.

### Phase 0 — Scaffold (0.5 day)

- Install Rust toolchain + MSVC build tools; `corepack enable pnpm`.
- pnpm workspace, `packages/{ui,types,config}`, lint/format/tsconfig.
- `infra/` compose + Dockerfile.api + `.env.example`; empty Axum app with `/healthz`.
- Tauri v2 skeleton in `apps/desktop` with placeholder icon; verify `cargo tauri dev` on Win11.
- **Done when:** `docker compose up` serves `/healthz`; Tauri window opens with React.

### Phase 1 — Data layer (1 day)

- `0001_init.sql` migrations for all tables + indexes.
- SQLx models, `domain::entitlements` with unit tests covering direct/group/auto-department/
  All Users/disabled-app cases.
- Bootstrap seeding; `settings` defaults.

### Phase 2 — Auth (1.5 days)

- login / logout / change-password / forgot / reset; Argon2id; sessions; lockout; rate limits.
- `audit_log` write path; uniform error envelope.
- Integration tests against a disposable Postgres (docker).

### Phase 3 — Admin API (2 days)

- Apps CRUD + icon fetch/upload + entitlement wiring.
- Groups CRUD incl. auto-department + All Users.
- Users CRUD + grant matrix + reset code issuing + CSV import.
- Settings, stats, audit query, revision bumping on every mutation.

### Phase 4 — Admin SPA (3 days)

- Login, shell/navigation, dashboard.
- Apps table + drawer (tile preview, icon actions, entitlements tab).
- Groups, Users (grant matrix, role scope), CSV import wizard, reset queue, settings, audit.
- Role/scope-aware navigation.

### Phase 5 — Client (3 days)

- Rust: reqwest sync client, Credential Manager token, AES-GCM cache, icon cache with ETag,
  shortcut creation, single instance, `open::that`.
- React: login, change-password, directory grid, search, category filter, announcement,
  offline banner, sync status, manual refresh, background timer.
- **Done when:** end-to-end against a seeded local server, including a cold start with the
  server stopped (offline snapshot renders).

### Phase 6 — Packaging & pilot (1 day)

- `cargo tauri build`; extract the portable single exe; verify on a clean Win11 VM.
- First-run shortcut creation verified (desktop + Start menu, correct icon, no duplicates).
- Intune Win32 packaging notes (`IntuneWinAppUtil`, install/uninstall commands, detection rule).
- Deploy to 5 pilot users → collect feedback → fix.

### Phase 7 — Rollout (1 day + buffer)

- Import 400 users, issue onboarding codes in batches through the admin portal.
- Distribute via Intune; publish a one-page "how to log in / reset" note.
- Monitor logs and audit for the first week.

**Total ≈ 12–13 working days**, plus rollout buffer.

---

## 11. Known risks & deferred work

| Risk | Mitigation now | Later |
|---|---|---|
| Passwords in cleartext on LAN | LAN-only bind, lockout, rate limits, no reuse prompts | TLS + cert trust before broad rollout |
| Unsigned exe triggers SmartScreen | Intune deployment avoids Mark-of-the-Web | Code-signing certificate |
| Offline revocation gap | 7-day cap, `offline_visible=false` per app, 4 h sync | Shorter grace or per-app TTL if needed |
| SMTP configured | Onboarding codes emailed; portal display remains the fallback | SMTP relay → true self-service email |
| Favicon fetch fails on authenticated internal apps | Letter-tile fallback with `apps.color` | Custom icon upload UI |
| VM moves → client rebuild (baked URL) | Single `config.rs` constant | Server discovery / redirect |
| Admin lockout (sole super_admin forgets password) | CLI `reset-admin` subcommand on the VM | Second super_admin account in practice |

Design seams left intentionally: a `?sso=<signed-token>` parameter appended to app URLs
(future credential pass-through), an `icon_source = 'upload'` path already in the schema, and
`sync cursor` accepted but unused (future delta sync).

---

## 12. Immediate next actions

1. Install Rust + MSVC build tools, enable pnpm via corepack.
2. Phase 0 scaffold (workspace, compose, empty API, Tauri shell).
3. Phase 1 migrations and entitlement tests.

**Open items awaiting input from the client:** product icon (512×512 PNG / `.ico`) — placeholder
used until then; final `.exe` name confirmation (`App Directory.exe`).
