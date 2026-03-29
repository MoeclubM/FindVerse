-- Initial production-grade storage schema scaffold for FindVerse.
-- Phase 1 wires infrastructure and creates stable relational seams.

create extension if not exists pg_trgm;

create table if not exists users (
    id uuid primary key,
    external_id text not null unique,
    username text not null unique,
    role text not null,
    enabled boolean not null default true,
    qps_limit integer not null default 5,
    daily_limit integer not null default 10000,
    created_at timestamptz not null default now()
);

create table if not exists password_credentials (
    user_id uuid primary key references users(id) on delete cascade,
    password_hash text not null,
    password_scheme text not null default 'argon2id',
    password_salt text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists sessions (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    token_hash text not null unique,
    created_at timestamptz not null default now(),
    last_used_at timestamptz not null default now(),
    expires_at timestamptz,
    revoked_at timestamptz
);

create table if not exists api_keys (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    name text not null,
    preview text not null,
    token_hash text not null unique,
    created_at timestamptz not null default now(),
    last_used_at timestamptz,
    revoked_at timestamptz
);

create table if not exists daily_usage (
    user_id uuid not null references users(id) on delete cascade,
    usage_day date not null,
    used_count integer not null default 0,
    primary key (user_id, usage_day)
);

create table if not exists documents (
    id text primary key,
    url text not null unique,
    title text not null,
    display_url text not null,
    snippet text not null,
    body text not null,
    language text not null,
    site_authority real not null default 0,
    suggest_terms jsonb not null default '[]'::jsonb,
    last_crawled_at timestamptz not null,
    search_vector tsvector not null default ''
);

create index if not exists documents_search_vector_idx on documents using gin (search_vector);
create index if not exists documents_url_trgm_idx on documents using gin (url gin_trgm_ops);
create index if not exists documents_title_trgm_idx on documents using gin (title gin_trgm_ops);
create index if not exists documents_last_crawled_at_idx on documents (last_crawled_at desc);

create table if not exists crawlers (
    id text primary key,
    owner_user_id uuid,
    name text not null,
    api_key_hash text,
    created_at timestamptz not null default now(),
    last_seen_at timestamptz not null default now(),
    metadata jsonb not null default '{}'::jsonb
);

create table if not exists crawl_rules (
    id text primary key,
    owner_user_id uuid,
    pattern text not null,
    status text not null,
    max_depth integer not null default 2,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists crawl_jobs (
    id text primary key,
    owner_user_id uuid,
    url text not null,
    depth integer not null default 0,
    status text not null default 'queued',
    claimed_by text,
    claimed_at timestamptz,
    lease_expires_at timestamptz,
    discovered_at timestamptz not null default now(),
    unique (owner_user_id, url)
);

create index if not exists crawl_jobs_claim_idx on crawl_jobs (status, lease_expires_at, discovered_at);

create table if not exists crawl_events (
    id text primary key,
    owner_user_id uuid,
    crawler_id text,
    event_type text not null,
    payload jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);
