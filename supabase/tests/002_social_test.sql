begin;
select * from no_plan();

select has_table('public', 'friendships', 'friendships table exists');
select has_table('public', 'blocks', 'blocks table exists');
select has_table('public', 'friend_mutes', 'friend_mutes table exists');
select has_table('public', 'waitlist', 'waitlist table exists');
select col_is_pk('public', 'friendships', array['user_a', 'user_b'], 'friendships use one canonical pair row');
select col_is_pk('public', 'blocks', array['blocker_id', 'blocked_id'], 'blocks are directional');
select col_is_pk('public', 'friend_mutes', array['owner_id', 'muted_user_id'], 'mutes are directional');
select col_is_pk('public', 'waitlist', 'id', 'waitlist has a stable canonical id');

select ok((select relrowsecurity from pg_class where oid = 'public.friendships'::regclass), 'friendships has RLS enabled');
select ok((select relrowsecurity from pg_class where oid = 'public.blocks'::regclass), 'blocks has RLS enabled');
select ok((select relrowsecurity from pg_class where oid = 'public.friend_mutes'::regclass), 'friend_mutes has RLS enabled');
select ok((select relrowsecurity from pg_class where oid = 'public.waitlist'::regclass), 'waitlist has RLS enabled');
select ok(
  exists (
    select 1 from pg_policies
    where schemaname = 'public'
      and tablename = 'waitlist'
      and policyname = 'waitlist_no_direct_access'
  ),
  'waitlist has an explicit deny-all direct-access policy'
);

select ok(has_table_privilege('authenticated', 'public.friendships', 'select'), 'authenticated users can read authorized friendships');
select ok(not has_table_privilege('authenticated', 'public.friendships', 'insert'), 'friendships cannot bypass RPC transitions');
select ok(not has_table_privilege('authenticated', 'public.blocks', 'insert'), 'blocks cannot bypass RPC transitions');
select ok(not has_table_privilege('authenticated', 'public.friend_mutes', 'insert'), 'mutes cannot bypass RPC transitions');
select ok(not has_table_privilege('anon', 'public.waitlist', 'insert'), 'anonymous waitlist writes must use the RPC');
select ok(not has_table_privilege('anon', 'public.waitlist', 'select'), 'anonymous callers cannot enumerate the waitlist');
select ok(not has_schema_privilege('authenticated', 'private', 'usage'), 'social callers cannot access private rate-limit rows');

select has_function('public', 'send_friend_request', array['uuid'], 'send_friend_request RPC exists');
select has_function('public', 'accept_friend_request', array['uuid'], 'accept_friend_request RPC exists');
select has_function('public', 'unfriend', array['uuid'], 'unfriend RPC exists');
select has_function('public', 'block_user', array['uuid'], 'block_user RPC exists');
select has_function('public', 'mute_user', array['uuid', 'boolean'], 'mute_user RPC exists');
select has_function('public', 'join_waitlist', array['text'], 'join_waitlist RPC exists');
select ok(
  pg_get_functiondef('private.lock_social_pair(uuid,uuid)'::regprocedure)
    ~* 'order by[[:space:]]+profile\.id[[:space:]]+for update',
  'social mutations lock both profile rows in canonical order'
);
select ok(
  position('''friend_request''' in (
    select prosrc from pg_proc where oid = 'public.send_friend_request(uuid)'::regprocedure
  )) > 0
  and position('''all''' in (
    select prosrc from pg_proc where oid = 'public.send_friend_request(uuid)'::regprocedure
  )) > 0,
  'friend request limit is keyed globally per actor rather than per target'
);
select is(
  (
    select count(*)
    from pg_proc
    where oid in (
      'public.accept_friend_request(uuid)'::regprocedure,
      'public.unfriend(uuid)'::regprocedure,
      'public.block_user(uuid)'::regprocedure,
      'public.mute_user(uuid,boolean)'::regprocedure
    )
      and position('''social_mutation''' in prosrc) > 0
      and position('''all''' in prosrc) > 0
  ),
  4::bigint,
  'all other social mutation limits are keyed globally per actor'
);

select ok(has_function_privilege('authenticated', 'public.send_friend_request(uuid)', 'execute'), 'authenticated can send friend requests');
select ok(not has_function_privilege('anon', 'public.send_friend_request(uuid)', 'execute'), 'anon cannot send friend requests');
select ok(has_function_privilege('anon', 'public.join_waitlist(text)', 'execute'), 'anon can join the waitlist');
select ok(has_function_privilege('authenticated', 'public.join_waitlist(text)', 'execute'), 'authenticated can join the waitlist');

select is(
  (select count(*) from pg_proc where oid in (
    'public.send_friend_request(uuid)'::regprocedure,
    'public.accept_friend_request(uuid)'::regprocedure,
    'public.unfriend(uuid)'::regprocedure,
    'public.block_user(uuid)'::regprocedure,
    'public.mute_user(uuid,boolean)'::regprocedure,
    'public.join_waitlist(text)'::regprocedure
  ) and prosecdef),
  6::bigint,
  'all public mutation wrappers are security definer'
);
select is(
  (select count(*) from pg_proc where oid in (
    'public.send_friend_request(uuid)'::regprocedure,
    'public.accept_friend_request(uuid)'::regprocedure,
    'public.unfriend(uuid)'::regprocedure,
    'public.block_user(uuid)'::regprocedure,
    'public.mute_user(uuid,boolean)'::regprocedure,
    'public.join_waitlist(text)'::regprocedure
  ) and coalesce(proconfig, '{}'::text[]) @> array['search_path=""']::text[]),
  6::bigint,
  'all public mutation wrappers pin an empty search_path'
);

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
  ('00000000-0000-0000-0000-00000000a201', 'authenticated', 'authenticated', 'alice-social@example.test', '', now(), '{"provider":"github"}', '{"user_name":"alice-social"}', now(), now()),
  ('00000000-0000-0000-0000-00000000b202', 'authenticated', 'authenticated', 'bob-social@example.test', '', now(), '{"provider":"github"}', '{"user_name":"bob-social"}', now(), now()),
  ('00000000-0000-0000-0000-00000000c203', 'authenticated', 'authenticated', 'carol-social@example.test', '', now(), '{"provider":"github"}', '{"user_name":"carol-social"}', now(), now()),
  ('00000000-0000-0000-0000-00000000d204', 'authenticated', 'authenticated', 'dave-social@example.test', '', now(), '{"provider":"github"}', '{"user_name":"dave-social"}', now(), now());

select lives_ok(
  $$insert into public.friendships(user_a, user_b, requester_id, status)
    values (
      '00000000-0000-0000-0000-00000000c203',
      '00000000-0000-0000-0000-00000000d204',
      '00000000-0000-0000-0000-00000000c203',
      'accepted'
    )$$,
  'accepted friendship rows normalize their accepted timestamp'
);
select ok(
  (select accepted_at is not null from public.friendships
   where user_a = '00000000-0000-0000-0000-00000000c203'
     and user_b = '00000000-0000-0000-0000-00000000d204'),
  'accepted friendship normalization records accepted_at'
);
delete from public.friendships
where user_a = '00000000-0000-0000-0000-00000000c203'
  and user_b = '00000000-0000-0000-0000-00000000d204';

select throws_ok(
  $$insert into public.friendships(user_a, user_b, requester_id, status)
    values (
      '00000000-0000-0000-0000-00000000b202',
      '00000000-0000-0000-0000-00000000a201',
      '00000000-0000-0000-0000-00000000a201',
      'pending'
    )$$,
  '23514',
  null,
  'database constraint rejects non-canonical friendship ordering'
);

select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000a201', true);
select set_config('request.jwt.claim.role', 'authenticated', true);
set local role authenticated;

select throws_ok(
  $$select public.send_friend_request('00000000-0000-0000-0000-00000000a201')$$,
  '22023',
  'cannot_target_self',
  'friend requests reject self targeting'
);
select lives_ok(
  $$select public.send_friend_request('00000000-0000-0000-0000-00000000b202')$$,
  'Alice can send Bob a friend request'
);
select is(
  (select status from public.friendships where user_a = '00000000-0000-0000-0000-00000000a201' and user_b = '00000000-0000-0000-0000-00000000b202'),
  'pending',
  'friend request creates a pending canonical row'
);
select is(
  (select requester_id from public.friendships where user_a = '00000000-0000-0000-0000-00000000a201' and user_b = '00000000-0000-0000-0000-00000000b202'),
  '00000000-0000-0000-0000-00000000a201'::uuid,
  'requester identity comes from auth.uid'
);
select lives_ok(
  $$select public.send_friend_request('00000000-0000-0000-0000-00000000b202')$$,
  'replayed outgoing friend request is idempotent'
);
select is(
  (select count(*) from public.friendships where user_a = '00000000-0000-0000-0000-00000000a201' and user_b = '00000000-0000-0000-0000-00000000b202'),
  1::bigint,
  'replayed friend request does not duplicate rows'
);

reset role;
select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000b202', true);
set local role authenticated;
select lives_ok(
  $$select public.accept_friend_request('00000000-0000-0000-0000-00000000a201')$$,
  'Bob can accept Alice incoming request'
);
select is(
  (select status from public.friendships where user_a = '00000000-0000-0000-0000-00000000a201' and user_b = '00000000-0000-0000-0000-00000000b202'),
  'accepted',
  'accept transitions the canonical friendship row'
);
select lives_ok(
  $$select public.accept_friend_request('00000000-0000-0000-0000-00000000a201')$$,
  'replayed accept is idempotent'
);

reset role;
select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000a201', true);
set local role authenticated;
select lives_ok(
  $$select public.mute_user('00000000-0000-0000-0000-00000000b202', true)$$,
  'Alice can directionally mute an accepted friend'
);
select is(
  (select muted from public.friend_mutes where owner_id = auth.uid()),
  true,
  'mute state is visible to its owner'
);

reset role;
select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000b202', true);
set local role authenticated;
select is((select count(*) from public.friend_mutes), 0::bigint, 'mute state is private from the muted user');

reset role;
select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000a201', true);
set local role authenticated;
select lives_ok(
  $$select public.unfriend('00000000-0000-0000-0000-00000000b202')$$,
  'Alice can unfriend Bob'
);
select is((select count(*) from public.friendships), 0::bigint, 'unfriend deletes the canonical friendship row');
select is((select count(*) from public.friend_mutes), 0::bigint, 'unfriend removes directional mute state');

select lives_ok(
  $$select public.send_friend_request('00000000-0000-0000-0000-00000000b202')$$,
  'Alice can request Bob again after unfriend'
);
select lives_ok(
  $$select public.block_user('00000000-0000-0000-0000-00000000b202')$$,
  'Alice can block Bob'
);
select is((select count(*) from public.friendships), 0::bigint, 'blocking removes friendship in the same transaction');
select is((select count(*) from public.blocks where blocker_id = auth.uid()), 1::bigint, 'directional block is visible to its owner');
select lives_ok(
  $$select public.block_user('00000000-0000-0000-0000-00000000b202')$$,
  'replayed block is idempotent'
);

reset role;
select ok(
  private.is_blocked(
    '00000000-0000-0000-0000-00000000a201',
    '00000000-0000-0000-0000-00000000b202'
  ),
  'private block lookup is bidirectional'
);

select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000b202', true);
set local role authenticated;
select is((select count(*) from public.blocks), 0::bigint, 'blocked users cannot enumerate who blocked them');
select is(
  (select count(*) from public.profiles where id = '00000000-0000-0000-0000-00000000a201'),
  0::bigint,
  'blocked users cannot discover the blocker profile'
);
select throws_ok(
  $$select public.send_friend_request('00000000-0000-0000-0000-00000000a201')$$,
  'P0001',
  'blocked_relationship',
  'blocked users cannot recreate friendship'
);
select throws_ok(
  $$select public.mute_user('00000000-0000-0000-0000-00000000a201', true)$$,
  'P0001',
  'blocked_relationship',
  'blocked users cannot create mute state'
);

reset role;
select set_config('request.jwt.claim.sub', '00000000-0000-0000-0000-00000000d204', true);
select private.consume_rate_limit(
  '00000000-0000-0000-0000-00000000d204',
  'friend_request',
  'all',
  20,
  interval '1 day'
) from generate_series(1, 20);
set local role authenticated;
select throws_ok(
  $$select public.send_friend_request('00000000-0000-0000-0000-00000000c203')$$,
  'P0001',
  'rate_limit_exceeded',
  'friend request RPC enforces the 20 per day actor limit'
);

reset role;
select set_config('request.jwt.claim.sub', '', true);
select set_config('request.jwt.claim.role', 'anon', true);
set local role anon;
select lives_ok(
  $$select public.join_waitlist('  Person+Test@Example.COM  ')$$,
  'anonymous caller can join the waitlist'
);

reset role;
select is(
  (select email from public.waitlist where email = 'person+test@example.com'),
  'person+test@example.com',
  'waitlist stores a normalized email'
);
select is(
  (select count(*) from public.waitlist where email = 'person+test@example.com'),
  1::bigint,
  'waitlist has one canonical row per normalized email'
);

set local role anon;
select lives_ok(
  $$select public.join_waitlist('person+test@example.com')$$,
  'duplicate waitlist join is idempotent'
);
select lives_ok(
  $$select public.join_waitlist('PERSON+TEST@example.com')$$,
  'third normalized waitlist join remains idempotent'
);
select lives_ok(
  $$select public.join_waitlist(' person+test@example.com ')$$,
  'fourth normalized waitlist join remains idempotent'
);
select lives_ok(
  $$select public.join_waitlist('person+test@example.com')$$,
  'fifth normalized waitlist join remains idempotent'
);
select throws_ok(
  $$select public.join_waitlist('person+test@example.com')$$,
  'P0001',
  'rate_limit_exceeded',
  'sixth normalized waitlist join in an hour is rate limited'
);
select throws_ok(
  $$select public.join_waitlist('not-an-email')$$,
  '22023',
  'invalid_email',
  'waitlist rejects invalid email input'
);
select throws_ok(
  $$insert into public.waitlist(email) values ('bypass@example.com')$$,
  '42501',
  null,
  'anon cannot bypass waitlist validation and rate limiting'
);

reset role;
select private.consume_rate_limit(
  '00000000-0000-0000-0000-000000000000',
  'waitlist',
  encode(extensions.digest('limited@example.com', 'sha256'), 'hex'),
  5,
  interval '1 hour'
) from generate_series(1, 5);
set local role anon;
select throws_ok(
  $$select public.join_waitlist('limited@example.com')$$,
  'P0001',
  'rate_limit_exceeded',
  'waitlist RPC enforces its normalized-email fixed window limit'
);

reset role;
select is(
  (select count(*) from public.waitlist where email = 'limited@example.com'),
  0::bigint,
  'rate-limited waitlist insert is rolled back atomically'
);

select * from finish();
rollback;
