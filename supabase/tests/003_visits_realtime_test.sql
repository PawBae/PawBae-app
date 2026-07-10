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
  ('00000000-0000-0000-0000-000000000001', 'authenticated', 'authenticated',
   'alice-visits@pawbae.test', now(), '{"provider":"github","providers":["github"]}',
   '{"user_name":"alice-visits","avatar_url":"https://avatars.githubusercontent.com/u/1"}', now(), now()),
  ('00000000-0000-0000-0000-000000000002', 'authenticated', 'authenticated',
   'bob-visits@pawbae.test', now(), '{"provider":"github","providers":["github"]}',
   '{"user_name":"bob-visits","avatar_url":"https://avatars.githubusercontent.com/u/2"}', now(), now()),
  ('00000000-0000-0000-0000-000000000003', 'authenticated', 'authenticated',
   'carol-visits@pawbae.test', now(), '{"provider":"github","providers":["github"]}',
   '{"user_name":"carol-visits","avatar_url":"https://avatars.githubusercontent.com/u/3"}', now(), now())
ON CONFLICT (id) DO NOTHING;

INSERT INTO public.profiles (id, handle, display_name, avatar_url)
VALUES
  ('00000000-0000-0000-0000-000000000001', 'alice-visits', 'Alice', 'https://avatars.githubusercontent.com/u/1'),
  ('00000000-0000-0000-0000-000000000002', 'bob-visits', 'Bob', 'https://avatars.githubusercontent.com/u/2'),
  ('00000000-0000-0000-0000-000000000003', 'carol-visits', 'Carol', 'https://avatars.githubusercontent.com/u/3')
ON CONFLICT (id) DO UPDATE
SET handle = EXCLUDED.handle,
    display_name = EXCLUDED.display_name,
    avatar_url = EXCLUDED.avatar_url;

INSERT INTO public.friendships (user_a, user_b, requester_id, status, accepted_at)
VALUES
  ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002',
   '00000000-0000-0000-0000-000000000001', 'accepted', clock_timestamp()),
  ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000003',
   '00000000-0000-0000-0000-000000000003', 'accepted', clock_timestamp())
ON CONFLICT (user_a, user_b) DO UPDATE
SET status = 'accepted', accepted_at = clock_timestamp();

SELECT has_table('public', 'visits', 'visits table exists');
SELECT has_table('public', 'pet_projections', 'pet projections table exists');
SELECT has_table('public', 'invite_codes', 'invite codes table exists');
SELECT has_table('public', 'invite_redemptions', 'invite redemptions table exists');
SELECT has_function('public', 'request_visit', ARRAY['uuid', 'text'], 'request_visit RPC exists');
SELECT has_function('public', 'accept_visit', ARRAY['uuid', 'text'], 'accept_visit RPC exists');
SELECT has_function('public', 'decline_visit', ARRAY['uuid', 'text'], 'decline_visit RPC exists');
SELECT has_function('public', 'cancel_visit', ARRAY['uuid', 'text'], 'cancel_visit RPC exists');
SELECT has_function('public', 'recall_visit', ARRAY['uuid', 'text'], 'recall_visit RPC exists');
SELECT has_function('public', 'end_visit', ARRAY['uuid', 'text'], 'end_visit RPC exists');
SELECT has_function('public', 'update_projection', ARRAY['text', 'text', 'projection_status'], 'update_projection RPC exists');
SELECT has_function('public', 'redeem_invite', ARRAY['text', 'text'], 'redeem_invite RPC exists');

SELECT throws_ok(
  $$INSERT INTO public.visits (
      visitor_user_id, host_user_id, status, requested_at, request_expires_at,
      started_at, ends_at
    ) VALUES (
      '00000000-0000-0000-0000-000000000001',
      '00000000-0000-0000-0000-000000000003',
      'accepted', clock_timestamp(), clock_timestamp() + interval '24 hours',
      clock_timestamp(), clock_timestamp() + interval '30 minutes'
    )$$,
  'P0001',
  'new visits must begin as requested',
  'privileged inserts cannot forge an already accepted lease'
);
SELECT throws_ok(
  $$INSERT INTO public.visits (
      visitor_user_id, host_user_id, status, requested_at, request_expires_at
    ) VALUES (
      '00000000-0000-0000-0000-000000000001',
      '00000000-0000-0000-0000-000000000003',
      'requested',
      clock_timestamp() - interval '1 hour',
      clock_timestamp() + interval '23 hours'
    )$$,
  'P0001',
  'new visit request must use the database clock',
  'privileged inserts cannot forge request timestamps'
);

CREATE TEMP TABLE visit_test_state (
  name text PRIMARY KEY,
  visit_id uuid NOT NULL
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
INSERT INTO visit_test_state (name, visit_id)
SELECT 'alice_bob', (public.request_visit(
  '00000000-0000-0000-0000-000000000002',
  'visit-request-alice-bob-0001'
)).id;

SELECT is(
  (SELECT status::text FROM public.visits WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')),
  'requested',
  'request_visit creates a requested visit'
);
SELECT is(
  (public.request_visit(
    '00000000-0000-0000-0000-000000000002',
    'visit-request-alice-bob-0001'
  )).id,
  (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob'),
  'request_visit replays the original canonical row for the same actor and key'
);
SELECT is(
  (SELECT visitor_user_id FROM public.visits WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')),
  '00000000-0000-0000-0000-000000000001'::uuid,
  'request_visit derives the visitor from auth.uid()'
);
SELECT is(
  (SELECT request_expires_at - requested_at FROM public.visits WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')),
  interval '24 hours',
  'visit requests expire exactly 24 hours after creation'
);
SELECT is(
  (public.request_visit(
    '00000000-0000-0000-0000-000000000002',
    'visit-request-alice-bob-alias'
  )).id,
  (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob'),
  'duplicate request with a different key returns the existing canonical request'
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SELECT is(
  (public.accept_visit(
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob'),
    'visit-accept-alice-bob-0001'
  )).status::text,
  'accepted',
  'the host can accept a pending visit'
);
SELECT is(
  (SELECT ends_at - started_at FROM public.visits WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')),
  interval '30 minutes',
  'accepted visits receive an exact 30 minute lease'
);
SELECT is(
  (public.accept_visit(
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob'),
    'visit-accept-alice-bob-0001'
  )).status::text,
  'accepted',
  'accept_visit replay returns its original accepted result'
);
SELECT throws_ok(
  format(
    'SELECT public.decline_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob'),
    'visit-accept-alice-bob-0001'
  ),
  'P0001',
  'idempotency key already used for another operation',
  'one actor cannot reuse an idempotency key across visit operations'
);
SELECT throws_ok(
  format(
    'SELECT public.decline_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob'),
    'visit-decline-invalid-state-0001'
  ),
  'P0001',
  'only a requested visit can be declined',
  'invalid visit transitions are rejected'
);
SELECT throws_ok(
  format(
    'UPDATE public.visits SET visitor_user_id = %L::uuid WHERE id = %L::uuid',
    '00000000-0000-0000-0000-000000000003',
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')
  ),
  'P0001',
  'visit participants are immutable',
  'privileged updates cannot forge visit ownership'
);
SELECT throws_ok(
  format(
    'UPDATE public.visits SET started_at = started_at + interval ''1 second'', ends_at = ends_at + interval ''1 second'' WHERE id = %L::uuid',
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')
  ),
  'P0001',
  'visit lease timestamps are immutable after acceptance',
  'privileged updates cannot shift an accepted lease'
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000003');
INSERT INTO visit_test_state (name, visit_id)
SELECT 'carol_bob', (public.request_visit(
  '00000000-0000-0000-0000-000000000002',
  'visit-request-carol-bob-0001'
)).id;

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SELECT throws_ok(
  format(
    'SELECT public.accept_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob'),
    'visit-accept-carol-bob-0001'
  ),
  'P0001',
  'host already has an active visit',
  'host uniqueness is enforced when accepting one of several pending requests'
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000003');
SELECT is(
  (public.cancel_visit(
    (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob'),
    'visit-cancel-carol-bob-0001'
  )).status::text,
  'cancelled',
  'the visitor can cancel its requested visit'
);

INSERT INTO visit_test_state (name, visit_id)
SELECT 'carol_bob_stale', (public.request_visit(
  '00000000-0000-0000-0000-000000000002',
  'visit-request-carol-bob-stale'
)).id;
ALTER TABLE public.visits DISABLE TRIGGER visits_validate_row;
UPDATE public.visits
SET requested_at = clock_timestamp() - interval '25 hours',
    request_expires_at = clock_timestamp() - interval '1 hour'
WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_stale');
ALTER TABLE public.visits ENABLE TRIGGER visits_validate_row;
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SELECT throws_ok(
  format(
    'SELECT public.accept_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_stale'),
    'visit-accept-carol-bob-stale'
  ),
  'P0001',
  'visit request has expired',
  'stale visit requests cannot be accepted even before cron updates status'
);
SELECT throws_ok(
  format(
    'SELECT public.decline_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_stale'),
    'visit-decline-carol-bob-stale'
  ),
  'P0001',
  'visit request has expired',
  'stale visit requests cannot be declined during cron lag'
);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000003');
SELECT throws_ok(
  format(
    'SELECT public.cancel_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_stale'),
    'visit-cancel-carol-bob-stale'
  ),
  'P0001',
  'visit request has expired',
  'stale visit requests cannot be cancelled during cron lag'
);
SELECT private.maintain_visits();

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
DELETE FROM realtime.messages
WHERE topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
  (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text;
SELECT is(
  (public.update_projection('alice-pet', 'yoonie', 'working')).owner_user_id,
  '00000000-0000-0000-0000-000000000001'::uuid,
  'update_projection derives the projection owner from auth.uid()'
);
SELECT is(
  (SELECT display_name FROM public.pet_projections WHERE owner_user_id = '00000000-0000-0000-0000-000000000001'),
  'Alice',
  'projection display name is sourced from the validated server profile'
);
SELECT is(
  (
    SELECT count(*) FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text
      AND event = 'projection_updated'
      AND extension = 'broadcast'
      AND private
  ),
  1::bigint,
  'projection trigger inserts one private Broadcast message on the exact lease topic'
);
SELECT is(
  (
    SELECT m.payload - 'id'
    FROM realtime.messages AS m
    WHERE m.topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text
      AND m.event = 'projection_updated'
    ORDER BY m.inserted_at DESC
    LIMIT 1
  ),
  (
    SELECT jsonb_build_object(
      'v', p.version,
      'petId', p.pet_id,
      'displayName', p.display_name,
      'skinId', p.skin_id,
      'status', p.status,
      'updatedAt', p.updated_at
    )
    FROM public.pet_projections AS p
    WHERE p.owner_user_id = '00000000-0000-0000-0000-000000000001'
  ),
  'projection Broadcast payload is exactly the approved safe projection fields'
);
SELECT ok(
  private.can_receive_visit_topic(
    '00000000-0000-0000-0000-000000000002',
    'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text
  ),
  'the active host is authorized for the private projection topic'
);
SELECT ok(
  NOT private.can_receive_visit_topic(
    '00000000-0000-0000-0000-000000000003',
    'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text
  ),
  'a stranger is not authorized for the private projection topic'
);

SELECT realtime.send(
  jsonb_build_object('kind', 'cross-topic-decoy'),
  'projection_updated',
  'pet:00000000-0000-0000-0000-000000000003:decoy-lease',
  true
);

SELECT set_config(
  'realtime.topic',
  'pet:00000000-0000-0000-0000-000000000001:' ||
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text,
  true
);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SET LOCAL ROLE authenticated;
SELECT is(
  (SELECT count(*) FROM realtime.messages WHERE event = 'projection_updated'),
  1::bigint,
  'Realtime RLS lets the active host read only the emitted projection message on its selected topic'
);
SELECT is(
  (
    SELECT count(*)
    FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000003:decoy-lease'
  ),
  0::bigint,
  'Realtime RLS does not leak a private Broadcast row from another topic'
);
RESET ROLE;
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000003');
SET LOCAL ROLE authenticated;
SELECT is(
  (SELECT count(*) FROM realtime.messages WHERE event = 'projection_updated'),
  0::bigint,
  'Realtime RLS hides the emitted projection message from a stranger'
);
RESET ROLE;

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
SELECT is(
  (public.recall_visit(
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob'),
    'visit-recall-alice-bob-0001'
  )).status::text,
  'returning',
  'the visitor can recall an active visit into returning state'
);
SELECT ok(
  NOT private.can_receive_visit_topic(
    '00000000-0000-0000-0000-000000000002',
    'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text
  ),
  'topic authorization ends immediately on recall'
);
SELECT ok(
  NOT EXISTS (
    SELECT 1 FROM private.authorized_projection_visits(
      '00000000-0000-0000-0000-000000000001'
    )
  ),
  'projection publishing ends immediately on recall'
);
SELECT is(
  (
    SELECT count(*) FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text
      AND event = 'visit_ended'
      AND payload->>'status' = 'returning'
  ),
  1::bigint,
  'recall emits one final visit_ended Broadcast before authorization closes'
);
SELECT is(
  (
    SELECT m.payload - 'id'
    FROM realtime.messages AS m
    WHERE m.topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text
      AND m.event = 'visit_ended'
      AND m.extension = 'broadcast'
      AND m.private
    ORDER BY m.inserted_at DESC
    LIMIT 1
  ),
  (
    SELECT jsonb_build_object(
      'leaseId', v.id,
      'status', v.status,
      'endedAt', v.returning_started_at
    )
    FROM public.visits AS v
    WHERE v.id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')
  ),
  'visit_ended is a private Broadcast with only lease, status, and server end time fields'
);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
SELECT lives_ok(
  $$SELECT public.update_projection('alice-pet', 'yoonie', 'waiting')$$,
  'owner may update its stored projection after recall'
);
SELECT is(
  (
    SELECT count(*) FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')::text
      AND event = 'projection_updated'
  ),
  1::bigint,
  'post-recall projection update emits no additional Broadcast'
);

ALTER TABLE public.visits DISABLE TRIGGER visits_validate_row;
UPDATE public.visits
SET returning_started_at = clock_timestamp() - interval '1 minute'
WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob');
ALTER TABLE public.visits ENABLE TRIGGER visits_validate_row;
SELECT private.maintain_visits();
SELECT is(
  (SELECT status::text FROM public.visits WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob')),
  'recalled',
  'maintenance settles a returning recall to recalled'
);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SELECT is(
  (public.accept_visit(
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob'),
    'visit-accept-alice-bob-0001'
  )).status::text,
  'accepted',
  'idempotent replay returns the original accepted result after the visit became recalled'
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
INSERT INTO visit_test_state (name, visit_id)
SELECT 'alice_bob_end', (public.request_visit(
  '00000000-0000-0000-0000-000000000002',
  'visit-request-alice-bob-0002'
)).id;
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SELECT lives_ok(
  format(
    'SELECT public.accept_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_end'),
    'visit-accept-alice-bob-0002'
  ),
  'host accepts the visit used to test end_visit revocation'
);
SELECT is(
  (public.end_visit(
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_end'),
    'visit-end-alice-bob-0002'
  )).status::text,
  'returning',
  'either participant can end an active visit into returning state'
);
SELECT ok(
  NOT private.can_receive_visit_topic(
    '00000000-0000-0000-0000-000000000002',
    'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_end')::text
  ),
  'topic authorization ends immediately on end_visit'
);
SELECT ok(
  NOT EXISTS (
    SELECT 1 FROM private.authorized_projection_visits(
      '00000000-0000-0000-0000-000000000001'
    )
  ),
  'projection publishing ends immediately on end_visit'
);
SELECT is(
  (
    SELECT count(*) FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_end')::text
      AND event = 'visit_ended'
  ),
  1::bigint,
  'end_visit emits one final visit_ended Broadcast'
);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
SELECT lives_ok(
  $$SELECT public.update_projection('alice-pet', 'yoonie', 'working')$$,
  'projection storage remains writable after end_visit'
);
SELECT is(
  (
    SELECT count(*) FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_end')::text
      AND event = 'projection_updated'
  ),
  0::bigint,
  'post-end projection update emits no Broadcast'
);
ALTER TABLE public.visits DISABLE TRIGGER visits_validate_row;
UPDATE public.visits
SET returning_started_at = clock_timestamp() - interval '1 minute'
WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_end');
ALTER TABLE public.visits ENABLE TRIGGER visits_validate_row;
SELECT private.maintain_visits();

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000003');
INSERT INTO visit_test_state (name, visit_id)
SELECT 'carol_bob_expiry', (public.request_visit(
  '00000000-0000-0000-0000-000000000002',
  'visit-request-carol-bob-0002'
)).id;
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SELECT lives_ok(
  format(
    'SELECT public.accept_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry'),
    'visit-accept-carol-bob-0002'
  ),
  'host accepts the visit used to test immediate lease expiry revocation'
);
ALTER TABLE public.visits DISABLE TRIGGER visits_validate_row;
UPDATE public.visits
SET started_at = clock_timestamp() - interval '31 minutes',
    ends_at = clock_timestamp() - interval '1 minute'
WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry');
ALTER TABLE public.visits ENABLE TRIGGER visits_validate_row;
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SELECT throws_ok(
  format(
    'SELECT public.end_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry'),
    'visit-end-carol-bob-expired-lease'
  ),
  'P0001',
  'visit lease has expired',
  'a host cannot end an already expired lease during cron lag'
);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000003');
SELECT throws_ok(
  format(
    'SELECT public.recall_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry'),
    'visit-recall-carol-bob-expired-lease'
  ),
  'P0001',
  'visit lease has expired',
  'a visitor cannot recall an already expired lease during cron lag'
);
SELECT ok(
  NOT private.can_receive_visit_topic(
    '00000000-0000-0000-0000-000000000002',
    'pet:00000000-0000-0000-0000-000000000003:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry')::text
  ),
  'topic authorization ends at lease expiry even before cron runs'
);
SELECT ok(
  NOT EXISTS (
    SELECT 1 FROM private.authorized_projection_visits(
      '00000000-0000-0000-0000-000000000003'
    )
  ),
  'projection publishing ends at lease expiry even before cron runs'
);
SELECT lives_ok(
  $$SELECT public.update_projection('carol-pet', 'yoonie', 'offline')$$,
  'projection storage remains writable after lease expiry'
);
SELECT is(
  (
    SELECT count(*) FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000003:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry')::text
      AND event = 'projection_updated'
  ),
  0::bigint,
  'post-expiry projection update emits no Broadcast'
);
SELECT private.maintain_visits();
SELECT is(
  (
    SELECT status::text
    FROM public.visits
    WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry')
  ),
  'completed',
  'maintenance completes an active visit at natural lease expiry'
);
SELECT is(
  (
    SELECT count(*)
    FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000003:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry')::text
      AND event = 'visit_ended'
      AND extension = 'broadcast'
      AND private
  ),
  1::bigint,
  'natural lease expiry emits exactly one private visit_ended Broadcast'
);
SELECT is(
  (
    SELECT message.payload - 'id'
    FROM realtime.messages AS message
    WHERE message.topic = 'pet:00000000-0000-0000-0000-000000000003:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry')::text
      AND message.event = 'visit_ended'
    ORDER BY message.inserted_at DESC
    LIMIT 1
  ),
  (
    SELECT jsonb_build_object(
      'leaseId', visit.id,
      'status', 'completed',
      'endedAt', visit.ends_at
    )
    FROM public.visits AS visit
    WHERE visit.id = (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry')
  ),
  'natural expiry visit_ended uses the canonical lease end and exact safe fields'
);
SELECT private.maintain_visits();
SELECT is(
  (
    SELECT count(*)
    FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000003:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'carol_bob_expiry')::text
      AND event = 'visit_ended'
  ),
  1::bigint,
  'repeated maintenance does not duplicate the natural-expiry visit_ended Broadcast'
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
INSERT INTO visit_test_state (name, visit_id)
SELECT 'alice_bob_unfriend', (public.request_visit(
  '00000000-0000-0000-0000-000000000002',
  'visit-request-alice-bob-0003'
)).id;
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000002');
SELECT lives_ok(
  format(
    'SELECT public.accept_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_unfriend'),
    'visit-accept-alice-bob-0003'
  ),
  'host accepts the visit used to test unfriend revocation'
);
DELETE FROM public.friendships
WHERE user_a = '00000000-0000-0000-0000-000000000001'
  AND user_b = '00000000-0000-0000-0000-000000000002';
SELECT is(
  (SELECT status::text FROM public.visits WHERE id = (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_unfriend')),
  'recalled',
  'unfriend recalls an active visit in the same transaction'
);
SELECT ok(
  NOT private.can_receive_visit_topic(
    '00000000-0000-0000-0000-000000000002',
    'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_unfriend')::text
  ),
  'topic authorization remains off after unfriend'
);
SELECT ok(
  NOT EXISTS (
    SELECT 1 FROM private.authorized_projection_visits(
      '00000000-0000-0000-0000-000000000001'
    )
  ),
  'projection publishing remains off after unfriend'
);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
SELECT lives_ok(
  $$SELECT public.update_projection('alice-pet', 'yoonie', 'offline')$$,
  'projection storage remains writable after unfriend'
);
SELECT is(
  (
    SELECT count(*) FROM realtime.messages
    WHERE topic = 'pet:00000000-0000-0000-0000-000000000001:' ||
      (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_unfriend')::text
      AND event = 'projection_updated'
  ),
  0::bigint,
  'post-unfriend projection update emits no Broadcast'
);

INSERT INTO public.invite_codes (code_hash, issued_by, max_uses, expires_at)
VALUES (
  extensions.digest('PAIR-CODE-001', 'sha256'),
  '00000000-0000-0000-0000-000000000001',
  1,
  clock_timestamp() + interval '7 days'
);

SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000003');
SELECT is(
  (public.redeem_invite('PAIR-CODE-001', 'invite-redeem-carol-0001')).user_id,
  '00000000-0000-0000-0000-000000000003'::uuid,
  'redeem_invite records the authenticated redeemer'
);
SELECT is(
  (public.redeem_invite('PAIR-CODE-001', 'invite-redeem-carol-0001')).id,
  (SELECT id FROM public.invite_redemptions WHERE user_id = '00000000-0000-0000-0000-000000000003'),
  'invite redemption replays without consuming another use'
);
SELECT is(
  (SELECT use_count FROM public.invite_codes WHERE code_hash = extensions.digest('PAIR-CODE-001', 'sha256')),
  1::smallint,
  'idempotent invite replay increments usage only once'
);

DELETE FROM private.rate_limits
WHERE actor_id = '00000000-0000-0000-0000-000000000001'
  AND action IN ('visit_request', 'visit_mutation', 'projection_update');
SELECT private.consume_rate_limit(
  '00000000-0000-0000-0000-000000000001', 'visit_request', 'all', 30, interval '1 hour'
) FROM generate_series(1, 30);
SELECT private.consume_rate_limit(
  '00000000-0000-0000-0000-000000000001', 'visit_mutation', 'all', 30, interval '1 hour'
) FROM generate_series(1, 30);
SELECT private.consume_rate_limit(
  '00000000-0000-0000-0000-000000000001', 'projection_update', 'all', 120, interval '1 minute'
) FROM generate_series(1, 120);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000001');
SELECT throws_ok(
  $$SELECT public.request_visit(
      '00000000-0000-0000-0000-000000000003', 'visit-request-rate-limited'
    )$$,
  'P0001', 'rate_limit_exceeded',
  'visit requests enforce the global 30/hour actor limit'
);
SELECT throws_ok(
  format(
    'SELECT public.recall_visit(%L::uuid, %L)',
    (SELECT visit_id FROM visit_test_state WHERE name = 'alice_bob_unfriend'),
    'visit-mutation-rate-limited'
  ),
  'P0001', 'rate_limit_exceeded',
  'visit transitions enforce the global 30/hour actor limit'
);
SELECT throws_ok(
  $$SELECT public.update_projection('alice-pet', 'yoonie', 'idle')$$,
  'P0001', 'rate_limit_exceeded',
  'projection updates enforce the global 120/minute actor limit'
);

DELETE FROM private.rate_limits
WHERE actor_id = '00000000-0000-0000-0000-000000000003'
  AND action = 'social_mutation';
SELECT private.consume_rate_limit(
  '00000000-0000-0000-0000-000000000003', 'social_mutation', 'all', 60, interval '1 hour'
) FROM generate_series(1, 60);
SELECT pg_temp.set_actor('00000000-0000-0000-0000-000000000003');
SELECT throws_ok(
  $$SELECT public.redeem_invite('PAIR-CODE-001', 'invite-redeem-rate-limited')$$,
  'P0001', 'rate_limit_exceeded',
  'invite redemption enforces the global social mutation limit'
);

SET LOCAL ROLE authenticated;
SELECT throws_ok(
  $$INSERT INTO public.pet_projections (
      owner_user_id, pet_id, version, display_name, skin_id, status
    ) VALUES (
      '00000000-0000-0000-0000-000000000003', 'forged-pet', 1, 'Forged', 'yoonie', 'idle'
    )$$,
  '42501',
  'permission denied for table pet_projections',
  'projection rows cannot be written directly by clients'
);
RESET ROLE;

SELECT * FROM finish();
ROLLBACK;
