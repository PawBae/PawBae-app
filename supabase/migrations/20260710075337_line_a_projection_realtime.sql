BEGIN;

CREATE TYPE public.projection_status AS ENUM (
  'idle',
  'working',
  'waiting',
  'compacting',
  'offline'
);

CREATE TABLE private.approved_skins (
  skin_id text PRIMARY KEY,
  CONSTRAINT approved_skin_id_shape CHECK (
    skin_id ~ '^[a-z0-9][a-z0-9._-]{0,63}$'
  )
);

INSERT INTO private.approved_skins (skin_id)
VALUES
  ('doro.codex-pet'),
  ('elaina-2'),
  ('homie'),
  ('linnea-2'),
  ('mambo'),
  ('naruto'),
  ('nezuko'),
  ('phoebe.codex-pet'),
  ('shimeji-bola'),
  ('skirk-2'),
  ('taffy'),
  ('wukong'),
  ('yoonie');

ALTER TABLE private.approved_skins ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON private.approved_skins FROM PUBLIC, anon, authenticated;
CREATE POLICY approved_skins_no_client_access
ON private.approved_skins AS RESTRICTIVE FOR ALL TO anon, authenticated
USING (false) WITH CHECK (false);

CREATE TABLE public.pet_projections (
  owner_user_id uuid PRIMARY KEY REFERENCES public.profiles(id) ON DELETE CASCADE,
  pet_id text NOT NULL,
  version smallint NOT NULL DEFAULT 1 CHECK (version = 1),
  display_name text NOT NULL,
  skin_id text NOT NULL REFERENCES private.approved_skins(skin_id) ON DELETE RESTRICT,
  status public.projection_status NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT projection_pet_id_shape CHECK (
    pet_id ~ '^[a-z0-9][a-z0-9._-]{0,63}$'
  ),
  CONSTRAINT projection_skin_id_shape CHECK (
    skin_id ~ '^[a-z0-9][a-z0-9._-]{0,63}$'
  ),
  CONSTRAINT projection_display_name_length CHECK (
    char_length(display_name) BETWEEN 1 AND 64
  )
);

CREATE INDEX pet_projections_updated_at_idx
ON public.pet_projections (updated_at DESC);

CREATE INDEX pet_projections_skin_id_idx
ON public.pet_projections (skin_id);

CREATE OR REPLACE FUNCTION private.can_receive_visit_topic(p_actor uuid, p_topic text)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM public.visits AS v
    WHERE p_actor = v.host_user_id
      AND p_topic = 'pet:' || v.visitor_user_id::text || ':' || v.id::text
      AND v.status IN ('accepted', 'traveling', 'visiting')
      AND v.ends_at > statement_timestamp()
      AND private.are_accepted_friends(v.visitor_user_id, v.host_user_id)
      AND NOT private.is_blocked(v.visitor_user_id, v.host_user_id)
  );
$$;

CREATE OR REPLACE FUNCTION private.can_read_projection(p_actor uuid, p_owner uuid)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT p_actor = p_owner OR EXISTS (
    SELECT 1
    FROM public.visits AS v
    WHERE v.visitor_user_id = p_owner
      AND v.host_user_id = p_actor
      AND v.status IN ('accepted', 'traveling', 'visiting')
      AND v.ends_at > statement_timestamp()
      AND private.are_accepted_friends(v.visitor_user_id, v.host_user_id)
      AND NOT private.is_blocked(v.visitor_user_id, v.host_user_id)
  );
$$;

REVOKE ALL ON FUNCTION private.can_receive_visit_topic(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.can_read_projection(uuid, uuid) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.authorized_projection_visits(p_owner uuid)
RETURNS SETOF public.visits
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT v.*
  FROM public.visits AS v
  WHERE v.visitor_user_id = p_owner
    AND v.status IN ('accepted', 'traveling', 'visiting')
    AND v.ends_at > statement_timestamp()
    AND private.are_accepted_friends(v.visitor_user_id, v.host_user_id)
    AND NOT private.is_blocked(v.visitor_user_id, v.host_user_id);
$$;

REVOKE ALL ON FUNCTION private.authorized_projection_visits(uuid) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.lock_projection_visit(p_owner uuid)
RETURNS public.visits
LANGUAGE plpgsql
VOLATILE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_candidate public.visits%ROWTYPE;
  v_locked public.visits%ROWTYPE;
BEGIN
  SELECT * INTO v_candidate
  FROM private.authorized_projection_visits(p_owner)
  ORDER BY id
  LIMIT 1;

  IF NOT FOUND THEN
    RETURN NULL;
  END IF;

  PERFORM private.lock_social_pair(v_candidate.visitor_user_id, v_candidate.host_user_id);

  SELECT * INTO v_locked
  FROM public.visits
  WHERE id = v_candidate.id
  FOR UPDATE;

  IF NOT FOUND
     OR v_locked.status NOT IN ('accepted', 'traveling', 'visiting')
     OR v_locked.ends_at <= clock_timestamp()
     OR NOT private.are_accepted_friends(v_locked.visitor_user_id, v_locked.host_user_id)
     OR private.is_blocked(v_locked.visitor_user_id, v_locked.host_user_id) THEN
    RETURN NULL;
  END IF;

  RETURN v_locked;
END;
$$;

REVOKE ALL ON FUNCTION private.lock_projection_visit(uuid) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION public.authorize_visit_topic()
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT private.can_receive_visit_topic((SELECT auth.uid()), (SELECT realtime.topic()));
$$;

CREATE OR REPLACE FUNCTION public.authorize_projection_read(p_owner_user_id uuid)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT private.can_read_projection((SELECT auth.uid()), p_owner_user_id);
$$;

CREATE OR REPLACE FUNCTION private.update_projection_impl(
  p_pet_id text,
  p_skin_id text,
  p_status public.projection_status
)
RETURNS public.pet_projections
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor uuid := (SELECT auth.uid());
  v_display_name text;
  v_result public.pet_projections%ROWTYPE;
BEGIN
  IF v_actor IS NULL THEN
    RAISE EXCEPTION 'authenticated actor required';
  END IF;
  IF p_pet_id IS NULL OR p_pet_id !~ '^[a-z0-9][a-z0-9._-]{0,63}$' THEN
    RAISE EXCEPTION 'invalid pet id';
  END IF;
  IF p_skin_id IS NULL OR p_skin_id !~ '^[a-z0-9][a-z0-9._-]{0,63}$'
     OR NOT EXISTS (SELECT 1 FROM private.approved_skins WHERE skin_id = p_skin_id) THEN
    RAISE EXCEPTION 'skin is not approved for public projection';
  END IF;

  SELECT COALESCE(NULLIF(display_name, ''), handle)
  INTO v_display_name
  FROM public.profiles
  WHERE id = v_actor;

  IF v_display_name IS NULL THEN
    RAISE EXCEPTION 'profile not found';
  END IF;

  PERFORM private.consume_rate_limit(
    v_actor, 'projection_update', 'all', 120, interval '1 minute'
  );

  -- Linearize projection publication with recall/end/unfriend/block. The pair
  -- advisory lock is held through the upsert, its AFTER trigger, and commit.
  PERFORM private.lock_projection_visit(v_actor);

  INSERT INTO public.pet_projections (
    owner_user_id, pet_id, version, display_name, skin_id, status, updated_at
  )
  VALUES (
    v_actor, p_pet_id, 1, v_display_name, p_skin_id, p_status, clock_timestamp()
  )
  ON CONFLICT (owner_user_id) DO UPDATE
  SET pet_id = EXCLUDED.pet_id,
      version = 1,
      display_name = EXCLUDED.display_name,
      skin_id = EXCLUDED.skin_id,
      status = EXCLUDED.status,
      updated_at = EXCLUDED.updated_at
  RETURNING * INTO v_result;

  RETURN v_result;
END;
$$;

REVOKE ALL ON FUNCTION private.update_projection_impl(text, text, public.projection_status) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION public.update_projection(
  p_pet_id text,
  p_skin_id text,
  p_status public.projection_status
)
RETURNS public.pet_projections
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.update_projection_impl(p_pet_id, p_skin_id, p_status); $$;

CREATE OR REPLACE FUNCTION private.broadcast_projection_change()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_visit public.visits%ROWTYPE;
BEGIN
  v_visit := private.lock_projection_visit(NEW.owner_user_id);
  IF v_visit.id IS NOT NULL THEN
    PERFORM realtime.send(
      jsonb_build_object(
        'v', NEW.version,
        'petId', NEW.pet_id,
        'displayName', NEW.display_name,
        'skinId', NEW.skin_id,
        'status', NEW.status,
        'updatedAt', NEW.updated_at
      ),
      'projection_updated',
      'pet:' || NEW.owner_user_id::text || ':' || v_visit.id::text,
      true
    );
  END IF;
  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION private.broadcast_visit_ended()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_ended_at timestamptz;
BEGIN
  IF NEW.status = OLD.status
     OR OLD.status NOT IN ('accepted', 'traveling', 'visiting')
     OR NEW.status NOT IN ('returning', 'completed', 'recalled', 'blocked') THEN
    RETURN NEW;
  END IF;

  IF (OLD.ends_at > v_now OR NEW.status = 'completed')
     AND private.are_accepted_friends(OLD.visitor_user_id, OLD.host_user_id)
     AND NOT private.is_blocked(OLD.visitor_user_id, OLD.host_user_id) THEN
    v_ended_at := CASE
      WHEN NEW.status = 'completed' AND OLD.ends_at <= v_now THEN OLD.ends_at
      ELSE COALESCE(NEW.ended_at, NEW.returning_started_at, v_now)
    END;

    PERFORM realtime.send(
      jsonb_build_object(
        'leaseId', NEW.id,
        'status', NEW.status,
        'endedAt', v_ended_at
      ),
      'visit_ended',
      'pet:' || OLD.visitor_user_id::text || ':' || OLD.id::text,
      true
    );
  END IF;
  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.broadcast_projection_change() FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.broadcast_visit_ended() FROM PUBLIC, anon, authenticated;

CREATE TRIGGER pet_projections_broadcast_change
AFTER INSERT OR UPDATE ON public.pet_projections
FOR EACH ROW EXECUTE FUNCTION private.broadcast_projection_change();

CREATE TRIGGER visits_broadcast_end
AFTER UPDATE OF status ON public.visits
FOR EACH ROW EXECUTE FUNCTION private.broadcast_visit_ended();

CREATE OR REPLACE FUNCTION private.terminate_visits_on_block()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  UPDATE public.visits
  SET status = 'blocked'
  WHERE status IN ('requested', 'accepted', 'traveling', 'visiting', 'returning')
    AND (
      (visitor_user_id = NEW.blocker_id AND host_user_id = NEW.blocked_id)
      OR (visitor_user_id = NEW.blocked_id AND host_user_id = NEW.blocker_id)
    );
  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION private.terminate_visits_on_unfriend()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  UPDATE public.visits
  SET status = CASE
    WHEN EXISTS (
      SELECT 1 FROM public.blocks
      WHERE (blocker_id = OLD.user_a AND blocked_id = OLD.user_b)
         OR (blocker_id = OLD.user_b AND blocked_id = OLD.user_a)
    ) THEN 'blocked'::public.visit_status
    WHEN status = 'requested' THEN 'cancelled'::public.visit_status
    ELSE 'recalled'::public.visit_status
  END
  WHERE status IN ('requested', 'accepted', 'traveling', 'visiting', 'returning')
    AND visitor_user_id IN (OLD.user_a, OLD.user_b)
    AND host_user_id IN (OLD.user_a, OLD.user_b);
  RETURN OLD;
END;
$$;

REVOKE ALL ON FUNCTION private.terminate_visits_on_block() FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.terminate_visits_on_unfriend() FROM PUBLIC, anon, authenticated;

CREATE TRIGGER blocks_terminate_visits
AFTER INSERT ON public.blocks
FOR EACH ROW EXECUTE FUNCTION private.terminate_visits_on_block();

CREATE TRIGGER friendships_terminate_visits
AFTER DELETE ON public.friendships
FOR EACH ROW EXECUTE FUNCTION private.terminate_visits_on_unfriend();

ALTER TABLE public.visits ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.invite_codes ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.invite_redemptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.pet_projections ENABLE ROW LEVEL SECURITY;

CREATE POLICY visits_participants_select
ON public.visits FOR SELECT TO authenticated
USING ((SELECT auth.uid()) IN (visitor_user_id, host_user_id));

CREATE POLICY invite_codes_issuer_select
ON public.invite_codes FOR SELECT TO authenticated
USING ((SELECT auth.uid()) = issued_by);

CREATE POLICY invite_redemptions_owner_select
ON public.invite_redemptions FOR SELECT TO authenticated
USING ((SELECT auth.uid()) = user_id);

CREATE POLICY pet_projections_authorized_select
ON public.pet_projections FOR SELECT TO authenticated
USING ((SELECT public.authorize_projection_read(owner_user_id)));

REVOKE ALL ON public.visits FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.invite_codes FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.invite_redemptions FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.pet_projections FROM PUBLIC, anon, authenticated;
GRANT SELECT ON public.visits TO authenticated;
GRANT SELECT ON public.invite_codes TO authenticated;
GRANT SELECT ON public.invite_redemptions TO authenticated;
GRANT SELECT ON public.pet_projections TO authenticated;

REVOKE ALL ON FUNCTION public.request_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.accept_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.decline_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.cancel_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.recall_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.end_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.redeem_invite(text, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.update_projection(text, text, public.projection_status) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.authorize_visit_topic() FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.authorize_projection_read(uuid) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.request_visit(uuid, text) TO authenticated;
GRANT EXECUTE ON FUNCTION public.accept_visit(uuid, text) TO authenticated;
GRANT EXECUTE ON FUNCTION public.decline_visit(uuid, text) TO authenticated;
GRANT EXECUTE ON FUNCTION public.cancel_visit(uuid, text) TO authenticated;
GRANT EXECUTE ON FUNCTION public.recall_visit(uuid, text) TO authenticated;
GRANT EXECUTE ON FUNCTION public.end_visit(uuid, text) TO authenticated;
GRANT EXECUTE ON FUNCTION public.redeem_invite(text, text) TO authenticated;
GRANT EXECUTE ON FUNCTION public.update_projection(text, text, public.projection_status) TO authenticated;
GRANT EXECUTE ON FUNCTION public.authorize_visit_topic() TO authenticated;
GRANT EXECUTE ON FUNCTION public.authorize_projection_read(uuid) TO authenticated;

ALTER TABLE realtime.messages ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS pawbae_visit_broadcast_select ON realtime.messages;
CREATE POLICY pawbae_visit_broadcast_select
ON realtime.messages FOR SELECT TO authenticated
USING (
  realtime.messages.extension = 'broadcast'
  AND realtime.messages.topic = (SELECT realtime.topic())
  AND (SELECT public.authorize_visit_topic())
);

REVOKE INSERT, UPDATE, DELETE ON realtime.messages FROM PUBLIC, anon, authenticated;
REVOKE SELECT ON realtime.messages FROM PUBLIC, anon;
GRANT SELECT ON realtime.messages TO authenticated;

CREATE OR REPLACE FUNCTION private.cleanup_line_a_runtime()
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.maintain_visits();
  DELETE FROM private.idempotency_records
  WHERE created_at < clock_timestamp() - interval '48 hours';
  DELETE FROM private.rate_limits
  WHERE window_started_at < clock_timestamp() - interval '7 days';
END;
$$;

REVOKE ALL ON FUNCTION private.cleanup_line_a_runtime() FROM PUBLIC, anon, authenticated;

DO $$
DECLARE
  v_job_id bigint;
BEGIN
  FOR v_job_id IN SELECT jobid FROM cron.job WHERE jobname = 'pawbae-maintain-visits'
  LOOP
    PERFORM cron.unschedule(v_job_id);
  END LOOP;
  FOR v_job_id IN SELECT jobid FROM cron.job WHERE jobname = 'pawbae-cleanup-runtime'
  LOOP
    PERFORM cron.unschedule(v_job_id);
  END LOOP;
END;
$$;

SELECT cron.schedule(
  'pawbae-maintain-visits',
  '* * * * *',
  'SELECT private.maintain_visits()'
);

SELECT cron.schedule(
  'pawbae-cleanup-runtime',
  '17 4 * * *',
  'SELECT private.cleanup_line_a_runtime()'
);

COMMIT;
