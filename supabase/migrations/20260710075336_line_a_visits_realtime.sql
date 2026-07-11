BEGIN;

CREATE EXTENSION IF NOT EXISTS pg_cron WITH SCHEMA pg_catalog;

CREATE TYPE public.visit_status AS ENUM (
  'requested',
  'accepted',
  'traveling',
  'visiting',
  'returning',
  'completed',
  'declined',
  'cancelled',
  'expired',
  'recalled',
  'blocked'
);

CREATE TABLE public.visits (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  visitor_user_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  host_user_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  status public.visit_status NOT NULL DEFAULT 'requested',
  requested_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  request_expires_at timestamptz NOT NULL DEFAULT (clock_timestamp() + interval '24 hours'),
  started_at timestamptz,
  ends_at timestamptz,
  returning_started_at timestamptz,
  terminal_status public.visit_status,
  ended_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT visits_distinct_participants CHECK (visitor_user_id <> host_user_id),
  CONSTRAINT visits_terminal_target CHECK (
    terminal_status IS NULL OR terminal_status IN ('completed', 'recalled')
  )
);

CREATE UNIQUE INDEX visits_one_unfinished_visitor_idx
ON public.visits (visitor_user_id)
WHERE status IN ('requested', 'accepted', 'traveling', 'visiting', 'returning');

CREATE UNIQUE INDEX visits_one_busy_host_idx
ON public.visits (host_user_id)
WHERE status IN ('accepted', 'traveling', 'visiting', 'returning');

CREATE INDEX visits_host_status_lease_idx
ON public.visits (host_user_id, status, ends_at);

CREATE INDEX visits_visitor_status_lease_idx
ON public.visits (visitor_user_id, status, ends_at);

CREATE INDEX visits_request_expiry_idx
ON public.visits (request_expires_at)
WHERE status = 'requested';

CREATE INDEX visits_active_lease_maintenance_idx
ON public.visits (ends_at)
WHERE status IN ('accepted', 'traveling', 'visiting');

CREATE INDEX visits_returning_maintenance_idx
ON public.visits (returning_started_at)
WHERE status = 'returning';

CREATE INDEX visits_stage_maintenance_idx
ON public.visits (started_at, status)
WHERE status IN ('accepted', 'traveling');

CREATE TABLE private.idempotency_records (
  actor_id uuid NOT NULL REFERENCES public.profiles(id) ON DELETE CASCADE,
  idempotency_key text NOT NULL,
  operation text NOT NULL,
  result jsonb,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (actor_id, idempotency_key),
  CONSTRAINT idempotency_key_shape CHECK (
    idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{7,127}$'
  ),
  CONSTRAINT idempotency_operation_shape CHECK (
    operation ~ '^[a-z][a-z0-9_]{2,63}$'
  )
);

CREATE INDEX idempotency_records_created_at_idx
ON private.idempotency_records (created_at);

ALTER TABLE private.idempotency_records ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON private.idempotency_records FROM PUBLIC, anon, authenticated;
CREATE POLICY idempotency_records_no_client_access
ON private.idempotency_records AS RESTRICTIVE FOR ALL TO anon, authenticated
USING (false) WITH CHECK (false);

CREATE TABLE public.invite_codes (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  code_hash bytea NOT NULL UNIQUE,
  issued_by uuid REFERENCES public.profiles(id) ON DELETE SET NULL,
  max_uses smallint NOT NULL DEFAULT 1 CHECK (max_uses BETWEEN 1 AND 10),
  use_count smallint NOT NULL DEFAULT 0 CHECK (use_count >= 0 AND use_count <= max_uses),
  expires_at timestamptz NOT NULL,
  revoked_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX invite_codes_redeemable_idx
ON public.invite_codes (expires_at)
WHERE revoked_at IS NULL;

CREATE INDEX invite_codes_issued_by_idx
ON public.invite_codes (issued_by)
WHERE issued_by IS NOT NULL;

CREATE TABLE public.invite_redemptions (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  invite_code_id uuid NOT NULL REFERENCES public.invite_codes(id) ON DELETE RESTRICT,
  user_id uuid NOT NULL UNIQUE REFERENCES public.profiles(id) ON DELETE CASCADE,
  redeemed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  UNIQUE (invite_code_id, user_id)
);

CREATE INDEX invite_redemptions_code_idx
ON public.invite_redemptions (invite_code_id);

CREATE OR REPLACE FUNCTION private.validate_visit_row()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_request_seconds numeric;
  v_lease_seconds numeric;
BEGIN
  IF NEW.visitor_user_id = NEW.host_user_id THEN
    RAISE EXCEPTION 'visitor and host must differ';
  END IF;

  IF TG_OP = 'INSERT' THEN
    IF NEW.status <> 'requested' THEN
      RAISE EXCEPTION 'new visits must begin as requested';
    END IF;
    IF abs(extract(epoch FROM (NEW.requested_at - clock_timestamp()))) > 2 THEN
      RAISE EXCEPTION 'new visit request must use the database clock';
    END IF;
    IF NEW.started_at IS NOT NULL
       OR NEW.ends_at IS NOT NULL
       OR NEW.returning_started_at IS NOT NULL
       OR NEW.terminal_status IS NOT NULL
       OR NEW.ended_at IS NOT NULL THEN
      RAISE EXCEPTION 'new visit request cannot contain lifecycle timestamps';
    END IF;
  END IF;

  IF TG_OP = 'UPDATE' THEN
    IF NEW.visitor_user_id IS DISTINCT FROM OLD.visitor_user_id
       OR NEW.host_user_id IS DISTINCT FROM OLD.host_user_id THEN
      RAISE EXCEPTION 'visit participants are immutable';
    END IF;
    IF NEW.requested_at IS DISTINCT FROM OLD.requested_at
       OR NEW.request_expires_at IS DISTINCT FROM OLD.request_expires_at THEN
      RAISE EXCEPTION 'visit request timestamps are immutable';
    END IF;
    IF OLD.status = 'requested' AND NEW.status = 'accepted' THEN
      IF NEW.started_at IS NULL
         OR abs(extract(epoch FROM (NEW.started_at - clock_timestamp()))) > 1 THEN
        RAISE EXCEPTION 'accepted visit must start at the database clock';
      END IF;
    ELSIF NEW.started_at IS DISTINCT FROM OLD.started_at
          OR NEW.ends_at IS DISTINCT FROM OLD.ends_at THEN
      RAISE EXCEPTION 'visit lease timestamps are immutable after acceptance';
    END IF;
    IF NEW.ended_at IS DISTINCT FROM OLD.ended_at THEN
      RAISE EXCEPTION 'visit ended_at is server managed';
    END IF;
    IF OLD.status IN ('accepted', 'traveling', 'visiting') AND NEW.status = 'returning' THEN
      IF NEW.returning_started_at IS NULL
         OR abs(extract(epoch FROM (NEW.returning_started_at - clock_timestamp()))) > 1 THEN
        RAISE EXCEPTION 'returning visit must start at the database clock';
      END IF;
    ELSIF NEW.returning_started_at IS DISTINCT FROM OLD.returning_started_at THEN
      RAISE EXCEPTION 'returning timestamp is server managed';
    END IF;
  END IF;

  v_request_seconds := extract(epoch FROM (NEW.request_expires_at - NEW.requested_at));
  IF abs(v_request_seconds - 86400) > 1 THEN
    RAISE EXCEPTION 'visit request expiry must be 24 hours';
  END IF;

  IF NEW.status = 'requested' AND (NEW.started_at IS NOT NULL OR NEW.ends_at IS NOT NULL) THEN
    RAISE EXCEPTION 'requested visits cannot have lease timestamps';
  END IF;

  IF NEW.status IN ('accepted', 'traveling', 'visiting', 'returning') THEN
    IF NEW.started_at IS NULL OR NEW.ends_at IS NULL THEN
      RAISE EXCEPTION 'active visits require lease timestamps';
    END IF;
  END IF;

  IF NEW.started_at IS NOT NULL OR NEW.ends_at IS NOT NULL THEN
    IF NEW.started_at IS NULL OR NEW.ends_at IS NULL THEN
      RAISE EXCEPTION 'lease timestamps must be set together';
    END IF;
    v_lease_seconds := extract(epoch FROM (NEW.ends_at - NEW.started_at));
    IF abs(v_lease_seconds - 1800) > 1 THEN
      RAISE EXCEPTION 'accepted visit leases must be 30 minutes';
    END IF;
  END IF;

  IF NEW.status = 'returning' THEN
    NEW.returning_started_at := COALESCE(NEW.returning_started_at, clock_timestamp());
    IF NEW.terminal_status NOT IN ('completed', 'recalled') THEN
      RAISE EXCEPTION 'returning visits require a completed or recalled target';
    END IF;
  ELSIF NEW.status IN ('completed', 'declined', 'cancelled', 'expired', 'recalled', 'blocked') THEN
    NEW.returning_started_at := NULL;
    NEW.terminal_status := NULL;
  ELSIF NEW.terminal_status IS NOT NULL THEN
    RAISE EXCEPTION 'terminal target is only valid while returning';
  END IF;

  IF TG_OP = 'UPDATE' AND NEW.status <> OLD.status THEN
    IF NOT (
      CASE OLD.status
        WHEN 'requested' THEN NEW.status IN ('accepted', 'declined', 'cancelled', 'expired', 'blocked')
        WHEN 'accepted' THEN NEW.status IN ('traveling', 'returning', 'completed', 'recalled', 'blocked')
        WHEN 'traveling' THEN NEW.status IN ('visiting', 'returning', 'completed', 'recalled', 'blocked')
        WHEN 'visiting' THEN NEW.status IN ('returning', 'completed', 'recalled', 'blocked')
        WHEN 'returning' THEN NEW.status IN ('completed', 'recalled', 'blocked')
        ELSE false
      END
    ) THEN
      RAISE EXCEPTION 'invalid visit transition: % -> %', OLD.status, NEW.status;
    END IF;
  END IF;

  IF NEW.status IN ('completed', 'declined', 'cancelled', 'expired', 'recalled', 'blocked') THEN
    NEW.ended_at := COALESCE(NEW.ended_at, clock_timestamp());
  ELSIF NEW.ended_at IS NOT NULL THEN
    RAISE EXCEPTION 'unfinished visits cannot have ended_at';
  END IF;

  NEW.updated_at := clock_timestamp();
  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.validate_visit_row() FROM PUBLIC, anon, authenticated;

CREATE TRIGGER visits_validate_row
BEFORE INSERT OR UPDATE ON public.visits
FOR EACH ROW EXECUTE FUNCTION private.validate_visit_row();

CREATE OR REPLACE FUNCTION private.claim_idempotency(
  p_actor uuid,
  p_key text,
  p_operation text
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_claimed boolean;
  v_existing private.idempotency_records%ROWTYPE;
BEGIN
  IF p_actor IS NULL OR p_actor <> (SELECT auth.uid()) THEN
    RAISE EXCEPTION 'authenticated actor required';
  END IF;
  IF p_key IS NULL OR p_key !~ '^[A-Za-z0-9][A-Za-z0-9._:-]{7,127}$' THEN
    RAISE EXCEPTION 'invalid idempotency key';
  END IF;
  IF p_operation IS NULL OR p_operation !~ '^[a-z][a-z0-9_]{2,63}$' THEN
    RAISE EXCEPTION 'invalid idempotency operation';
  END IF;

  INSERT INTO private.idempotency_records (actor_id, idempotency_key, operation)
  VALUES (p_actor, p_key, p_operation)
  ON CONFLICT (actor_id, idempotency_key) DO NOTHING
  RETURNING true INTO v_claimed;

  IF COALESCE(v_claimed, false) THEN
    RETURN NULL;
  END IF;

  SELECT * INTO v_existing
  FROM private.idempotency_records
  WHERE actor_id = p_actor AND idempotency_key = p_key
  FOR UPDATE;

  IF v_existing.operation <> p_operation THEN
    RAISE EXCEPTION 'idempotency key already used for another operation';
  END IF;
  IF v_existing.result IS NULL THEN
    RAISE EXCEPTION 'idempotency result is not available';
  END IF;
  RETURN v_existing.result;
END;
$$;

CREATE OR REPLACE FUNCTION private.finish_idempotency(
  p_actor uuid,
  p_key text,
  p_operation text,
  p_result jsonb
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  UPDATE private.idempotency_records
  SET result = p_result
  WHERE actor_id = p_actor
    AND idempotency_key = p_key
    AND operation = p_operation;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'idempotency claim not found';
  END IF;
END;
$$;

REVOKE ALL ON FUNCTION private.claim_idempotency(uuid, text, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.finish_idempotency(uuid, text, text, jsonb) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.are_accepted_friends(p_first uuid, p_second uuid)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM public.friendships AS f
    WHERE f.user_a = LEAST(p_first, p_second)
      AND f.user_b = GREATEST(p_first, p_second)
      AND f.status::text = 'accepted'
  );
$$;

REVOKE ALL ON FUNCTION private.are_accepted_friends(uuid, uuid) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.maintain_visits()
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
BEGIN
  UPDATE public.visits
  SET status = 'expired'
  WHERE status = 'requested'
    AND request_expires_at <= v_now;

  UPDATE public.visits
  SET status = 'completed'
  WHERE status IN ('accepted', 'traveling', 'visiting')
    AND ends_at <= v_now;

  UPDATE public.visits
  SET status = terminal_status
  WHERE status = 'returning'
    AND returning_started_at <= v_now - interval '15 seconds';

  UPDATE public.visits
  SET status = 'traveling'
  WHERE status = 'accepted'
    AND started_at <= v_now - interval '5 seconds';

  UPDATE public.visits
  SET status = 'visiting'
  WHERE status = 'traveling'
    AND started_at <= v_now - interval '15 seconds';
END;
$$;

REVOKE ALL ON FUNCTION private.maintain_visits() FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.request_visit_impl(p_host_user_id uuid, p_idempotency_key text)
RETURNS public.visits
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor uuid := (SELECT auth.uid());
  v_replay jsonb;
  v_existing public.visits%ROWTYPE;
  v_result public.visits%ROWTYPE;
  v_now timestamptz := clock_timestamp();
BEGIN
  IF v_actor IS NULL THEN
    RAISE EXCEPTION 'authenticated actor required';
  END IF;
  IF p_host_user_id IS NULL OR p_host_user_id = v_actor THEN
    RAISE EXCEPTION 'a visit requires another user';
  END IF;

  v_replay := private.claim_idempotency(v_actor, p_idempotency_key, 'request_visit');
  IF v_replay IS NOT NULL THEN
    SELECT * INTO v_result FROM jsonb_populate_record(NULL::public.visits, v_replay);
    RETURN v_result;
  END IF;

  PERFORM private.consume_rate_limit(
    v_actor, 'visit_request', 'all', 30, interval '1 hour'
  );
  PERFORM private.lock_social_pair(v_actor, p_host_user_id);

  PERFORM 1
  FROM public.profiles
  WHERE id IN (v_actor, p_host_user_id)
  ORDER BY id
  FOR UPDATE;

  IF NOT EXISTS (SELECT 1 FROM public.profiles WHERE id = p_host_user_id) THEN
    RAISE EXCEPTION 'visit host not found';
  END IF;
  IF NOT private.are_accepted_friends(v_actor, p_host_user_id) THEN
    RAISE EXCEPTION 'visits require an accepted friendship';
  END IF;
  IF private.is_blocked(v_actor, p_host_user_id) THEN
    RAISE EXCEPTION 'visit is blocked';
  END IF;

  SELECT * INTO v_existing
  FROM public.visits
  WHERE visitor_user_id = v_actor
    AND status IN ('requested', 'accepted', 'traveling', 'visiting', 'returning')
  FOR UPDATE;

  IF FOUND THEN
    IF v_existing.host_user_id = p_host_user_id AND v_existing.status = 'requested' THEN
      PERFORM private.finish_idempotency(
        v_actor, p_idempotency_key, 'request_visit', to_jsonb(v_existing)
      );
      RETURN v_existing;
    END IF;
    RAISE EXCEPTION 'visitor already has an unfinished visit';
  END IF;

  v_now := clock_timestamp();
  INSERT INTO public.visits (
    visitor_user_id, host_user_id, status, requested_at, request_expires_at
  )
  VALUES (
    v_actor, p_host_user_id, 'requested', v_now, v_now + interval '24 hours'
  )
  RETURNING * INTO v_result;

  PERFORM private.finish_idempotency(
    v_actor, p_idempotency_key, 'request_visit', to_jsonb(v_result)
  );
  RETURN v_result;
END;
$$;

CREATE OR REPLACE FUNCTION private.transition_visit_impl(
  p_visit_id uuid,
  p_idempotency_key text,
  p_action text
)
RETURNS public.visits
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor uuid := (SELECT auth.uid());
  v_operation text := p_action || '_visit';
  v_replay jsonb;
  v_visit public.visits%ROWTYPE;
  v_result public.visits%ROWTYPE;
  v_now timestamptz := clock_timestamp();
BEGIN
  IF v_actor IS NULL THEN
    RAISE EXCEPTION 'authenticated actor required';
  END IF;
  IF p_action NOT IN ('accept', 'decline', 'cancel', 'recall', 'end') THEN
    RAISE EXCEPTION 'unknown visit action';
  END IF;

  v_replay := private.claim_idempotency(v_actor, p_idempotency_key, v_operation);
  IF v_replay IS NOT NULL THEN
    SELECT * INTO v_result FROM jsonb_populate_record(NULL::public.visits, v_replay);
    RETURN v_result;
  END IF;

  PERFORM private.consume_rate_limit(
    v_actor, 'visit_mutation', 'all', 30, interval '1 hour'
  );

  SELECT * INTO v_visit
  FROM public.visits
  WHERE id = p_visit_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'visit not found';
  END IF;
  PERFORM private.lock_social_pair(v_visit.visitor_user_id, v_visit.host_user_id);
  SELECT * INTO v_visit
  FROM public.visits
  WHERE id = p_visit_id
  FOR UPDATE;
  IF NOT private.are_accepted_friends(v_visit.visitor_user_id, v_visit.host_user_id)
     OR private.is_blocked(v_visit.visitor_user_id, v_visit.host_user_id) THEN
    RAISE EXCEPTION 'visit friendship is no longer authorized';
  END IF;
  IF v_visit.status = 'requested'
     AND v_visit.request_expires_at <= clock_timestamp() THEN
    RAISE EXCEPTION 'visit request has expired';
  END IF;
  IF v_visit.status IN ('accepted', 'traveling', 'visiting')
     AND v_visit.ends_at <= clock_timestamp() THEN
    RAISE EXCEPTION 'visit lease has expired';
  END IF;

  CASE p_action
    WHEN 'accept' THEN
      IF v_actor <> v_visit.host_user_id THEN
        RAISE EXCEPTION 'only the visit host can accept this request';
      END IF;
      IF v_visit.status <> 'requested' THEN
        RAISE EXCEPTION 'only a requested visit can be accepted';
      END IF;
      PERFORM 1 FROM public.profiles WHERE id = v_visit.host_user_id FOR UPDATE;
      IF EXISTS (
        SELECT 1 FROM public.visits
        WHERE host_user_id = v_visit.host_user_id
          AND id <> v_visit.id
          AND status IN ('accepted', 'traveling', 'visiting', 'returning')
      ) THEN
        RAISE EXCEPTION 'host already has an active visit';
      END IF;
      v_now := clock_timestamp();
      UPDATE public.visits
      SET status = 'accepted', started_at = v_now, ends_at = v_now + interval '30 minutes'
      WHERE id = v_visit.id
      RETURNING * INTO v_result;

    WHEN 'decline' THEN
      IF v_actor <> v_visit.host_user_id THEN
        RAISE EXCEPTION 'only the visit host can decline this request';
      END IF;
      IF v_visit.status <> 'requested' THEN
        RAISE EXCEPTION 'only a requested visit can be declined';
      END IF;
      UPDATE public.visits SET status = 'declined'
      WHERE id = v_visit.id RETURNING * INTO v_result;

    WHEN 'cancel' THEN
      IF v_actor <> v_visit.visitor_user_id THEN
        RAISE EXCEPTION 'only the visitor can cancel this request';
      END IF;
      IF v_visit.status <> 'requested' THEN
        RAISE EXCEPTION 'only a requested visit can be cancelled';
      END IF;
      UPDATE public.visits SET status = 'cancelled'
      WHERE id = v_visit.id RETURNING * INTO v_result;

    WHEN 'recall' THEN
      IF v_actor <> v_visit.visitor_user_id THEN
        RAISE EXCEPTION 'only the visitor can recall this visit';
      END IF;
      IF v_visit.status NOT IN ('accepted', 'traveling', 'visiting') THEN
        RAISE EXCEPTION 'only an active visit can be recalled';
      END IF;
      v_now := clock_timestamp();
      UPDATE public.visits
      SET status = 'returning', terminal_status = 'recalled', returning_started_at = v_now
      WHERE id = v_visit.id RETURNING * INTO v_result;

    WHEN 'end' THEN
      IF v_actor NOT IN (v_visit.visitor_user_id, v_visit.host_user_id) THEN
        RAISE EXCEPTION 'only a visit participant can end this visit';
      END IF;
      IF v_visit.status NOT IN ('accepted', 'traveling', 'visiting') THEN
        RAISE EXCEPTION 'only an active visit can be ended';
      END IF;
      v_now := clock_timestamp();
      UPDATE public.visits
      SET status = 'returning', terminal_status = 'completed', returning_started_at = v_now
      WHERE id = v_visit.id RETURNING * INTO v_result;
  END CASE;

  PERFORM private.finish_idempotency(
    v_actor, p_idempotency_key, v_operation, to_jsonb(v_result)
  );
  RETURN v_result;
END;
$$;

REVOKE ALL ON FUNCTION private.request_visit_impl(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.transition_visit_impl(uuid, text, text) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION public.request_visit(p_host_user_id uuid, p_idempotency_key text)
RETURNS public.visits
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.request_visit_impl(p_host_user_id, p_idempotency_key); $$;

CREATE OR REPLACE FUNCTION public.accept_visit(p_visit_id uuid, p_idempotency_key text)
RETURNS public.visits
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.transition_visit_impl(p_visit_id, p_idempotency_key, 'accept'); $$;

CREATE OR REPLACE FUNCTION public.decline_visit(p_visit_id uuid, p_idempotency_key text)
RETURNS public.visits
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.transition_visit_impl(p_visit_id, p_idempotency_key, 'decline'); $$;

CREATE OR REPLACE FUNCTION public.cancel_visit(p_visit_id uuid, p_idempotency_key text)
RETURNS public.visits
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.transition_visit_impl(p_visit_id, p_idempotency_key, 'cancel'); $$;

CREATE OR REPLACE FUNCTION public.recall_visit(p_visit_id uuid, p_idempotency_key text)
RETURNS public.visits
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.transition_visit_impl(p_visit_id, p_idempotency_key, 'recall'); $$;

CREATE OR REPLACE FUNCTION public.end_visit(p_visit_id uuid, p_idempotency_key text)
RETURNS public.visits
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.transition_visit_impl(p_visit_id, p_idempotency_key, 'end'); $$;

CREATE OR REPLACE FUNCTION private.redeem_invite_impl(p_code text, p_idempotency_key text)
RETURNS public.invite_redemptions
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor uuid := (SELECT auth.uid());
  v_replay jsonb;
  v_code public.invite_codes%ROWTYPE;
  v_result public.invite_redemptions%ROWTYPE;
BEGIN
  IF v_actor IS NULL THEN
    RAISE EXCEPTION 'authenticated actor required';
  END IF;
  IF p_code IS NULL OR btrim(p_code) !~ '^[A-Za-z0-9][A-Za-z0-9-]{7,63}$' THEN
    RAISE EXCEPTION 'invalid invite code';
  END IF;

  v_replay := private.claim_idempotency(v_actor, p_idempotency_key, 'redeem_invite');
  IF v_replay IS NOT NULL THEN
    SELECT * INTO v_result FROM jsonb_populate_record(NULL::public.invite_redemptions, v_replay);
    RETURN v_result;
  END IF;

  PERFORM private.consume_rate_limit(
    v_actor, 'social_mutation', 'all', 60, interval '1 hour'
  );

  SELECT * INTO v_result
  FROM public.invite_redemptions
  WHERE user_id = v_actor
  FOR UPDATE;

  IF FOUND THEN
    PERFORM private.finish_idempotency(
      v_actor, p_idempotency_key, 'redeem_invite', to_jsonb(v_result)
    );
    RETURN v_result;
  END IF;

  SELECT * INTO v_code
  FROM public.invite_codes
  WHERE code_hash = extensions.digest(upper(btrim(p_code)), 'sha256')
  FOR UPDATE;

  IF NOT FOUND OR v_code.revoked_at IS NOT NULL
     OR v_code.expires_at <= clock_timestamp()
     OR v_code.use_count >= v_code.max_uses THEN
    RAISE EXCEPTION 'invite code is invalid, expired, revoked, or exhausted';
  END IF;

  INSERT INTO public.invite_redemptions (invite_code_id, user_id)
  VALUES (v_code.id, v_actor)
  RETURNING * INTO v_result;

  UPDATE public.invite_codes
  SET use_count = use_count + 1
  WHERE id = v_code.id;

  PERFORM private.finish_idempotency(
    v_actor, p_idempotency_key, 'redeem_invite', to_jsonb(v_result)
  );
  RETURN v_result;
END;
$$;

REVOKE ALL ON FUNCTION private.redeem_invite_impl(text, text) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION public.redeem_invite(p_code text, p_idempotency_key text)
RETURNS public.invite_redemptions
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$ SELECT private.redeem_invite_impl(p_code, p_idempotency_key); $$;

-- Deny access during ordered deployment before the projection/Realtime policy
-- migration grants each narrow interface.
ALTER TABLE public.visits ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.invite_codes ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.invite_redemptions ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON public.visits FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.invite_codes FROM PUBLIC, anon, authenticated;
REVOKE ALL ON public.invite_redemptions FROM PUBLIC, anon, authenticated;

REVOKE ALL ON FUNCTION public.request_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.accept_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.decline_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.cancel_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.recall_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.end_visit(uuid, text) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.redeem_invite(text, text) FROM PUBLIC, anon, authenticated;

COMMIT;
