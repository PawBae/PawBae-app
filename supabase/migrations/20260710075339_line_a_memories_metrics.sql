BEGIN;

CREATE TYPE public.memory_template_key AS ENUM (
  'played_together',
  'worked_together',
  'celebrated_completion',
  'shared_snack'
);

CREATE TYPE private.funnel_event_type AS ENUM (
  'friend_request_sent',
  'friend_request_accepted',
  'visit_requested',
  'visit_completed',
  'memory_created',
  'memory_viewed'
);

CREATE TABLE public.shared_memories (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  visit_id uuid NOT NULL UNIQUE REFERENCES public.visits(id) ON DELETE RESTRICT,
  visitor_user_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  host_user_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  template_key public.memory_template_key NOT NULL,
  params jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT shared_memory_distinct_participants CHECK (visitor_user_id <> host_user_id)
);

CREATE INDEX shared_memories_visitor_created_idx
ON public.shared_memories (visitor_user_id, created_at DESC);

CREATE INDEX shared_memories_host_created_idx
ON public.shared_memories (host_user_id, created_at DESC);

CREATE TABLE private.funnel_events (
  id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  event_type private.funnel_event_type NOT NULL,
  actor_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  counterpart_id uuid REFERENCES public.profiles(id) ON DELETE CASCADE,
  pair_user_a uuid,
  pair_user_b uuid,
  attempt_id uuid,
  visit_id uuid,
  memory_id uuid,
  dedupe_key text NOT NULL UNIQUE,
  occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT funnel_pair_order CHECK (
    (pair_user_a IS NULL AND pair_user_b IS NULL)
    OR (pair_user_a IS NOT NULL AND pair_user_b IS NOT NULL AND pair_user_a < pair_user_b)
  ),
  CONSTRAINT funnel_dedupe_shape CHECK (
    dedupe_key ~ '^[a-z][a-z0-9_:.-]{7,255}$'
  )
);

CREATE INDEX funnel_events_type_time_idx
ON private.funnel_events (event_type, occurred_at);

CREATE INDEX funnel_events_actor_idx
ON private.funnel_events (actor_id);

CREATE INDEX funnel_events_counterpart_idx
ON private.funnel_events (counterpart_id)
WHERE counterpart_id IS NOT NULL;

CREATE INDEX funnel_events_pair_type_time_idx
ON private.funnel_events (pair_user_a, pair_user_b, event_type, occurred_at);

CREATE INDEX funnel_events_attempt_type_idx
ON private.funnel_events (attempt_id, event_type)
WHERE attempt_id IS NOT NULL;

CREATE INDEX funnel_events_visit_type_idx
ON private.funnel_events (visit_id, event_type);

CREATE INDEX funnel_events_memory_type_idx
ON private.funnel_events (memory_id, event_type);

ALTER TABLE private.funnel_events ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON private.funnel_events FROM PUBLIC, anon, authenticated;
CREATE POLICY funnel_events_no_client_access
ON private.funnel_events AS RESTRICTIVE FOR ALL TO anon, authenticated
USING (false) WITH CHECK (false);

CREATE OR REPLACE FUNCTION private.valid_memory_params(p_params jsonb)
RETURNS boolean
LANGUAGE sql
IMMUTABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT jsonb_typeof(p_params) = 'object'
    AND (SELECT count(*) FROM jsonb_object_keys(p_params)) = 3
    AND p_params ?& ARRAY['durationBucket', 'timeOfDay', 'interactionCount']
    AND p_params->>'durationBucket' IN ('short', 'full')
    AND p_params->>'timeOfDay' IN ('morning', 'afternoon', 'evening', 'night')
    AND jsonb_typeof(p_params->'interactionCount') = 'number'
    AND (p_params->>'interactionCount') ~ '^[0-9]+$'
    AND (p_params->>'interactionCount')::integer BETWEEN 0 AND 100;
$$;

REVOKE ALL ON FUNCTION private.valid_memory_params(jsonb) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.validate_shared_memory()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_visit public.visits%ROWTYPE;
BEGIN
  SELECT * INTO v_visit
  FROM public.visits
  WHERE id = NEW.visit_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'memory visit not found';
  END IF;
  IF NEW.visitor_user_id <> v_visit.visitor_user_id
     OR NEW.host_user_id <> v_visit.host_user_id THEN
    RAISE EXCEPTION 'memory participants must match the visit';
  END IF;
  IF v_visit.status NOT IN ('completed', 'recalled') OR v_visit.started_at IS NULL THEN
    RAISE EXCEPTION 'only a completed or recalled started visit can settle a memory';
  END IF;
  IF NOT private.valid_memory_params(NEW.params) THEN
    RAISE EXCEPTION 'invalid shared memory parameters';
  END IF;
  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION private.protect_shared_memory()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  RAISE EXCEPTION 'shared memories are append-only';
END;
$$;

REVOKE ALL ON FUNCTION private.validate_shared_memory() FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protect_shared_memory() FROM PUBLIC, anon, authenticated;

CREATE TRIGGER shared_memories_validate
BEFORE INSERT ON public.shared_memories
FOR EACH ROW EXECUTE FUNCTION private.validate_shared_memory();

CREATE TRIGGER shared_memories_append_only
BEFORE UPDATE ON public.shared_memories
FOR EACH ROW EXECUTE FUNCTION private.protect_shared_memory();

CREATE OR REPLACE FUNCTION private.log_funnel_event(
  p_event_type private.funnel_event_type,
  p_actor uuid,
  p_counterpart uuid,
  p_visit_id uuid,
  p_memory_id uuid,
  p_dedupe_key text,
  p_occurred_at timestamptz DEFAULT clock_timestamp(),
  p_attempt_id uuid DEFAULT NULL
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_pair_a uuid;
  v_pair_b uuid;
BEGIN
  IF p_actor IS NULL OR p_dedupe_key IS NULL THEN
    RAISE EXCEPTION 'funnel event actor and key are required';
  END IF;
  IF p_counterpart IS NOT NULL THEN
    v_pair_a := LEAST(p_actor, p_counterpart);
    v_pair_b := GREATEST(p_actor, p_counterpart);
  END IF;

  INSERT INTO private.funnel_events (
    event_type, actor_id, counterpart_id, pair_user_a, pair_user_b, attempt_id,
    visit_id, memory_id, dedupe_key, occurred_at
  )
  VALUES (
    p_event_type, p_actor, p_counterpart, v_pair_a, v_pair_b, p_attempt_id,
    p_visit_id, p_memory_id, p_dedupe_key, p_occurred_at
  )
  ON CONFLICT (dedupe_key) DO NOTHING;
END;
$$;

REVOKE ALL ON FUNCTION private.log_funnel_event(
  private.funnel_event_type, uuid, uuid, uuid, uuid, text, timestamptz, uuid
) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.track_friendship_funnel()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_counterpart uuid;
  v_attempt_id uuid;
BEGIN
  v_counterpart := CASE
    WHEN NEW.requester_id = NEW.user_a THEN NEW.user_b
    ELSE NEW.user_a
  END;

  IF TG_OP = 'INSERT' THEN
    v_attempt_id := gen_random_uuid();
    PERFORM private.log_funnel_event(
      'friend_request_sent', NEW.requester_id, v_counterpart,
      NULL, NULL,
      'friend_request_sent:' || NEW.user_a::text || ':' || NEW.user_b::text || ':' || txid_current()::text,
      NEW.created_at,
      v_attempt_id
    );
    IF NEW.status::text = 'accepted' THEN
      PERFORM private.log_funnel_event(
        'friend_request_accepted', v_counterpart, NEW.requester_id,
        NULL, NULL,
        'friend_request_accepted:' || NEW.user_a::text || ':' || NEW.user_b::text || ':' || txid_current()::text,
        COALESCE(NEW.accepted_at, clock_timestamp()),
        v_attempt_id
      );
    END IF;
  ELSIF OLD.status::text <> 'accepted' AND NEW.status::text = 'accepted' THEN
    SELECT attempt_id INTO v_attempt_id
    FROM private.funnel_events
    WHERE event_type = 'friend_request_sent'
      AND pair_user_a = NEW.user_a
      AND pair_user_b = NEW.user_b
    ORDER BY occurred_at DESC, id DESC
    LIMIT 1;
    v_attempt_id := COALESCE(v_attempt_id, gen_random_uuid());
    PERFORM private.log_funnel_event(
      'friend_request_accepted', v_counterpart, NEW.requester_id,
      NULL, NULL,
      'friend_request_accepted:' || NEW.user_a::text || ':' || NEW.user_b::text || ':' || txid_current()::text,
      COALESCE(NEW.accepted_at, clock_timestamp()),
      v_attempt_id
    );
  END IF;
  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION private.track_visit_funnel()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    PERFORM private.log_funnel_event(
      'visit_requested', NEW.visitor_user_id, NEW.host_user_id,
      NEW.id, NULL, 'visit_requested:' || NEW.id::text, NEW.requested_at
    );
    IF NEW.status = 'completed' THEN
      PERFORM private.log_funnel_event(
        'visit_completed', NEW.visitor_user_id, NEW.host_user_id,
        NEW.id, NULL, 'visit_completed:' || NEW.id::text,
        COALESCE(NEW.ended_at, clock_timestamp())
      );
    END IF;
  ELSIF OLD.status <> 'completed' AND NEW.status = 'completed' THEN
    PERFORM private.log_funnel_event(
      'visit_completed', NEW.visitor_user_id, NEW.host_user_id,
      NEW.id, NULL, 'visit_completed:' || NEW.id::text,
      COALESCE(NEW.ended_at, clock_timestamp())
    );
  END IF;
  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION private.track_memory_created()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.log_funnel_event(
    'memory_created', NEW.visitor_user_id, NEW.host_user_id,
    NEW.visit_id, NEW.id, 'memory_created:' || NEW.id::text, NEW.created_at
  );
  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.track_friendship_funnel() FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.track_visit_funnel() FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.track_memory_created() FROM PUBLIC, anon, authenticated;

CREATE TRIGGER friendships_track_funnel
AFTER INSERT OR UPDATE OF status ON public.friendships
FOR EACH ROW EXECUTE FUNCTION private.track_friendship_funnel();

CREATE TRIGGER visits_track_funnel
AFTER INSERT OR UPDATE OF status ON public.visits
FOR EACH ROW EXECUTE FUNCTION private.track_visit_funnel();

CREATE TRIGGER shared_memories_track_funnel
AFTER INSERT ON public.shared_memories
FOR EACH ROW EXECUTE FUNCTION private.track_memory_created();

CREATE OR REPLACE FUNCTION private.settle_shared_memory_impl(
  p_visit_id uuid,
  p_idempotency_key text
)
RETURNS public.shared_memories
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor uuid := (SELECT auth.uid());
  v_replay jsonb;
  v_visit public.visits%ROWTYPE;
  v_result public.shared_memories%ROWTYPE;
  v_duration_bucket text;
  v_time_of_day text;
  v_hour integer;
BEGIN
  IF v_actor IS NULL THEN
    RAISE EXCEPTION 'authenticated actor required';
  END IF;

  v_replay := private.claim_idempotency(v_actor, p_idempotency_key, 'settle_shared_memory');
  IF v_replay IS NOT NULL THEN
    SELECT * INTO v_result FROM jsonb_populate_record(NULL::public.shared_memories, v_replay);
    RETURN v_result;
  END IF;

  PERFORM private.consume_rate_limit(
    v_actor, 'social_mutation', 'all', 60, interval '1 hour'
  );

  SELECT * INTO v_visit
  FROM public.visits
  WHERE id = p_visit_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'memory visit not found';
  END IF;
  IF v_actor NOT IN (v_visit.visitor_user_id, v_visit.host_user_id) THEN
    RAISE EXCEPTION 'only visit participants can settle its memory';
  END IF;
  IF v_visit.status NOT IN ('completed', 'recalled') OR v_visit.started_at IS NULL THEN
    RAISE EXCEPTION 'visit is not eligible for memory settlement';
  END IF;

  SELECT * INTO v_result
  FROM public.shared_memories
  WHERE visit_id = p_visit_id
  FOR UPDATE;

  IF FOUND THEN
    PERFORM private.finish_idempotency(
      v_actor, p_idempotency_key, 'settle_shared_memory', to_jsonb(v_result)
    );
    RETURN v_result;
  END IF;

  v_duration_bucket := CASE
    WHEN COALESCE(v_visit.ended_at, v_visit.ends_at) - v_visit.started_at >= interval '25 minutes'
      THEN 'full'
    ELSE 'short'
  END;
  v_hour := extract(hour FROM (v_visit.started_at AT TIME ZONE 'UTC'))::integer;
  v_time_of_day := CASE
    WHEN v_hour BETWEEN 5 AND 11 THEN 'morning'
    WHEN v_hour BETWEEN 12 AND 16 THEN 'afternoon'
    WHEN v_hour BETWEEN 17 AND 21 THEN 'evening'
    ELSE 'night'
  END;

  INSERT INTO public.shared_memories (
    visit_id, visitor_user_id, host_user_id, template_key, params
  )
  VALUES (
    v_visit.id,
    v_visit.visitor_user_id,
    v_visit.host_user_id,
    'played_together',
    jsonb_build_object(
      'durationBucket', v_duration_bucket,
      'timeOfDay', v_time_of_day,
      'interactionCount', 0
    )
  )
  RETURNING * INTO v_result;

  PERFORM private.finish_idempotency(
    v_actor, p_idempotency_key, 'settle_shared_memory', to_jsonb(v_result)
  );
  RETURN v_result;
END;
$$;

CREATE OR REPLACE FUNCTION private.record_memory_view_impl(
  p_memory_id uuid,
  p_idempotency_key text
)
RETURNS public.shared_memories
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor uuid := (SELECT auth.uid());
  v_replay jsonb;
  v_memory public.shared_memories%ROWTYPE;
  v_counterpart uuid;
BEGIN
  IF v_actor IS NULL THEN
    RAISE EXCEPTION 'authenticated actor required';
  END IF;

  v_replay := private.claim_idempotency(v_actor, p_idempotency_key, 'record_memory_view');
  IF v_replay IS NOT NULL THEN
    SELECT * INTO v_memory FROM jsonb_populate_record(NULL::public.shared_memories, v_replay);
    RETURN v_memory;
  END IF;

  PERFORM private.consume_rate_limit(
    v_actor, 'social_mutation', 'all', 60, interval '1 hour'
  );

  SELECT * INTO v_memory
  FROM public.shared_memories
  WHERE id = p_memory_id;

  IF NOT FOUND OR v_actor NOT IN (v_memory.visitor_user_id, v_memory.host_user_id) THEN
    RAISE EXCEPTION 'shared memory not found for participant';
  END IF;

  v_counterpart := CASE
    WHEN v_actor = v_memory.visitor_user_id THEN v_memory.host_user_id
    ELSE v_memory.visitor_user_id
  END;
  PERFORM private.log_funnel_event(
    'memory_viewed', v_actor, v_counterpart,
    v_memory.visit_id, v_memory.id,
    'memory_viewed:' || v_memory.id::text || ':' || v_actor::text,
    clock_timestamp()
  );

  PERFORM private.finish_idempotency(
    v_actor, p_idempotency_key, 'record_memory_view', to_jsonb(v_memory)
  );
  RETURN v_memory;
END;
$$;

REVOKE ALL ON FUNCTION private.settle_shared_memory_impl(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.record_memory_view_impl(uuid, text) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION public.settle_shared_memory(p_visit_id uuid, p_idempotency_key text)
RETURNS public.shared_memories
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.settle_shared_memory_impl(p_visit_id, p_idempotency_key); $$;

CREATE OR REPLACE FUNCTION public.record_memory_view(p_memory_id uuid, p_idempotency_key text)
RETURNS public.shared_memories
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.record_memory_view_impl(p_memory_id, p_idempotency_key); $$;

ALTER TABLE public.shared_memories ENABLE ROW LEVEL SECURITY;

CREATE POLICY shared_memories_participants_select
ON public.shared_memories FOR SELECT TO authenticated
USING ((SELECT auth.uid()) IN (visitor_user_id, host_user_id));

REVOKE ALL ON public.shared_memories FROM PUBLIC, anon, authenticated;
GRANT SELECT ON public.shared_memories TO authenticated;

REVOKE ALL ON FUNCTION public.settle_shared_memory(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.record_memory_view(uuid, text) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.settle_shared_memory(uuid, text) TO authenticated;
GRANT EXECUTE ON FUNCTION public.record_memory_view(uuid, text) TO authenticated;

CREATE VIEW public.funnel_friend_request_acceptance
WITH (security_invoker = true)
AS
SELECT
  sent.pair_user_a,
  sent.pair_user_b,
  sent.occurred_at AS request_sent_at,
  accepted.occurred_at AS accepted_at,
  accepted.id IS NOT NULL AS converted
FROM private.funnel_events AS sent
LEFT JOIN LATERAL (
  SELECT candidate.id, candidate.occurred_at
  FROM private.funnel_events AS candidate
  WHERE candidate.event_type = 'friend_request_accepted'
    AND candidate.attempt_id = sent.attempt_id
    AND candidate.occurred_at >= sent.occurred_at
  ORDER BY candidate.occurred_at
  LIMIT 1
) AS accepted ON true
WHERE sent.event_type = 'friend_request_sent';

CREATE VIEW public.funnel_friend_to_first_visit
WITH (security_invoker = true)
AS
SELECT
  accepted.pair_user_a,
  accepted.pair_user_b,
  accepted.occurred_at AS friendship_accepted_at,
  first_visit.occurred_at AS first_visit_requested_at,
  first_visit.id IS NOT NULL AS converted
FROM private.funnel_events AS accepted
LEFT JOIN LATERAL (
  SELECT candidate.id, candidate.occurred_at
  FROM private.funnel_events AS candidate
  WHERE candidate.event_type = 'visit_requested'
    AND candidate.pair_user_a = accepted.pair_user_a
    AND candidate.pair_user_b = accepted.pair_user_b
    AND candidate.occurred_at >= accepted.occurred_at
  ORDER BY candidate.occurred_at
  LIMIT 1
) AS first_visit ON true
WHERE accepted.event_type = 'friend_request_accepted';

CREATE VIEW public.funnel_visit_completion
WITH (security_invoker = true)
AS
SELECT
  requested.visit_id,
  requested.pair_user_a,
  requested.pair_user_b,
  requested.occurred_at AS visit_requested_at,
  completed.occurred_at AS visit_completed_at,
  completed.id IS NOT NULL AS converted
FROM private.funnel_events AS requested
LEFT JOIN private.funnel_events AS completed
  ON completed.event_type = 'visit_completed'
 AND completed.visit_id = requested.visit_id
WHERE requested.event_type = 'visit_requested';

CREATE VIEW public.funnel_memory_view
WITH (security_invoker = true)
AS
SELECT
  completed.visit_id,
  memory.memory_id,
  completed.occurred_at AS visit_completed_at,
  first_view.occurred_at AS first_memory_viewed_at,
  first_view.id IS NOT NULL AS converted
FROM private.funnel_events AS completed
LEFT JOIN private.funnel_events AS memory
  ON memory.event_type = 'memory_created'
 AND memory.visit_id = completed.visit_id
LEFT JOIN LATERAL (
  SELECT candidate.id, candidate.occurred_at
  FROM private.funnel_events AS candidate
  WHERE candidate.event_type = 'memory_viewed'
    AND candidate.memory_id = memory.memory_id
    AND candidate.occurred_at >= memory.occurred_at
  ORDER BY candidate.occurred_at
  LIMIT 1
) AS first_view ON true
WHERE completed.event_type = 'visit_completed';

CREATE VIEW public.funnel_seven_day_repeat_visit
WITH (security_invoker = true)
AS
WITH ranked AS (
  SELECT
    pair_user_a,
    pair_user_b,
    visit_id,
    occurred_at,
    row_number() OVER (
      PARTITION BY pair_user_a, pair_user_b ORDER BY occurred_at, id
    ) AS visit_number
  FROM private.funnel_events
  WHERE event_type = 'visit_completed'
), first_visits AS (
  SELECT * FROM ranked WHERE visit_number = 1
)
SELECT
  first_visits.pair_user_a,
  first_visits.pair_user_b,
  first_visits.visit_id AS first_visit_id,
  first_visits.occurred_at AS first_visit_completed_at,
  repeat_visit.visit_id AS repeat_visit_id,
  repeat_visit.occurred_at AS repeat_visit_completed_at,
  repeat_visit.visit_id IS NOT NULL AS converted
FROM first_visits
LEFT JOIN LATERAL (
  SELECT candidate.visit_id, candidate.occurred_at
  FROM ranked AS candidate
  WHERE candidate.pair_user_a = first_visits.pair_user_a
    AND candidate.pair_user_b = first_visits.pair_user_b
    AND candidate.visit_number > 1
    AND candidate.occurred_at <= first_visits.occurred_at + interval '7 days'
  ORDER BY candidate.occurred_at
  LIMIT 1
) AS repeat_visit ON true;

REVOKE ALL ON public.funnel_friend_request_acceptance FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.funnel_friend_to_first_visit FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.funnel_visit_completion FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.funnel_memory_view FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.funnel_seven_day_repeat_visit FROM PUBLIC, anon, authenticated;
COMMIT;
