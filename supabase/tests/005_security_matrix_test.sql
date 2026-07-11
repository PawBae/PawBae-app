BEGIN;
SELECT no_plan();

CREATE OR REPLACE FUNCTION pg_temp.set_actor(p_actor uuid)
RETURNS void
LANGUAGE plpgsql
AS $$
BEGIN
  PERFORM set_config('request.jwt.claim.sub', p_actor::text, true);
  PERFORM set_config(
    'request.jwt.claims',
    jsonb_build_object('sub', p_actor, 'role', 'authenticated')::text,
    true
  );
END;
$$;

INSERT INTO auth.users (
  id, aud, role, email, email_confirmed_at, raw_app_meta_data, raw_user_meta_data,
  created_at, updated_at
)
VALUES
  ('00000000-0000-0000-0000-000000000021', 'authenticated', 'authenticated',
   'alice-security@pawbae.test', now(), '{"provider":"github","providers":["github"]}', '{"user_name":"alice-security"}', now(), now()),
  ('00000000-0000-0000-0000-000000000022', 'authenticated', 'authenticated',
   'bob-security@pawbae.test', now(), '{"provider":"github","providers":["github"]}', '{"user_name":"bob-security"}', now(), now()),
  ('00000000-0000-0000-0000-000000000023', 'authenticated', 'authenticated',
   'stranger-security@pawbae.test', now(), '{"provider":"github","providers":["github"]}', '{"user_name":"stranger-security"}', now(), now()),
  ('00000000-0000-0000-0000-000000000024', 'authenticated', 'authenticated',
   'blocked-security@pawbae.test', now(), '{"provider":"github","providers":["github"]}', '{"user_name":"blocked-security"}', now(), now())
ON CONFLICT (id) DO NOTHING;

INSERT INTO public.profiles (id, handle)
VALUES
  ('00000000-0000-0000-0000-000000000021', 'alice-security'),
  ('00000000-0000-0000-0000-000000000022', 'bob-security'),
  ('00000000-0000-0000-0000-000000000023', 'stranger-security'),
  ('00000000-0000-0000-0000-000000000024', 'blocked-security')
ON CONFLICT (id) DO UPDATE SET handle = EXCLUDED.handle;

INSERT INTO public.friendships (user_a, user_b, requester_id, status, accepted_at)
VALUES (
  '00000000-0000-0000-0000-000000000021',
  '00000000-0000-0000-0000-000000000022',
  '00000000-0000-0000-0000-000000000021',
  'accepted',
  clock_timestamp()
)
ON CONFLICT (user_a, user_b) DO UPDATE
SET status = 'accepted', accepted_at = clock_timestamp();

ALTER TABLE public.visits DISABLE TRIGGER visits_validate_row;
INSERT INTO public.visits (
  id, visitor_user_id, host_user_id, status, requested_at, request_expires_at,
  started_at, ends_at, ended_at
)
VALUES (
  '20000000-0000-0000-0000-000000000021',
  '00000000-0000-0000-0000-000000000021',
  '00000000-0000-0000-0000-000000000022',
  'completed',
  clock_timestamp() - interval '35 minutes',
  clock_timestamp() + interval '23 hours 25 minutes',
  clock_timestamp() - interval '30 minutes',
  clock_timestamp(),
  clock_timestamp()
);
INSERT INTO public.visits (
  id, visitor_user_id, host_user_id, status, requested_at, request_expires_at,
  started_at, ends_at
)
VALUES (
  '10000000-0000-0000-0000-000000000021',
  '00000000-0000-0000-0000-000000000021',
  '00000000-0000-0000-0000-000000000022',
  'visiting',
  clock_timestamp() - interval '5 minutes',
  clock_timestamp() + interval '23 hours 55 minutes',
  clock_timestamp() - interval '4 minutes',
  clock_timestamp() + interval '26 minutes'
);
ALTER TABLE public.visits ENABLE TRIGGER visits_validate_row;

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000021');
SELECT lives_ok(
  $$SELECT public.settle_shared_memory(
      '20000000-0000-0000-0000-000000000021',
      'security-memory-settle-0001'
    )$$,
  'a visit participant can settle the historical memory fixture'
);
SET LOCAL ROLE authenticated;
SELECT is(
  (SELECT count(*)::integer FROM public.shared_memories),
  1,
  'the visitor can read its private historical memory before a block'
);
RESET ROLE;

DELETE FROM realtime.messages
WHERE topic = 'pet:00000000-0000-0000-0000-000000000021:10000000-0000-0000-0000-000000000021';
INSERT INTO public.pet_projections (
  owner_user_id, pet_id, version, display_name, skin_id, status
)
VALUES (
  '00000000-0000-0000-0000-000000000021',
  'alice-security-pet', 1, 'Alice Security', 'yoonie', 'working'
);

SELECT is(
  (
    SELECT count(*)
    FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000021:10000000-0000-0000-0000-000000000021'
      AND event = 'projection_updated'
  ),
  1::bigint,
  'active visit fixture emits exactly one projection Broadcast'
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000023');
SET LOCAL ROLE authenticated;
SELECT is((SELECT count(*)::integer FROM public.visits), 0, 'a stranger cannot read another pair visit');
SELECT is((SELECT count(*)::integer FROM public.pet_projections), 0, 'a stranger cannot read an active visitor projection');
SELECT is((SELECT count(*)::integer FROM public.shared_memories), 0, 'a stranger cannot read another pair memory');
SELECT throws_ok(
  format(
    'SELECT public.accept_visit(%L::uuid, %L)',
    '10000000-0000-0000-0000-000000000021',
    'forged-host-accept-0001'
  ),
  'P0001',
  'only the visit host can accept this request',
  'a stranger cannot forge host identity in an RPC'
);
RESET ROLE;

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000024');
SET LOCAL ROLE authenticated;
SELECT is((SELECT count(*)::integer FROM public.shared_memories), 0, 'an unrelated blocked user cannot read the pair memory');
RESET ROLE;

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000022');
SET LOCAL ROLE authenticated;
SELECT is((SELECT count(*)::integer FROM public.visits), 2, 'the host can read its active and historical visits');
SELECT is((SELECT count(*)::integer FROM public.pet_projections), 1, 'the host can read the projection during a valid lease');
SELECT is((SELECT count(*)::integer FROM public.shared_memories), 1, 'the host can read its historical memory during a visit');
SELECT throws_ok(
  $$UPDATE public.visits SET ends_at = clock_timestamp() + interval '1 day'$$,
  '42501',
  'permission denied for table visits',
  'the host cannot forge a lease end time with a direct update'
);
SELECT ok(
  NOT has_table_privilege('authenticated', 'realtime.messages', 'INSERT'),
  'authenticated clients cannot publish Realtime Broadcast messages'
);
SELECT ok(
  NOT has_table_privilege('authenticated', 'realtime.messages', 'UPDATE'),
  'authenticated clients cannot alter Realtime Broadcast messages'
);
SELECT ok(
  NOT has_table_privilege('authenticated', 'realtime.messages', 'DELETE'),
  'authenticated clients cannot delete Realtime Broadcast messages'
);
SELECT ok(
  NOT has_table_privilege('anon', 'realtime.messages', 'SELECT'),
  'anonymous clients cannot read private Realtime Broadcast messages'
);
RESET ROLE;

INSERT INTO public.blocks (blocker_id, blocked_id)
VALUES (
  '00000000-0000-0000-0000-000000000022',
  '00000000-0000-0000-0000-000000000021'
)
ON CONFLICT DO NOTHING;

SELECT is(
  (
    SELECT status::text
    FROM public.visits
    WHERE id = '10000000-0000-0000-0000-000000000021'
  ),
  'blocked',
  'blocking terminates an active visit in the same transaction'
);
SELECT ok(
  NOT private.can_receive_visit_topic(
    '00000000-0000-0000-0000-000000000022',
    'pet:00000000-0000-0000-0000-000000000021:10000000-0000-0000-0000-000000000021'
  ),
  'topic authorization is revoked immediately after a block'
);
SELECT ok(
  NOT EXISTS (
    SELECT 1 FROM private.authorized_projection_visits(
      '00000000-0000-0000-0000-000000000021'
    )
  ),
  'projection publishing is revoked immediately after a block'
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000021');
SELECT lives_ok(
  $$SELECT public.update_projection('alice-security-pet', 'yoonie', 'waiting')$$,
  'the owner may still update its stored projection after a block'
);
SELECT is(
  (
    SELECT count(*)
    FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000021:10000000-0000-0000-0000-000000000021'
      AND event = 'projection_updated'
  ),
  1::bigint,
  'post-block projection updates emit no additional Broadcast'
);
SET LOCAL ROLE authenticated;
SELECT is(
  (SELECT count(*)::integer FROM public.shared_memories),
  1,
  'the visitor retains its private historical memory after a block'
);
RESET ROLE;

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000022');
SET LOCAL ROLE authenticated;
SELECT is((SELECT count(*)::integer FROM public.pet_projections), 0, 'a blocked former host cannot read the visitor projection');
SELECT is(
  (SELECT count(*)::integer FROM public.shared_memories),
  1,
  'the former host retains its private historical memory after a block'
);
RESET ROLE;

SELECT ok(
  NOT has_function_privilege('anon', 'public.request_visit(uuid,text)', 'EXECUTE'),
  'anonymous callers cannot execute authenticated visit RPCs'
);
SELECT ok(
  NOT EXISTS (
    SELECT 1
    FROM pg_proc p
    CROSS JOIN LATERAL aclexplode(COALESCE(p.proacl, acldefault('f', p.proowner))) acl
    WHERE p.oid = 'public.request_visit(uuid,text)'::regprocedure
      AND acl.grantee = 0
      AND acl.privilege_type = 'EXECUTE'
  ),
  'visit RPC execution is revoked from PUBLIC'
);
SELECT ok(
  NOT EXISTS (
    SELECT 1
    FROM pg_proc p
    CROSS JOIN LATERAL aclexplode(COALESCE(p.proacl, acldefault('f', p.proowner))) acl
    WHERE p.oid = 'private.request_visit_impl(uuid,text)'::regprocedure
      AND acl.grantee = 0
      AND acl.privilege_type = 'EXECUTE'
  ),
  'privileged visit helpers are not executable by PUBLIC'
);
SELECT ok(
  NOT has_schema_privilege('authenticated', 'private', 'USAGE'),
  'authenticated clients have no USAGE on the private schema'
);
SELECT ok(
  NOT has_schema_privilege('anon', 'private', 'USAGE'),
  'anonymous clients have no USAGE on the private schema'
);
SELECT ok(
  NOT has_function_privilege('authenticated', 'private.request_visit_impl(uuid,text)', 'EXECUTE'),
  'authenticated clients cannot execute privileged visit helpers directly'
);
SELECT ok(
  has_function_privilege('authenticated', 'public.authorize_visit_topic()', 'EXECUTE'),
  'authenticated Realtime policy evaluation can call its narrow public authorization wrapper'
);
SELECT ok(
  NOT has_function_privilege('anon', 'public.authorize_visit_topic()', 'EXECUTE'),
  'anonymous callers cannot call the Realtime authorization wrapper'
);
SELECT ok(
  NOT has_function_privilege('authenticated', 'private.authorized_projection_visits(uuid)', 'EXECUTE'),
  'authenticated clients cannot execute the projection publishing guard directly'
);
SELECT ok(
  NOT has_table_privilege('authenticated', 'public.shared_memories', 'INSERT'),
  'shared memory participants cannot forge memory rows directly'
);
SELECT ok(
  NOT has_table_privilege('authenticated', 'public.invite_codes', 'INSERT'),
  'clients cannot mint their own invite codes'
);

SELECT * FROM finish();
ROLLBACK;
