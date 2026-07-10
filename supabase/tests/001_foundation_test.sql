begin;
select * from no_plan();

select has_table('public', 'profiles', 'profiles table exists');
select has_table('public', 'pets', 'pets table exists');
select has_table('public', 'events', 'events table exists');
select col_is_pk('public', 'profiles', 'id', 'profiles.id is the primary key');
select col_is_pk('public', 'pets', 'user_id', 'pets.user_id is the primary key');
select col_is_pk('public', 'events', 'id', 'events.id is the primary key');
select col_is_fk('public', 'profiles', 'id', 'profiles.id references auth.users');
select col_is_fk('public', 'pets', 'user_id', 'pets.user_id references auth.users');
select col_is_fk('public', 'events', 'user_id', 'events.user_id references auth.users');

select ok(
  (select relrowsecurity from pg_class where oid = 'public.profiles'::regclass),
  'profiles has RLS enabled'
);
select ok(
  (select relrowsecurity from pg_class where oid = 'public.pets'::regclass),
  'pets has RLS enabled'
);
select ok(
  (select relrowsecurity from pg_class where oid = 'public.events'::regclass),
  'events has RLS enabled'
);

select ok(has_table_privilege('authenticated', 'public.profiles', 'select'), 'authenticated can read profiles');
select ok(not has_table_privilege('anon', 'public.profiles', 'select'), 'anon cannot read profiles');
select ok(has_column_privilege('authenticated', 'public.profiles', 'handle', 'update'), 'owners can update profile handles');
select ok(has_column_privilege('authenticated', 'public.profiles', 'display_name', 'update'), 'owners can update display names');
select ok(has_column_privilege('authenticated', 'public.profiles', 'avatar_url', 'update'), 'owners can update avatar URLs');
select ok(not has_column_privilege('authenticated', 'public.profiles', 'id', 'update'), 'profile ownership cannot be reassigned');

select ok(has_table_privilege('authenticated', 'public.pets', 'select'), 'authenticated can read authorized pets');
select ok(has_column_privilege('authenticated', 'public.pets', 'snapshot', 'update'), 'owners can update private snapshots');
select ok(not has_column_privilege('authenticated', 'public.pets', 'connector_seen_at', 'update'), 'heartbeat timestamps cannot be written directly');
select ok(not has_table_privilege('anon', 'public.pets', 'select'), 'anon cannot read private snapshots');

select ok(has_table_privilege('authenticated', 'public.events', 'select'), 'authenticated can read authorized events');
select ok(has_column_privilege('authenticated', 'public.events', 'kind', 'insert'), 'authenticated can append event kinds');
select ok(has_column_privilege('authenticated', 'public.events', 'params', 'insert'), 'authenticated can append validated event params');
select ok(not has_column_privilege('authenticated', 'public.events', 'user_id', 'insert'), 'event ownership is derived from auth.uid');
select ok(not has_table_privilege('authenticated', 'public.events', 'update'), 'events cannot be updated');
select ok(not has_table_privilege('authenticated', 'public.events', 'delete'), 'events cannot be deleted');
select ok(not has_table_privilege('anon', 'public.events', 'select'), 'anon cannot read events');

select has_function('public', 'connector_heartbeat', array[]::text[], 'connector_heartbeat RPC exists');
select ok(has_function_privilege('authenticated', 'public.connector_heartbeat()', 'execute'), 'authenticated can call connector_heartbeat');
select ok(not has_function_privilege('anon', 'public.connector_heartbeat()', 'execute'), 'anon cannot call connector_heartbeat');
select ok(
  (select prosecdef from pg_proc where oid = 'public.connector_heartbeat()'::regprocedure),
  'connector_heartbeat is security definer'
);
select ok(
  exists (
    select 1
    from pg_proc
    where oid = 'public.connector_heartbeat()'::regprocedure
      and coalesce(proconfig, '{}'::text[]) @> array['search_path=""']::text[]
  ),
  'connector_heartbeat pins an empty search_path'
);
select has_trigger('public', 'profiles', 'profiles_rate_limit_update', 'profile updates have a transactional rate-limit trigger');
select has_trigger('public', 'pets', 'pets_validate_snapshot', 'snapshot writes use the validation and rate-limit trigger');
select has_trigger('public', 'events', 'events_validate_payload', 'event inserts use the validation and rate-limit trigger');

insert into auth.users (
  id,
  aud,
  role,
  email,
  encrypted_password,
  email_confirmed_at,
  raw_app_meta_data,
  raw_user_meta_data,
  created_at,
  updated_at
)
values
  (
    '00000000-0000-0000-0000-00000000a101',
    'authenticated',
    'authenticated',
    'alice-foundation@example.test',
    '',
    now(),
    '{"provider":"github","providers":["github"]}'::jsonb,
    '{"user_name":"Alice-GitHub","avatar_url":"https://avatars.githubusercontent.com/u/101"}'::jsonb,
    now(),
    now()
  ),
  (
    '00000000-0000-0000-0000-00000000b102',
    'authenticated',
    'authenticated',
    'bob-foundation@example.test',
    '',
    now(),
    '{"provider":"github","providers":["github"]}'::jsonb,
    '{"user_name":"Alice-GitHub","avatar_url":"http://insecure.example/avatar.png"}'::jsonb,
    now(),
    now()
  ),
  (
    '00000000-0000-0000-0000-00000000c103',
    'authenticated',
    'authenticated',
    'carol-foundation@example.test',
    '',
    now(),
    '{"provider":"email","providers":["email"]}'::jsonb,
    '{"user_name":"spoofed-github-handle","avatar_url":"https://avatars.githubusercontent.com/u/103"}'::jsonb,
    now(),
    now()
  );

select is(
  (select handle from public.profiles where id = '00000000-0000-0000-0000-00000000a101'),
  'alice-github',
  'GitHub metadata is normalized into a validated handle'
);
select ok(
  (select handle from public.profiles where id = '00000000-0000-0000-0000-00000000b102')
    like 'user-%',
  'duplicate GitHub handles receive a collision-proof system fallback'
);
select ok(
  (select handle from public.profiles where id = '00000000-0000-0000-0000-00000000c103')
    like 'user-%',
  'non-GitHub signup metadata cannot seed a spoofed GitHub handle'
);
select is(
  (select avatar_url from public.profiles where id = '00000000-0000-0000-0000-00000000c103'),
  null,
  'non-GitHub signup metadata cannot seed a spoofed GitHub avatar'
);
select is(
  (select avatar_url from public.profiles where id = '00000000-0000-0000-0000-00000000a101'),
  'https://avatars.githubusercontent.com/u/101',
  'HTTPS provider avatar is adopted'
);
select is(
  (select avatar_url from public.profiles where id = '00000000-0000-0000-0000-00000000b102'),
  null,
  'non-HTTPS provider avatar is discarded'
);
select is(
  (select count(*) from public.pets where user_id in (
    '00000000-0000-0000-0000-00000000a101',
    '00000000-0000-0000-0000-00000000b102',
    '00000000-0000-0000-0000-00000000c103'
  )),
  3::bigint,
  'auth provisioning creates one private pet row per user'
);
select ok(
  private.valid_private_snapshot(
    (select snapshot from public.pets where user_id = '00000000-0000-0000-0000-00000000a101')
  ),
  'provisioned pet snapshot satisfies the server validator'
);

select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000a101', true);
select set_config('request.jwt.claim.role', 'authenticated', true);
set local role authenticated;

select is((select count(*) from public.profiles), 3::bigint, 'authenticated users can discover validated profiles');
select is((select count(*) from public.pets), 1::bigint, 'pet RLS exposes only the caller private snapshot');
select is((select count(*) from public.events), 0::bigint, 'event RLS starts with only caller events');

select lives_ok(
  $$insert into public.events(kind, params) values ('task_completed', '{"source":"codex"}'::jsonb)$$,
  'authenticated users can append a valid own event'
);
select throws_ok(
  $$insert into public.events(kind, params) values ('task_completed', '{"source":"codex","note":"secret"}'::jsonb)$$,
  '23514',
  null,
  'server constraint rejects extra event params'
);
select throws_ok(
  $$insert into public.events(kind, params) values ('unknown', '{}'::jsonb)$$,
  '23514',
  null,
  'server constraint rejects unknown event kinds'
);
select throws_ok(
  $$insert into public.events(user_id, kind, params) values (
    '00000000-0000-0000-0000-00000000b102',
    'task_completed',
    '{"source":"cc"}'::jsonb
  )$$,
  '42501',
  null,
  'callers cannot forge event ownership'
);
select throws_ok(
  $$update public.pets set snapshot = snapshot || '{"note":"secret"}'::jsonb$$,
  '23514',
  null,
  'server constraint rejects extra private snapshot fields'
);
select throws_ok(
  $$update public.profiles set avatar_url = 'http://insecure.example/avatar.png' where id = auth.uid()$$,
  '23514',
  null,
  'profile validation rejects non-HTTPS avatars'
);
select throws_ok(
  $$update public.profiles set avatar_url = 'https://%' where id = auth.uid()$$,
  '23514',
  null,
  'profile validation rejects malformed HTTPS hosts'
);
select throws_ok(
  $$update public.profiles set avatar_url = 'https:///local/path' where id = auth.uid()$$,
  '23514',
  null,
  'profile validation rejects HTTPS URLs without a host'
);
select throws_ok(
  $$update public.profiles
    set display_name = 'safe' || chr(8238) || 'spoofed'
    where id = auth.uid()$$,
  '23514',
  null,
  'profile validation rejects bidi override controls'
);
select throws_ok(
  $$insert into public.events(kind, params, occurred_at)
    values ('task_completed', '{"source":"codex"}'::jsonb, clock_timestamp() + interval '10 minutes')$$,
  '22023',
  'invalid_event_timestamp',
  'events reject timestamps too far in the future'
);
select throws_ok(
  $$insert into public.events(kind, params, occurred_at)
    values ('task_completed', '{"source":"codex"}'::jsonb, clock_timestamp() - interval '31 days')$$,
  '22023',
  'invalid_event_timestamp',
  'events reject stale timestamps outside the replay window'
);

select lives_ok('select public.connector_heartbeat()', 'first heartbeat succeeds');
select lives_ok('select public.connector_heartbeat()', 'second heartbeat succeeds');
select throws_ok(
  'select public.connector_heartbeat()',
  'P0001',
  'rate_limit_exceeded',
  'third heartbeat in one minute is rate limited'
);
select ok(
  (select connector_seen_at is not null from public.pets where user_id = auth.uid()),
  'heartbeat updates only the caller canonical pet row'
);

reset role;
select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000b102', true);
set local role authenticated;
select is((select count(*) from public.events), 0::bigint, 'another user cannot read Alice events');
select is((select count(*) from public.pets), 1::bigint, 'another user sees only their own pet row');

reset role;
select private.consume_rate_limit(
  '00000000-0000-0000-0000-00000000c103',
  'profile_update',
  'self',
  60,
  interval '1 hour'
) from generate_series(1, 60);
select private.consume_rate_limit(
  '00000000-0000-0000-0000-00000000b102',
  'snapshot_update',
  'self',
  120,
  interval '1 minute'
) from generate_series(1, 120);
select private.consume_rate_limit(
  '00000000-0000-0000-0000-00000000c103',
  'event_insert',
  'self',
  120,
  interval '1 minute'
) from generate_series(1, 120);

select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000c103', true);
set local role authenticated;
select throws_ok(
  $$update public.profiles set display_name = 'Carol' where id = auth.uid()$$,
  'P0001',
  'rate_limit_exceeded',
  'profile updates are limited to 60 per hour per actor'
);
select throws_ok(
  $$insert into public.events(kind, params) values ('task_completed', '{"source":"cursor"}'::jsonb)$$,
  'P0001',
  'rate_limit_exceeded',
  'event inserts are limited to 120 per minute per actor'
);

reset role;
select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000b102', true);
set local role authenticated;
select throws_ok(
  $$update public.pets set snapshot = snapshot where user_id = auth.uid()$$,
  'P0001',
  'rate_limit_exceeded',
  'private snapshot updates are limited to 120 per minute per actor'
);

reset role;
select set_config('request.jwt.claim.sub', '', true);
select set_config('request.jwt.claim.role', 'anon', true);
set local role anon;
select throws_ok(
  'select public.connector_heartbeat()',
  '42501',
  null,
  'anonymous callers cannot execute connector_heartbeat'
);

reset role;
select * from finish();
rollback;
