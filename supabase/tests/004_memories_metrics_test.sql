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
  ('00000000-0000-0000-0000-000000000011', 'authenticated', 'authenticated',
   'alice-memory@pawbae.test', now(), '{"provider":"github","providers":["github"]}',
   '{"user_name":"alice-memory","avatar_url":"https://avatars.githubusercontent.com/u/11"}', now(), now()),
  ('00000000-0000-0000-0000-000000000012', 'authenticated', 'authenticated',
   'bob-memory@pawbae.test', now(), '{"provider":"github","providers":["github"]}',
   '{"user_name":"bob-memory","avatar_url":"https://avatars.githubusercontent.com/u/12"}', now(), now())
ON CONFLICT (id) DO NOTHING;

INSERT INTO public.profiles (id, handle, display_name, avatar_url)
VALUES
  ('00000000-0000-0000-0000-000000000011', 'alice-memory', 'Alice Memory', 'https://avatars.githubusercontent.com/u/11'),
  ('00000000-0000-0000-0000-000000000012', 'bob-memory', 'Bob Memory', 'https://avatars.githubusercontent.com/u/12')
ON CONFLICT (id) DO UPDATE
SET handle = EXCLUDED.handle,
    display_name = EXCLUDED.display_name,
    avatar_url = EXCLUDED.avatar_url;

INSERT INTO public.friendships (
  user_a, user_b, requester_id, status, accepted_at, created_at
)
VALUES (
  '00000000-0000-0000-0000-000000000011',
  '00000000-0000-0000-0000-000000000012',
  '00000000-0000-0000-0000-000000000011',
  'accepted',
  clock_timestamp() - interval '1 hour',
  clock_timestamp() - interval '1 hour 5 minutes'
)
ON CONFLICT (user_a, user_b) DO UPDATE
SET status = 'accepted',
    accepted_at = clock_timestamp() - interval '1 hour',
    created_at = clock_timestamp() - interval '1 hour 5 minutes';

ALTER TABLE public.visits DISABLE TRIGGER visits_validate_row;
INSERT INTO public.visits (
  visitor_user_id, host_user_id, status, requested_at, request_expires_at,
  started_at, ends_at, ended_at
)
VALUES (
  '00000000-0000-0000-0000-000000000011',
  '00000000-0000-0000-0000-000000000012',
  'completed',
  clock_timestamp() - interval '35 minutes',
  clock_timestamp() + interval '23 hours 25 minutes',
  clock_timestamp() - interval '30 minutes',
  clock_timestamp(),
  clock_timestamp()
);
ALTER TABLE public.visits ENABLE TRIGGER visits_validate_row;

SELECT has_table('public', 'shared_memories', 'shared memories table exists');
SELECT has_function('public', 'settle_shared_memory', ARRAY['uuid', 'text'], 'memory settlement RPC exists');
SELECT has_function('public', 'record_memory_view', ARRAY['uuid', 'text'], 'memory view instrumentation RPC exists');

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000011');
CREATE TEMP TABLE memory_test_state AS
SELECT (public.settle_shared_memory(
  (SELECT id FROM public.visits WHERE visitor_user_id = '00000000-0000-0000-0000-000000000011'),
  'memory-settle-alice-bob-0001'
)).*;

SELECT is((SELECT template_key::text FROM memory_test_state), 'played_together', 'settlement selects a server-owned template key');
SELECT is((SELECT visitor_user_id FROM memory_test_state), '00000000-0000-0000-0000-000000000011'::uuid, 'visitor participant is copied from the visit');
SELECT is((SELECT host_user_id FROM memory_test_state), '00000000-0000-0000-0000-000000000012'::uuid, 'host participant is copied from the visit');
SELECT ok(
  (SELECT params ? 'durationBucket' AND params ? 'timeOfDay' AND params ? 'interactionCount' FROM memory_test_state),
  'memory params contain only derived enum and numeric facts'
);
SELECT ok(
  (SELECT (SELECT count(*) FROM jsonb_object_keys(params)) = 3 FROM memory_test_state),
  'memory params have a closed key set with no free-text slot'
);
SELECT is(
  (public.settle_shared_memory(
    (SELECT visit_id FROM memory_test_state),
    'memory-settle-alice-bob-0001'
  )).id,
  (SELECT id FROM memory_test_state),
  'settlement replay returns the original canonical memory'
);
SELECT is(
  (SELECT count(*)::integer FROM public.shared_memories WHERE visit_id = (SELECT visit_id FROM memory_test_state)),
  1,
  'one visit creates at most one canonical memory'
);

SELECT throws_ok(
  $$INSERT INTO public.shared_memories (
      visit_id, visitor_user_id, host_user_id, template_key, params
    )
    SELECT
      visit_id, host_user_id, visitor_user_id, template_key, params
    FROM memory_test_state$$,
  'P0001',
  'memory participants must match the visit',
  'privileged inserts cannot forge memory participants'
);
SELECT throws_ok(
  $$INSERT INTO public.shared_memories (
      visit_id, visitor_user_id, host_user_id, template_key, params
    )
    SELECT
      visit_id, visitor_user_id, host_user_id, template_key,
      params || jsonb_build_object('note', 'untrusted free text')
    FROM memory_test_state$$,
  'P0001',
  'invalid shared memory parameters',
  'memory rows reject extra free-text parameters'
);
SELECT throws_ok(
  $$INSERT INTO public.shared_memories (
      visit_id, visitor_user_id, host_user_id, template_key, params
    )
    SELECT
      visit_id, visitor_user_id, host_user_id, template_key, params
    FROM memory_test_state$$,
  '23505',
  'duplicate key value violates unique constraint "shared_memories_visit_id_key"',
  'a visit cannot be forged into a second canonical memory'
);

SELECT lives_ok(
  format(
    'SELECT public.record_memory_view(%L::uuid, %L)',
    (SELECT id FROM memory_test_state),
    'memory-view-alice-0001'
  ),
  'a participant can record a replay-safe memory view'
);
SELECT lives_ok(
  format(
    'SELECT public.record_memory_view(%L::uuid, %L)',
    (SELECT id FROM memory_test_state),
    'memory-view-alice-0001'
  ),
  'memory-view replay is idempotent'
);

ALTER TABLE public.visits DISABLE TRIGGER visits_validate_row;
INSERT INTO public.visits (
  visitor_user_id, host_user_id, status, requested_at, request_expires_at,
  started_at, ends_at, ended_at
)
VALUES (
  '00000000-0000-0000-0000-000000000011',
  '00000000-0000-0000-0000-000000000012',
  'completed',
  clock_timestamp() - interval '5 minutes',
  clock_timestamp() + interval '23 hours 55 minutes',
  clock_timestamp() - interval '4 minutes',
  clock_timestamp(),
  clock_timestamp()
);
ALTER TABLE public.visits ENABLE TRIGGER visits_validate_row;

SELECT is(
  (
    SELECT converted
    FROM public.funnel_friend_request_acceptance
    WHERE pair_user_a = '00000000-0000-0000-0000-000000000011'
      AND pair_user_b = '00000000-0000-0000-0000-000000000012'
    ORDER BY request_sent_at
    LIMIT 1
  ),
  true,
  'friend-request acceptance funnel reports the accepted fixture'
);
SELECT is(
  (
    SELECT converted
    FROM public.funnel_friend_to_first_visit
    WHERE pair_user_a = '00000000-0000-0000-0000-000000000011'
      AND pair_user_b = '00000000-0000-0000-0000-000000000012'
    ORDER BY friendship_accepted_at
    LIMIT 1
  ),
  true,
  'friend-to-first-visit funnel reports the first request'
);
SELECT is(
  (
    SELECT bool_and(converted)
    FROM public.funnel_visit_completion
    WHERE pair_user_a = '00000000-0000-0000-0000-000000000011'
      AND pair_user_b = '00000000-0000-0000-0000-000000000012'
  ),
  true,
  'visit-completion funnel reports both completed visits'
);
SELECT is(
  (
    SELECT converted
    FROM public.funnel_memory_view
    WHERE visit_id = (SELECT visit_id FROM memory_test_state)
  ),
  true,
  'memory-view funnel reports the replay-safe participant view'
);
SELECT is(
  (
    SELECT converted
    FROM public.funnel_seven_day_repeat_visit
    WHERE pair_user_a = '00000000-0000-0000-0000-000000000011'
      AND pair_user_b = '00000000-0000-0000-0000-000000000012'
  ),
  true,
  'seven-day repeat funnel reports the second completed visit'
);

DELETE FROM private.rate_limits
WHERE actor_id = '00000000-0000-0000-0000-000000000011'
  AND action = 'social_mutation';
SELECT private.consume_rate_limit(
  '00000000-0000-0000-0000-000000000011',
  'social_mutation',
  'all',
  60,
  interval '1 hour'
) FROM generate_series(1, 60);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000011');
SELECT throws_ok(
  format(
    'SELECT public.record_memory_view(%L::uuid, %L)',
    (SELECT id FROM memory_test_state),
    'memory-view-rate-limited-0001'
  ),
  'P0001',
  'rate_limit_exceeded',
  'memory mutations enforce the global social mutation limit'
);

SELECT ok(
  EXISTS (
    SELECT 1
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'public'
      AND c.relname = 'funnel_friend_request_acceptance'
      AND c.relkind = 'v'
      AND COALESCE((c.reloptions && ARRAY['security_invoker=on', 'security_invoker=true']), false)
  ),
  'friend request funnel is a security_invoker view'
);
SELECT ok(
  EXISTS (
    SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'public' AND c.relname = 'funnel_friend_to_first_visit'
      AND COALESCE((c.reloptions && ARRAY['security_invoker=on', 'security_invoker=true']), false)
  ),
  'friend-to-visit funnel is a security_invoker view'
);
SELECT ok(
  EXISTS (
    SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'public' AND c.relname = 'funnel_visit_completion'
      AND COALESCE((c.reloptions && ARRAY['security_invoker=on', 'security_invoker=true']), false)
  ),
  'visit completion funnel is a security_invoker view'
);
SELECT ok(
  EXISTS (
    SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'public' AND c.relname = 'funnel_memory_view'
      AND COALESCE((c.reloptions && ARRAY['security_invoker=on', 'security_invoker=true']), false)
  ),
  'memory-view funnel is a security_invoker view'
);
SELECT ok(
  EXISTS (
    SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'public' AND c.relname = 'funnel_seven_day_repeat_visit'
      AND COALESCE((c.reloptions && ARRAY['security_invoker=on', 'security_invoker=true']), false)
  ),
  'seven-day repeat funnel is a security_invoker view'
);

SELECT * FROM finish();
ROLLBACK;
