-- App Directory — initial schema
-- Every table the application needs for v1. Access control columns are grouped so the
-- effective-access query in `domain::entitlements` stays the single source of truth.

-- ---------------------------------------------------------------- users

create table users (
    id                   uuid primary key default gen_random_uuid(),
    email                text not null,
    name                 text not null,
    department           text,
    password_hash        text,
    must_change_password boolean not null default true,
    role                 text not null default 'user'
                         check (role in ('user', 'app_admin', 'super_admin')),
    admin_scope          jsonb,
    is_active            boolean not null default true,
    failed_attempts      integer not null default 0,
    locked_until         timestamptz,
    last_login_at        timestamptz,
    created_at           timestamptz not null default now(),
    updated_at           timestamptz not null default now()
);

create index users_department_idx on users (department);
create index users_active_idx on users (is_active) where is_active;
-- Emails are stored lower-cased by the application; enforce uniqueness case-insensitively.
create unique index users_email_unique_idx on users (lower(email));

-- ---------------------------------------------------------------- groups

create table groups (
    id              uuid primary key default gen_random_uuid(),
    name            text not null unique,
    description     text,
    auto_department text,
    is_all_users    boolean not null default false,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);

-- Only one built-in "All Users" group may exist.
create unique index groups_single_all_users on groups (is_all_users) where is_all_users;
create index groups_auto_department_idx on groups (auto_department) where auto_department is not null;

create table group_members (
    group_id   uuid not null references groups (id) on delete cascade,
    user_id    uuid not null references users (id) on delete cascade,
    created_at timestamptz not null default now(),
    primary key (group_id, user_id)
);

create index group_members_user_idx on group_members (user_id);

-- ---------------------------------------------------------------- apps

create table apps (
    id              uuid primary key default gen_random_uuid(),
    name            text not null,
    url             text not null,
    description     text,
    category        text,
    color           text,
    sort_order      integer not null default 100,
    is_enabled      boolean not null default true,
    offline_visible boolean not null default true,
    icon_data       bytea,
    icon_mime       text,
    icon_source     text not null default 'none'
                    check (icon_source in ('favicon', 'upload', 'none')),
    icon_fetched_at timestamptz,
    icon_etag       text,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now(),
    deleted_at      timestamptz
);

create index apps_live_idx on apps (sort_order, name) where deleted_at is null;
create index apps_category_idx on apps (category) where deleted_at is null;

create table app_groups (
    app_id     uuid not null references apps (id) on delete cascade,
    group_id   uuid not null references groups (id) on delete cascade,
    created_at timestamptz not null default now(),
    primary key (app_id, group_id)
);

create index app_groups_group_idx on app_groups (group_id);

create table user_apps (
    app_id     uuid not null references apps (id) on delete cascade,
    user_id    uuid not null references users (id) on delete cascade,
    created_at timestamptz not null default now(),
    primary key (app_id, user_id)
);

create index user_apps_user_idx on user_apps (user_id);

-- ---------------------------------------------------------------- sessions

create table sessions (
    id           uuid primary key default gen_random_uuid(),
    user_id      uuid not null references users (id) on delete cascade,
    token_hash   text not null unique,
    created_at   timestamptz not null default now(),
    expires_at   timestamptz not null,
    last_seen_at timestamptz not null default now(),
    revoked_at   timestamptz,
    client_info  text
);

create index sessions_user_idx on sessions (user_id);

-- ---------------------------------------------------------------- password reset

create table reset_codes (
    id         uuid primary key default gen_random_uuid(),
    user_id    uuid not null references users (id) on delete cascade,
    code_hash  text not null,
    purpose    text not null check (purpose in ('onboard', 'reset')),
    expires_at timestamptz not null,
    used_at    timestamptz,
    created_by uuid references users (id) on delete set null,
    created_at timestamptz not null default now()
);

create index reset_codes_user_idx on reset_codes (user_id, used_at);

-- A user clicking "Forgot password" creates one of these for the admin queue.
create table reset_requests (
    id          uuid primary key default gen_random_uuid(),
    user_id     uuid not null references users (id) on delete cascade,
    status      text not null default 'open'
                check (status in ('open', 'issued', 'dismissed')),
    created_at  timestamptz not null default now(),
    resolved_at timestamptz,
    resolved_by uuid references users (id) on delete set null
);

create index reset_requests_status_idx on reset_requests (status, created_at);

-- ---------------------------------------------------------------- configuration + audit

create table settings (
    key        text primary key,
    value      jsonb not null,
    updated_at timestamptz not null default now(),
    updated_by uuid references users (id) on delete set null
);

create table audit_log (
    id            bigserial primary key,
    actor_user_id uuid references users (id) on delete set null,
    action        text not null,
    entity_type   text not null,
    entity_id     uuid,
    before        jsonb,
    after         jsonb,
    ip            inet,
    created_at    timestamptz not null default now()
);

create index audit_log_created_idx on audit_log (created_at desc);
create index audit_log_entity_idx on audit_log (entity_type, entity_id);

-- ---------------------------------------------------------------- sync revision

-- Single-row counter. Any change to what a client sees bumps the revision.
create table sync_state (
    id         smallint primary key default 1 check (id = 1),
    revision   bigint not null default 1,
    updated_at timestamptz not null default now()
);

insert into sync_state (id, revision) values (1, 1) on conflict (id) do nothing;

-- ---------------------------------------------------------------- helpers

create or replace function set_updated_at() returns trigger
language plpgsql as $$
begin
    new.updated_at = now();
    return new;
end;
$$;

create trigger users_set_updated_at before update on users
    for each row execute function set_updated_at();
create trigger groups_set_updated_at before update on groups
    for each row execute function set_updated_at();
create trigger apps_set_updated_at before update on apps
    for each row execute function set_updated_at();
create trigger settings_set_updated_at before update on settings
    for each row execute function set_updated_at();

create or replace function bump_revision() returns bigint
language plpgsql as $$
declare
    next_revision bigint;
begin
    update sync_state
       set revision = revision + 1,
           updated_at = now()
     where id = 1
    returning revision into next_revision;

    return next_revision;
end;
$$;
