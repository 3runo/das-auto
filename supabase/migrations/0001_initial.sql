-- das-auto — initial schema (plan.md §4) + RLS (D5).
--
-- Applied to the live Supabase project on 2026-11-09 (see progress.md T1-3).
-- Idempotency: this file is the canonical history entry; it was applied once
-- to an empty `public` schema. Re-running on a populated schema will fail on
-- `create table` (expected — migrations are forward-only).

create extension if not exists pgcrypto;

-- ---------------------------------------------------------------- vehicles
create table public.vehicles (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  make text not null,
  model text not null,
  version text,
  year int not null check (year between 1950 and 2030),
  plate text not null,
  renavam text,
  fuel_type text not null default 'gasoline'
    check (fuel_type in ('gasoline', 'ethanol', 'flex', 'diesel', 'hybrid', 'electric')),
  state_uo text not null default 'SP',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index vehicles_user_idx on public.vehicles (user_id);

-- ---------------------------------------------------------------- expenses
create table public.expenses (
  id uuid primary key default gen_random_uuid(),
  vehicle_id uuid not null references public.vehicles (id) on delete cascade,
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  category text not null default 'other'
    check (category in ('fuel', 'maintenance', 'insurance', 'tax', 'tire', 'fine', 'other')),
  title text not null,
  amount_cents bigint not null check (amount_cents >= 0),
  occurred_on date not null,
  odometer_km int check (odometer_km between 0 and 3000000),
  location text,
  notes text,
  deleted_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index expenses_vehicle_idx on public.expenses (vehicle_id, occurred_on desc);

-- ---------------------------------------------------------------- documents
create table public.documents (
  id uuid primary key default gen_random_uuid(),
  vehicle_id uuid not null references public.vehicles (id) on delete cascade,
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  name text not null,
  doc_type text not null default 'other'
    check (doc_type in ('crlv', 'insurance', 'registration', 'warranty', 'other')),
  storage_path text not null,
  valid_until date,
  valid_from date,
  deleted_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index documents_vehicle_idx on public.documents (vehicle_id);

-- ---------------------------------------------------------------- deadlines
create table public.deadlines (
  id uuid primary key default gen_random_uuid(),
  vehicle_id uuid not null references public.vehicles (id) on delete cascade,
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  kind text not null default 'other'
    check (kind in ('ipva', 'licensing', 'insurance', 'inspection', 'other')),
  label text not null,
  due_on date not null,
  paid boolean not null default false,
  deep_link text,
  deleted_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index deadlines_vehicle_idx on public.deadlines (vehicle_id, due_on);

-- ------------------------------------------------------ saved_parts_searches
create table public.saved_parts_searches (
  id uuid primary key default gen_random_uuid(),
  vehicle_id uuid not null references public.vehicles (id) on delete cascade,
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  query text not null,
  snapshot jsonb not null default '[]'::jsonb,
  created_at timestamptz not null default now()
);

create index saved_parts_user_idx on public.saved_parts_searches (user_id);

-- ---------------------------------------------------------------- updated_at
create or replace function public.set_updated_at()
returns trigger
language plpgsql
as $$
begin
  new.updated_at := now();
  return new;
end;
$$;

create trigger vehicles_set_updated_at
  before update on public.vehicles
  for each row execute function public.set_updated_at();

create trigger expenses_set_updated_at
  before update on public.expenses
  for each row execute function public.set_updated_at();

create trigger documents_set_updated_at
  before update on public.documents
  for each row execute function public.set_updated_at();

create trigger deadlines_set_updated_at
  before update on public.deadlines
  for each row execute function public.set_updated_at();

-- ---------------------------------------------------------------------- RLS
-- Every user-scoped table: owner-only read/write (plan.md §4, D5).
-- `auth.uid()` comes from Supabase's preinstalled `auth` schema.
alter table public.vehicles enable row level security;
alter table public.expenses enable row level security;
alter table public.documents enable row level security;
alter table public.deadlines enable row level security;
alter table public.saved_parts_searches enable row level security;

create policy vehicles_owner_all on public.vehicles
  for all using (auth.uid() = user_id) with check (auth.uid() = user_id);

create policy expenses_owner_all on public.expenses
  for all using (auth.uid() = user_id) with check (auth.uid() = user_id);

create policy documents_owner_all on public.documents
  for all using (auth.uid() = user_id) with check (auth.uid() = user_id);

create policy deadlines_owner_all on public.deadlines
  for all using (auth.uid() = user_id) with check (auth.uid() = user_id);

create policy saved_parts_owner_all on public.saved_parts_searches
  for all using (auth.uid() = user_id) with check (auth.uid() = user_id);

-- ------------------------------------------------------------ storage bucket
-- Private bucket for user document uploads (T2-2). Bucket creation lives here
-- so the object table's bucket FK exists from day one; the per-user RLS on
-- storage.objects is added in T2-2 (until then `authenticated` has no access,
-- which is the safe default).
insert into storage.buckets (id, name, public)
values ('documents', 'documents', false)
on conflict (id) do nothing;
