begin;

create table public.friendships (
  user_a uuid not null references public.profiles(id) on delete cascade,
  user_b uuid not null references public.profiles(id) on delete cascade,
  requester_id uuid not null references public.profiles(id) on delete cascade,
  status text not null default 'pending',
  accepted_at timestamptz,
  created_at timestamptz not null default clock_timestamp(),
  updated_at timestamptz not null default clock_timestamp(),
  primary key (user_a, user_b),
  constraint friendships_canonical_order check (user_a < user_b),
  constraint friendships_requester_participant check (requester_id in (user_a, user_b)),
  constraint friendships_status check (status in ('pending', 'accepted')),
  constraint friendships_accepted_timestamp check (
    (status = 'pending' and accepted_at is null)
    or (status = 'accepted' and accepted_at is not null)
  )
);

create index friendships_user_b_user_a_idx
on public.friendships (user_b, user_a);

create index friendships_requester_idx
on public.friendships (requester_id);

create table public.blocks (
  blocker_id uuid not null references public.profiles(id) on delete cascade,
  blocked_id uuid not null references public.profiles(id) on delete cascade,
  created_at timestamptz not null default clock_timestamp(),
  primary key (blocker_id, blocked_id),
  constraint blocks_distinct_users check (blocker_id <> blocked_id)
);

create index blocks_blocked_blocker_idx
on public.blocks (blocked_id, blocker_id);

create table public.friend_mutes (
  owner_id uuid not null references public.profiles(id) on delete cascade,
  muted_user_id uuid not null references public.profiles(id) on delete cascade,
  muted boolean not null default true,
  created_at timestamptz not null default clock_timestamp(),
  updated_at timestamptz not null default clock_timestamp(),
  primary key (owner_id, muted_user_id),
  constraint friend_mutes_distinct_users check (owner_id <> muted_user_id)
);

create index friend_mutes_muted_owner_idx
on public.friend_mutes (muted_user_id, owner_id);

create table public.waitlist (
  id bigint generated always as identity primary key,
  email text not null unique,
  created_at timestamptz not null default clock_timestamp(),
  constraint waitlist_email_normalized check (
    email = lower(btrim(email))
    and char_length(email) between 3 and 254
    and email ~ '^[a-z0-9.!#$%&''*+/=?^_`{|}~-]+@[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$'
  )
);

create or replace function private.normalize_friendship_state()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
  if new.status = 'accepted' then
    new.accepted_at := coalesce(new.accepted_at, clock_timestamp());
  else
    new.accepted_at := null;
  end if;
  return new;
end;
$$;

revoke all on function private.normalize_friendship_state()
  from public, anon, authenticated, service_role;

create trigger friendships_normalize_state
before insert or update of status on public.friendships
for each row execute function private.normalize_friendship_state();

create trigger friendships_set_updated_at
before update on public.friendships
for each row execute function private.set_updated_at();

create trigger friend_mutes_set_updated_at
before update on public.friend_mutes
for each row execute function private.set_updated_at();

create or replace function private.is_blocked(p_first uuid, p_second uuid)
returns boolean
language sql
stable
security definer
set search_path = ''
as $$
  select coalesce(
    exists (
      select 1
      from public.blocks
      where (blocker_id = p_first and blocked_id = p_second)
         or (blocker_id = p_second and blocked_id = p_first)
    ),
    false
  );
$$;

create or replace function private.lock_social_pair(p_first uuid, p_second uuid)
returns void
language plpgsql
volatile
security definer
set search_path = ''
as $$
begin
  if p_first is null or p_second is null or p_first = p_second then
    raise exception using errcode = '22023', message = 'invalid_social_pair';
  end if;

  perform pg_advisory_xact_lock(
    hashtextextended(
      least(p_first, p_second)::text || ':' || greatest(p_first, p_second)::text,
      0
    )
  );

  perform 1
  from public.profiles as profile
  where profile.id in (p_first, p_second)
  order by profile.id
  for update;
end;
$$;

create or replace function private.normalize_email(p_email text)
returns text
language sql
immutable
set search_path = ''
as $$
  select case when p_email is null then null else lower(btrim(p_email)) end;
$$;

create or replace function private.valid_email(p_email text)
returns boolean
language sql
immutable
set search_path = ''
as $$
  select coalesce(
    p_email = lower(btrim(p_email))
    and char_length(p_email) between 3 and 254
    and p_email ~ '^[a-z0-9.!#$%&''*+/=?^_`{|}~-]+@[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$',
    false
  );
$$;

revoke all on function private.is_blocked(uuid, uuid)
  from public, anon, authenticated, service_role;
revoke all on function private.lock_social_pair(uuid, uuid)
  from public, anon, authenticated, service_role;
revoke all on function private.normalize_email(text)
  from public, anon, authenticated, service_role;
revoke all on function private.valid_email(text)
  from public, anon, authenticated, service_role;

grant execute on function private.is_blocked(uuid, uuid) to authenticated;

alter table public.friendships enable row level security;
alter table public.blocks enable row level security;
alter table public.friend_mutes enable row level security;
alter table public.waitlist enable row level security;

create policy waitlist_no_direct_access
on public.waitlist
as restrictive
for all
to anon, authenticated
using (false)
with check (false);

create policy friendships_participant_select
on public.friendships
for select
to authenticated
using ((select auth.uid()) in (user_a, user_b));

create policy blocks_owner_select
on public.blocks
for select
to authenticated
using ((select auth.uid()) = blocker_id);

create policy friend_mutes_owner_select
on public.friend_mutes
for select
to authenticated
using ((select auth.uid()) = owner_id);

drop policy profiles_authenticated_select on public.profiles;
create policy profiles_authenticated_select
on public.profiles
for select
to authenticated
using (
  (select auth.uid()) = id
  or not private.is_blocked((select auth.uid()), id)
);

revoke all on public.friendships from public, anon, authenticated;
revoke all on public.blocks from public, anon, authenticated;
revoke all on public.friend_mutes from public, anon, authenticated;
revoke all on public.waitlist from public, anon, authenticated;
revoke all on sequence public.waitlist_id_seq from public, anon, authenticated;

grant select on public.friendships to authenticated;
grant select on public.blocks to authenticated;
grant select on public.friend_mutes to authenticated;

create or replace function public.send_friend_request(p_target_user_id uuid)
returns public.friendships
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
  v_user_a uuid;
  v_user_b uuid;
  v_result public.friendships%rowtype;
begin
  if v_actor is null then
    raise exception using errcode = '28000', message = 'authenticated_actor_required';
  end if;
  if p_target_user_id is null or p_target_user_id = v_actor then
    raise exception using errcode = '22023', message = 'cannot_target_self';
  end if;

  perform private.lock_social_pair(v_actor, p_target_user_id);

  if not exists (select 1 from public.profiles where id = p_target_user_id) then
    raise exception using errcode = 'P0002', message = 'target_user_not_found';
  end if;
  if private.is_blocked(v_actor, p_target_user_id) then
    raise exception using errcode = 'P0001', message = 'blocked_relationship';
  end if;

  v_user_a := least(v_actor, p_target_user_id);
  v_user_b := greatest(v_actor, p_target_user_id);

  select * into v_result
  from public.friendships
  where user_a = v_user_a and user_b = v_user_b
  for update;

  if found then
    if v_result.status = 'accepted' or v_result.requester_id = v_actor then
      return v_result;
    end if;
    raise exception using errcode = 'P0001', message = 'incoming_request_exists';
  end if;

  perform private.consume_rate_limit(
    v_actor,
    'friend_request',
    'all',
    20,
    interval '1 day'
  );

  insert into public.friendships (user_a, user_b, requester_id, status)
  values (v_user_a, v_user_b, v_actor, 'pending')
  returning * into v_result;

  return v_result;
end;
$$;

create or replace function public.accept_friend_request(p_requester_user_id uuid)
returns public.friendships
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
  v_user_a uuid;
  v_user_b uuid;
  v_result public.friendships%rowtype;
begin
  if v_actor is null then
    raise exception using errcode = '28000', message = 'authenticated_actor_required';
  end if;
  if p_requester_user_id is null or p_requester_user_id = v_actor then
    raise exception using errcode = '22023', message = 'cannot_target_self';
  end if;

  perform private.lock_social_pair(v_actor, p_requester_user_id);

  if private.is_blocked(v_actor, p_requester_user_id) then
    raise exception using errcode = 'P0001', message = 'blocked_relationship';
  end if;

  v_user_a := least(v_actor, p_requester_user_id);
  v_user_b := greatest(v_actor, p_requester_user_id);

  select * into v_result
  from public.friendships
  where user_a = v_user_a and user_b = v_user_b
  for update;

  if not found then
    raise exception using errcode = 'P0002', message = 'friendship_not_found';
  end if;
  if v_result.status = 'accepted' then
    return v_result;
  end if;
  if v_result.requester_id <> p_requester_user_id
     or v_result.requester_id = v_actor then
    raise exception using errcode = 'P0001', message = 'not_incoming_request';
  end if;

  perform private.consume_rate_limit(
    v_actor,
    'social_mutation',
    'all',
    60,
    interval '1 hour'
  );

  update public.friendships
  set status = 'accepted', accepted_at = clock_timestamp()
  where user_a = v_user_a and user_b = v_user_b
  returning * into v_result;

  return v_result;
end;
$$;

create or replace function public.unfriend(p_other_user_id uuid)
returns public.friendships
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
  v_user_a uuid;
  v_user_b uuid;
  v_result public.friendships%rowtype;
begin
  if v_actor is null then
    raise exception using errcode = '28000', message = 'authenticated_actor_required';
  end if;
  if p_other_user_id is null or p_other_user_id = v_actor then
    raise exception using errcode = '22023', message = 'cannot_target_self';
  end if;

  perform private.lock_social_pair(v_actor, p_other_user_id);
  v_user_a := least(v_actor, p_other_user_id);
  v_user_b := greatest(v_actor, p_other_user_id);

  select * into v_result
  from public.friendships
  where user_a = v_user_a and user_b = v_user_b
  for update;

  if not found then
    raise exception using errcode = 'P0002', message = 'friendship_not_found';
  end if;

  perform private.consume_rate_limit(
    v_actor,
    'social_mutation',
    'all',
    60,
    interval '1 hour'
  );

  delete from public.friend_mutes
  where (owner_id = v_actor and muted_user_id = p_other_user_id)
     or (owner_id = p_other_user_id and muted_user_id = v_actor);

  delete from public.friendships
  where user_a = v_user_a and user_b = v_user_b
  returning * into v_result;

  return v_result;
end;
$$;

create or replace function public.block_user(p_target_user_id uuid)
returns public.blocks
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
  v_user_a uuid;
  v_user_b uuid;
  v_result public.blocks%rowtype;
begin
  if v_actor is null then
    raise exception using errcode = '28000', message = 'authenticated_actor_required';
  end if;
  if p_target_user_id is null or p_target_user_id = v_actor then
    raise exception using errcode = '22023', message = 'cannot_target_self';
  end if;

  perform private.lock_social_pair(v_actor, p_target_user_id);

  if not exists (select 1 from public.profiles where id = p_target_user_id) then
    raise exception using errcode = 'P0002', message = 'target_user_not_found';
  end if;

  select * into v_result
  from public.blocks
  where blocker_id = v_actor and blocked_id = p_target_user_id
  for update;

  if found then
    return v_result;
  end if;

  perform private.consume_rate_limit(
    v_actor,
    'social_mutation',
    'all',
    60,
    interval '1 hour'
  );

  insert into public.blocks (blocker_id, blocked_id)
  values (v_actor, p_target_user_id)
  returning * into v_result;

  v_user_a := least(v_actor, p_target_user_id);
  v_user_b := greatest(v_actor, p_target_user_id);

  delete from public.friendships
  where user_a = v_user_a and user_b = v_user_b;

  delete from public.friend_mutes
  where (owner_id = v_actor and muted_user_id = p_target_user_id)
     or (owner_id = p_target_user_id and muted_user_id = v_actor);

  return v_result;
end;
$$;

create or replace function public.mute_user(
  p_target_user_id uuid,
  p_muted boolean default true
)
returns public.friend_mutes
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_actor uuid := (select auth.uid());
  v_user_a uuid;
  v_user_b uuid;
  v_result public.friend_mutes%rowtype;
begin
  if v_actor is null then
    raise exception using errcode = '28000', message = 'authenticated_actor_required';
  end if;
  if p_target_user_id is null or p_target_user_id = v_actor then
    raise exception using errcode = '22023', message = 'cannot_target_self';
  end if;
  if p_muted is null then
    raise exception using errcode = '22023', message = 'invalid_mute_state';
  end if;

  perform private.lock_social_pair(v_actor, p_target_user_id);

  if private.is_blocked(v_actor, p_target_user_id) then
    raise exception using errcode = 'P0001', message = 'blocked_relationship';
  end if;

  v_user_a := least(v_actor, p_target_user_id);
  v_user_b := greatest(v_actor, p_target_user_id);

  if not exists (
    select 1
    from public.friendships
    where user_a = v_user_a
      and user_b = v_user_b
      and status = 'accepted'
  ) then
    raise exception using errcode = 'P0001', message = 'accepted_friendship_required';
  end if;

  select * into v_result
  from public.friend_mutes
  where owner_id = v_actor and muted_user_id = p_target_user_id
  for update;

  if found and v_result.muted = p_muted then
    return v_result;
  end if;

  perform private.consume_rate_limit(
    v_actor,
    'social_mutation',
    'all',
    60,
    interval '1 hour'
  );

  insert into public.friend_mutes (owner_id, muted_user_id, muted)
  values (v_actor, p_target_user_id, p_muted)
  on conflict (owner_id, muted_user_id)
  do update set muted = excluded.muted
  returning * into v_result;

  return v_result;
end;
$$;

create or replace function public.join_waitlist(p_email text)
returns public.waitlist
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_email text := private.normalize_email(p_email);
  v_subject text;
  v_result public.waitlist%rowtype;
begin
  if not private.valid_email(v_email) then
    raise exception using errcode = '22023', message = 'invalid_email';
  end if;

  v_subject := encode(extensions.digest(v_email, 'sha256'), 'hex');
  perform private.consume_rate_limit(
    '00000000-0000-0000-0000-000000000000'::uuid,
    'waitlist',
    v_subject,
    5,
    interval '1 hour'
  );

  insert into public.waitlist (email)
  values (v_email)
  on conflict (email) do nothing
  returning * into v_result;

  if not found then
    select * into strict v_result
    from public.waitlist
    where email = v_email;
    return v_result;
  end if;

  return v_result;
end;
$$;

revoke all on function public.send_friend_request(uuid)
  from public, anon, authenticated, service_role;
revoke all on function public.accept_friend_request(uuid)
  from public, anon, authenticated, service_role;
revoke all on function public.unfriend(uuid)
  from public, anon, authenticated, service_role;
revoke all on function public.block_user(uuid)
  from public, anon, authenticated, service_role;
revoke all on function public.mute_user(uuid, boolean)
  from public, anon, authenticated, service_role;
revoke all on function public.join_waitlist(text)
  from public, anon, authenticated, service_role;

grant execute on function public.send_friend_request(uuid) to authenticated;
grant execute on function public.accept_friend_request(uuid) to authenticated;
grant execute on function public.unfriend(uuid) to authenticated;
grant execute on function public.block_user(uuid) to authenticated;
grant execute on function public.mute_user(uuid, boolean) to authenticated;
grant execute on function public.join_waitlist(text) to anon, authenticated;

commit;
