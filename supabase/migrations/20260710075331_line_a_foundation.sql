begin;

create schema private;
revoke all on schema private from public, anon, authenticated, service_role;

create extension if not exists pgcrypto with schema extensions;

grant usage on schema public to anon, authenticated;

create or replace function private.valid_github_handle(p_handle text)
returns boolean
language sql
immutable
set search_path = ''
as $$
  select coalesce(
    p_handle = lower(btrim(p_handle))
    and char_length(p_handle) between 1 and 39
    and p_handle ~ '^[a-z0-9]+(-[a-z0-9]+)*$',
    false
  );
$$;

create or replace function private.valid_https_url(p_url text)
returns boolean
language sql
immutable
set search_path = ''
as $$
  select coalesce(
    p_url is null
    or (
      char_length(p_url) between 9 and 2048
      and p_url ~* '^https://([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(:[0-9]{1,5})?([/?#][^[:space:][:cntrl:]]*)?$'
    ),
    false
  );
$$;

create or replace function private.valid_private_snapshot(p_snapshot jsonb)
returns boolean
language plpgsql
immutable
set search_path = ''
as $$
declare
  v_hunger numeric;
  v_level numeric;
  v_streak numeric;
begin
  if p_snapshot is null
     or jsonb_typeof(p_snapshot) <> 'object'
     or (select count(*) from jsonb_object_keys(p_snapshot)) <> 7
     or not (p_snapshot ?& array[
       'petId', 'spriteState', 'mood', 'hunger', 'level', 'streak', 'away'
     ]) then
    return false;
  end if;

  if jsonb_typeof(p_snapshot -> 'petId') <> 'string'
     or (p_snapshot ->> 'petId') !~ '^[a-z0-9][a-z0-9._-]{0,63}$'
     or jsonb_typeof(p_snapshot -> 'spriteState') <> 'string'
     or (p_snapshot ->> 'spriteState') not in (
       'idle', 'walk', 'run', 'sleep', 'work', 'waiting', 'compacting', 'happy', 'eat'
     )
     or jsonb_typeof(p_snapshot -> 'mood') <> 'string'
     or (p_snapshot ->> 'mood') not in ('neutral', 'happy', 'sleepy', 'focused')
     or jsonb_typeof(p_snapshot -> 'hunger') <> 'number'
     or jsonb_typeof(p_snapshot -> 'level') <> 'number'
     or jsonb_typeof(p_snapshot -> 'streak') <> 'number'
     or jsonb_typeof(p_snapshot -> 'away') <> 'boolean' then
    return false;
  end if;

  begin
    v_hunger := (p_snapshot ->> 'hunger')::numeric;
    v_level := (p_snapshot ->> 'level')::numeric;
    v_streak := (p_snapshot ->> 'streak')::numeric;
  exception
    when others then
      return false;
  end;

  return v_hunger = trunc(v_hunger)
    and v_hunger between 0 and 100
    and v_level = trunc(v_level)
    and v_level between 1 and 100
    and v_streak = trunc(v_streak)
    and v_streak between 0 and 3650;
end;
$$;

create or replace function private.valid_event_payload(p_kind text, p_params jsonb)
returns boolean
language plpgsql
immutable
set search_path = ''
as $$
declare
  v_days numeric;
begin
  if p_kind is null
     or p_params is null
     or jsonb_typeof(p_params) <> 'object'
     or (select count(*) from jsonb_object_keys(p_params)) <> 1 then
    return false;
  end if;

  case p_kind
    when 'task_completed' then
      return p_params ? 'source'
        and jsonb_typeof(p_params -> 'source') = 'string'
        and (p_params ->> 'source') in ('cc', 'codex', 'cursor');
    when 'egg_hatched', 'souvenir_found' then
      return p_params ? 'rarity'
        and jsonb_typeof(p_params -> 'rarity') = 'string'
        and (p_params ->> 'rarity') in ('common', 'rare', 'legendary');
    when 'streak_milestone' then
      if not (p_params ? 'days')
         or jsonb_typeof(p_params -> 'days') <> 'number' then
        return false;
      end if;
      begin
        v_days := (p_params ->> 'days')::numeric;
      exception
        when others then
          return false;
      end;
      return v_days = trunc(v_days) and v_days between 1 and 3650;
    else
      return false;
  end case;
end;
$$;

create table private.rate_limits (
  actor_id uuid not null,
  action text not null,
  subject text not null,
  window_seconds bigint not null,
  window_started_at timestamptz not null,
  request_count integer not null default 1,
  updated_at timestamptz not null default clock_timestamp(),
  primary key (actor_id, action, subject, window_seconds, window_started_at),
  constraint rate_limits_action_shape check (
    char_length(action) between 1 and 64
    and action ~ '^[a-z][a-z0-9_]*$'
  ),
  constraint rate_limits_subject_length check (char_length(subject) between 1 and 128),
  constraint rate_limits_window_bounds check (window_seconds between 1 and 2592000),
  constraint rate_limits_count_positive check (request_count >= 1)
);

create index rate_limits_cleanup_idx
on private.rate_limits (window_started_at);

alter table private.rate_limits enable row level security;
revoke all on private.rate_limits from public, anon, authenticated, service_role;

create policy rate_limits_no_client_access
on private.rate_limits
as restrictive
for all
to anon, authenticated
using (false)
with check (false);

create or replace function private.consume_rate_limit(
  p_actor uuid,
  p_action text,
  p_subject text,
  p_max_count integer,
  p_window interval
)
returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_window_seconds bigint;
  v_window_started_at timestamptz;
  v_count integer;
begin
  v_window_seconds := extract(epoch from p_window)::bigint;

  if p_actor is null
     or p_action is null
     or p_action !~ '^[a-z][a-z0-9_]{0,63}$'
     or p_subject is null
     or char_length(p_subject) not between 1 and 128
     or p_max_count is null
     or p_max_count < 1
     or v_window_seconds not between 1 and 2592000 then
    raise exception using errcode = '22023', message = 'invalid_rate_limit_configuration';
  end if;

  v_window_started_at := date_bin(
    p_window,
    clock_timestamp(),
    timestamptz '2000-01-01 00:00:00+00'
  );

  insert into private.rate_limits as limits (
    actor_id,
    action,
    subject,
    window_seconds,
    window_started_at,
    request_count,
    updated_at
  )
  values (
    p_actor,
    p_action,
    p_subject,
    v_window_seconds,
    v_window_started_at,
    1,
    clock_timestamp()
  )
  on conflict (actor_id, action, subject, window_seconds, window_started_at)
  do update
    set request_count = limits.request_count + 1,
        updated_at = clock_timestamp()
    where limits.request_count < p_max_count
  returning request_count into v_count;

  if v_count is null then
    raise exception using errcode = 'P0001', message = 'rate_limit_exceeded';
  end if;
end;
$$;

revoke all on function private.valid_github_handle(text) from public, anon, authenticated, service_role;
revoke all on function private.valid_https_url(text) from public, anon, authenticated, service_role;
revoke all on function private.valid_private_snapshot(jsonb) from public, anon, authenticated, service_role;
revoke all on function private.valid_event_payload(text, jsonb) from public, anon, authenticated, service_role;
revoke all on function private.consume_rate_limit(uuid, text, text, integer, interval)
  from public, anon, authenticated, service_role;

create table public.profiles (
  id uuid primary key references auth.users(id) on delete cascade,
  handle text not null unique,
  display_name text,
  avatar_url text,
  created_at timestamptz not null default clock_timestamp(),
  updated_at timestamptz not null default clock_timestamp(),
  constraint profiles_handle_format check (
    handle = lower(btrim(handle))
    and char_length(handle) between 1 and 39
    and handle ~ '^[a-z0-9]+(-[a-z0-9]+)*$'
  ),
  constraint profiles_display_name_format check (
    display_name is null
    or (
      display_name = btrim(display_name)
      and char_length(display_name) between 1 and 64
      and display_name !~ '[[:cntrl:]]'
      and position(chr(8234) in display_name) = 0
      and position(chr(8235) in display_name) = 0
      and position(chr(8236) in display_name) = 0
      and position(chr(8237) in display_name) = 0
      and position(chr(8238) in display_name) = 0
      and position(chr(8294) in display_name) = 0
      and position(chr(8295) in display_name) = 0
      and position(chr(8296) in display_name) = 0
      and position(chr(8297) in display_name) = 0
    )
  ),
  constraint profiles_avatar_https check (
    avatar_url is null
    or (
      char_length(avatar_url) between 9 and 2048
      and avatar_url ~* '^https://([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(:[0-9]{1,5})?([/?#][^[:space:][:cntrl:]]*)?$'
    )
  )
);

create table public.pets (
  user_id uuid primary key references auth.users(id) on delete cascade,
  snapshot jsonb not null default
    '{"petId":"yoonie","spriteState":"idle","mood":"neutral","hunger":100,"level":1,"streak":0,"away":false}'::jsonb,
  connector_seen_at timestamptz,
  updated_at timestamptz not null default clock_timestamp(),
  constraint pets_snapshot_object check (
    jsonb_typeof(snapshot) = 'object'
    and snapshot ?& array[
      'petId', 'spriteState', 'mood', 'hunger', 'level', 'streak', 'away'
    ]
    and snapshot - array[
      'petId', 'spriteState', 'mood', 'hunger', 'level', 'streak', 'away'
    ] = '{}'::jsonb
  )
);

create table public.events (
  id bigint generated always as identity primary key,
  user_id uuid not null default auth.uid() references auth.users(id) on delete cascade,
  kind text not null,
  params jsonb not null,
  occurred_at timestamptz not null default clock_timestamp(),
  created_at timestamptz not null default clock_timestamp(),
  constraint events_kind_dictionary check (
    kind in ('task_completed', 'egg_hatched', 'souvenir_found', 'streak_milestone')
  ),
  constraint events_params_object check (jsonb_typeof(params) = 'object')
);

create index events_user_occurred_idx
on public.events (user_id, occurred_at desc);

create or replace function private.enforce_pet_snapshot()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
begin
  if not private.valid_private_snapshot(new.snapshot) then
    raise exception using
      errcode = '23514',
      message = 'new row for relation "pets" violates check constraint "pets_snapshot_valid"';
  end if;
  if tg_op = 'UPDATE' and v_actor is not null then
    perform private.consume_rate_limit(
      v_actor,
      'snapshot_update',
      'self',
      120,
      interval '1 minute'
    );
  end if;
  return new;
end;
$$;

create trigger pets_validate_snapshot
before insert or update of snapshot on public.pets
for each row execute function private.enforce_pet_snapshot();

create or replace function private.enforce_event_payload()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
begin
  if not private.valid_event_payload(new.kind, new.params) then
    raise exception using
      errcode = '23514',
      message = 'new row for relation "events" violates check constraint "events_valid_payload"';
  end if;
  if new.occurred_at < clock_timestamp() - interval '30 days'
     or new.occurred_at > clock_timestamp() + interval '5 minutes' then
    raise exception using errcode = '22023', message = 'invalid_event_timestamp';
  end if;
  if tg_op = 'INSERT' and v_actor is not null then
    perform private.consume_rate_limit(
      v_actor,
      'event_insert',
      'self',
      120,
      interval '1 minute'
    );
  end if;
  return new;
end;
$$;

create trigger events_validate_payload
before insert or update of kind, params on public.events
for each row execute function private.enforce_event_payload();

create or replace function private.set_updated_at()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
  new.updated_at := clock_timestamp();
  return new;
end;
$$;

create or replace function private.enforce_profile_update_rate()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
begin
  if v_actor is not null then
    perform private.consume_rate_limit(
      v_actor,
      'profile_update',
      'self',
      60,
      interval '1 hour'
    );
  end if;
  return new;
end;
$$;

create trigger profiles_rate_limit_update
before update on public.profiles
for each row execute function private.enforce_profile_update_rate();

create trigger profiles_set_updated_at
before update on public.profiles
for each row execute function private.set_updated_at();

create trigger pets_set_updated_at
before update on public.pets
for each row execute function private.set_updated_at();

revoke all on function private.enforce_pet_snapshot() from public, anon, authenticated, service_role;
revoke all on function private.enforce_event_payload() from public, anon, authenticated, service_role;
revoke all on function private.set_updated_at() from public, anon, authenticated, service_role;
revoke all on function private.enforce_profile_update_rate()
  from public, anon, authenticated, service_role;

create or replace function private.provision_auth_user(
  p_user_id uuid,
  p_app_metadata jsonb,
  p_user_metadata jsonb
)
returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_candidate text;
  v_fallback text;
  v_avatar text;
  v_is_github boolean := coalesce(
    p_app_metadata ->> 'provider' = 'github'
      and jsonb_typeof(p_app_metadata -> 'providers') = 'array'
      and (p_app_metadata -> 'providers') ? 'github',
    false
  );
begin
  if p_user_id is null then
    raise exception using errcode = '22023', message = 'user_id_required';
  end if;

  if v_is_github then
    v_candidate := lower(btrim(coalesce(
      p_user_metadata ->> 'user_name',
      p_user_metadata ->> 'preferred_username',
      ''
    )));
    v_avatar := coalesce(
      p_user_metadata ->> 'avatar_url',
      p_user_metadata ->> 'picture'
    );
  else
    v_candidate := '';
    v_avatar := null;
  end if;

  if not private.valid_github_handle(v_candidate) then
    v_candidate := 'user-' || replace(p_user_id::text, '-', '');
  end if;

  if not private.valid_https_url(v_avatar) then
    v_avatar := null;
  end if;

  insert into public.profiles (id, handle, avatar_url)
  values (p_user_id, v_candidate, v_avatar)
  on conflict do nothing;

  if not exists (select 1 from public.profiles where id = p_user_id) then
    v_fallback := 'user-' || replace(p_user_id::text, '-', '');

    insert into public.profiles (id, handle, avatar_url)
    values (p_user_id, v_fallback, v_avatar)
    on conflict (id) do nothing;
  end if;

  insert into public.pets (user_id)
  values (p_user_id)
  on conflict (user_id) do nothing;
end;
$$;

create or replace function private.handle_auth_user_created()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  perform private.provision_auth_user(
    new.id,
    coalesce(new.raw_app_meta_data, '{}'::jsonb),
    coalesce(new.raw_user_meta_data, '{}'::jsonb)
  );
  return new;
end;
$$;

revoke all on function private.provision_auth_user(uuid, jsonb, jsonb)
  from public, anon, authenticated, service_role;
revoke all on function private.handle_auth_user_created()
  from public, anon, authenticated, service_role;

create trigger pawbae_provision_auth_user
after insert on auth.users
for each row execute function private.handle_auth_user_created();

select private.provision_auth_user(
  id,
  coalesce(raw_app_meta_data, '{}'::jsonb),
  coalesce(raw_user_meta_data, '{}'::jsonb)
)
from auth.users;

alter table public.profiles enable row level security;
alter table public.pets enable row level security;
alter table public.events enable row level security;

create policy profiles_authenticated_select
on public.profiles
for select
to authenticated
using (true);

create policy profiles_owner_update
on public.profiles
for update
to authenticated
using ((select auth.uid()) = id)
with check ((select auth.uid()) = id);

create policy pets_owner_select
on public.pets
for select
to authenticated
using ((select auth.uid()) = user_id);

create policy pets_owner_update
on public.pets
for update
to authenticated
using ((select auth.uid()) = user_id)
with check ((select auth.uid()) = user_id);

create policy events_owner_select
on public.events
for select
to authenticated
using ((select auth.uid()) = user_id);

create policy events_owner_insert
on public.events
for insert
to authenticated
with check ((select auth.uid()) = user_id);

revoke all on public.profiles from public, anon, authenticated;
revoke all on public.pets from public, anon, authenticated;
revoke all on public.events from public, anon, authenticated;
revoke all on sequence public.events_id_seq from public, anon, authenticated;

grant select on public.profiles to authenticated;
grant update (handle, display_name, avatar_url) on public.profiles to authenticated;

grant select on public.pets to authenticated;
grant update (snapshot) on public.pets to authenticated;

grant select on public.events to authenticated;
grant insert (kind, params, occurred_at) on public.events to authenticated;
grant usage on sequence public.events_id_seq to authenticated;

create or replace function public.connector_heartbeat()
returns public.pets
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
  v_result public.pets%rowtype;
begin
  if v_actor is null then
    raise exception using errcode = '28000', message = 'authenticated_actor_required';
  end if;

  perform private.consume_rate_limit(
    v_actor,
    'connector_heartbeat',
    'self',
    2,
    interval '1 minute'
  );

  update public.pets
  set connector_seen_at = clock_timestamp()
  where user_id = v_actor
  returning * into v_result;

  if not found then
    raise exception using errcode = 'P0002', message = 'pet_not_found';
  end if;

  return v_result;
end;
$$;

revoke all on function public.connector_heartbeat()
  from public, anon, authenticated, service_role;
grant execute on function public.connector_heartbeat() to authenticated;

commit;
