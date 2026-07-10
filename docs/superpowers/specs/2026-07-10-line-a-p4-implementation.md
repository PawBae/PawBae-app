# Line A P4 implementation specification

Date: 2026-07-10
Owner: Line A (`supabase/`, `packages/shared/`)
Product source: [Asynchronous visits and shared memories](2026-07-09-social-visiting-design.md)

This document freezes the database and RPC contract implemented by the Line A visit,
projection, invite, memory, Realtime, and funnel migrations. Database columns use
`snake_case`; clients map canonical rows to the camel-case `@pawbae/shared` types.

## 1. Security boundary

- `public` is the only Data API schema. Every public table has RLS enabled and an
  explicit least-privilege grant. Client roles never receive table write grants for
  visits, projections, invites, or memories.
- `private` has no `USAGE` grant for `PUBLIC`, `anon`, `authenticated`, or
  `service_role`. Its tables and privileged functions are not Data API surfaces.
- A public write RPC is a deliberately narrow `SECURITY DEFINER` wrapper with
  `search_path = ''`. Its private implementation derives the actor from `auth.uid()`,
  validates every input and transition, consumes a database rate limit in the same
  transaction, and returns the affected canonical row.
- Every function explicitly revokes the default `PUBLIC` execution grant. Only the
  documented public wrappers are granted to `authenticated`.
- Two policy-only boolean wrappers, `authorize_visit_topic()` and
  `authorize_projection_read(owner_user_id)`, exist because RLS is evaluated as the
  client role while that role intentionally has no `private` schema usage. They return
  no row data, derive the actor/topic from request context, and are not application RPC
  contracts.
- No authorization decision uses user-editable JWT metadata. User identity is always
  `auth.uid()` and relationship state is re-read from database rows.

## 2. Visit schema and state machine

`public.visits` stores one visitor, one host, the request window, and an optional
accepted lease. Requests expire exactly 24 hours after `requested_at`; acceptance sets
`started_at` and `ends_at = started_at + 30 minutes` using the database clock.

Canonical statuses:

```text
requested -> accepted -> traveling -> visiting -> returning -> completed
    |            |           |           |            |
    +-> declined +-----------+-----------+------------+-> blocked
    +-> cancelled            +-> completed / recalled
    +-> expired
```

- `accept_visit` leaves the durable row in `accepted`. Minute maintenance advances
  accepted/traveling animation phases deterministically; clients still use lease
  timestamps as the correctness source and must not wait for cron.
- `recall_visit` changes an active visit to `returning` with terminal target
  `recalled`. `end_visit` does the same with target `completed`. Maintenance settles a
  returning row after the farewell beat. Entering `returning` revokes projection reads,
  topic authorization, and publishing immediately; it remains unfinished only for
  uniqueness and crash recovery.
- Natural lease expiry settles accepted/traveling/visiting directly to `completed`.
  A pending request whose 24-hour deadline passes becomes `expired`.
- An insert into `blocks` changes every unfinished visit for that pair to `blocked` in
  the same transaction. Deleting their friendship cancels pending requests and recalls
  active visits.
- Terminal rows never transition again. A trigger rejects invalid transitions and
  forged request/lease timestamps even for privileged writers.

Partial unique indexes enforce:

- one visitor row across `requested|accepted|traveling|visiting|returning`;
- one host row across `accepted|traveling|visiting|returning`.

Multiple people may request the same host while requests are pending. Acceptance locks
the host and rejects the second concurrent acceptance with `host already has an active
visit`.

## 3. Visit and invite RPCs

RPC argument names are the PostgREST names shown here:

| RPC | Arguments | Actor and transition | Return |
|---|---|---|---|
| `request_visit` | `p_host_user_id uuid, p_idempotency_key text` | authenticated visitor; accepted friend; unblocked | `visits` |
| `accept_visit` | `p_visit_id uuid, p_idempotency_key text` | host; requested and unexpired | `visits` |
| `decline_visit` | same | host; requested | `visits` |
| `cancel_visit` | same | visitor; requested | `visits` |
| `recall_visit` | same | visitor; accepted/traveling/visiting | `visits` |
| `end_visit` | same | either participant; accepted/traveling/visiting | `visits` |
| `redeem_invite` | `p_code text, p_idempotency_key text` | authenticated user; valid unused capacity | `invite_redemptions` |

Idempotency keys are 8-128 characters from `[A-Za-z0-9._:-]`, start with an
alphanumeric character, and are unique per actor across all operations. The first call
stores the complete canonical result for 48 hours. A retry returns that original result,
even if the resource later changes. Reusing a key for a different operation is rejected.

`invite_codes` stores only a SHA-256 code hash. Codes have an expiry, optional
revocation time, and bounded use count (1-10). An account can redeem at most one code;
retry never consumes another use. Codes are provisioned by an operator/migration—there
is intentionally no client minting RPC.

Fixed-window defaults used by this slice:

- visit request: 30/hour/user;
- other visit mutations: 30/hour/user;
- invite and memory mutations: 60/hour/user;
- projection update: 120/minute/user.

These transactional counters are committed-mutation quotas: a replay that returns an
existing canonical result does not consume again, and any exception rolls the whole
transaction—including its counter increment—back. Invalid-request flood protection is
therefore a separate API/gateway throttle concern; the database limits do not claim to
be durable failed-attempt counters.

## 4. Public projection and Realtime

`public.pet_projections` is keyed by `owner_user_id` and contains only:

```text
owner_user_id, pet_id, version=1, display_name, skin_id,
status, updated_at
```

`pet_id` and `skin_id` match `^[a-z0-9][a-z0-9._-]{0,63}$`. `display_name` is
derived server-side from `profiles.display_name ?? profiles.handle`. Status is exactly
`idle|working|waiting|compacting|offline`.

Approved v1 built-ins are `doro.codex-pet`, `elaina-2`, `homie`, `linnea-2`,
`mambo`, `naruto`, `nezuko`, `phoebe.codex-pet`, `shimeji-bola`, `skirk-2`,
`taffy`, `wukong`, and `yoonie`. Adding a skin is a reviewed migration. Local custom
skins never enter the public projection automatically.

`update_projection(p_pet_id text, p_skin_id text, p_status projection_status)` is the
only client write path. It upserts and returns the owner's projection. The owner may
read it; a host may read it only while an unexpired, accepted-friend, unblocked visit is
active.

The private Broadcast topic is:

```text
pet:{visitor_user_id}:{visit_id}
```

- Authenticated clients have policy-filtered `SELECT`, but never `INSERT`, `UPDATE`, or
  `DELETE`, on `realtime.messages`; anonymous clients have no table access.
- A private-channel RLS policy authorizes only the current host for an active unexpired
  lease, requires a Broadcast authorization probe whose `topic` exactly equals
  `realtime.topic()`, and rechecks friendship and both block directions. Realtime's
  rolled-back authorization probe has `private = false` even for a private channel, so
  the policy intentionally does not inspect that column; the client `private: true`
  setting and private database send must still match before Realtime delivers a frame.
- Projection inserts/updates publish through `realtime.send` in a database trigger.
  The update path first takes the pair's canonical advisory lock, then locks the visit
  row and rechecks status, `ends_at`, friendship, and blocks while both locks are held.
  Recall, end, unfriend, and block use the same pair-before-visit order, so publication
  and revocation are linearized. The trigger repeats the locked check before every send.
  Cached channel authorization therefore cannot leak post-revocation updates.
- Current Realtime injects a UUID `id` into each `realtime.send` payload as transport
  metadata. Clients validate and remove it before passing the remaining exact six keys
  to the shared projection sanitizer.
- Leaving an active state emits a best-effort `visit_ended` event if friendship and
  block state still permit it. Natural expiry reports the canonical lease `ends_at` as
  `endedAt`; lease timestamps remain the correctness mechanism.
- Production Realtime must disable **Allow public access**, and Auth JWT expiry is ten
  minutes. Those hosted settings are audited separately because they are not DDL.

## 5. Shared memories

`public.shared_memories` is client-append-only, participant-readable, and unique on
`visit_id`. Participants are copied from the locked visit row and cannot be supplied by
the caller. Only a started visit in `completed` or `recalled` may settle a memory.
Clients have no update/delete grant; trusted foreign-key cascades remain available for
account erasure.

`settle_shared_memory(p_visit_id uuid, p_idempotency_key text)` derives all content
server-side and returns the canonical memory. The initial deterministic template is
`played_together`; the enum reserves `worked_together`, `celebrated_completion`, and
`shared_snack` for future server-observed facts.

The exact parameter object is:

```json
{
  "durationBucket": "short | full",
  "timeOfDay": "morning | afternoon | evening | night",
  "interactionCount": 0
}
```

`full` means at least 25 elapsed minutes. Time of day derives from the UTC start hour.
The validator requires exactly those three keys and bounds `interactionCount` to an
integer from 0 through 100. There is no rendered prose or free-text slot.

`record_memory_view(p_memory_id uuid, p_idempotency_key text)` is a minimal analytics
write required to measure the documented completion-to-memory-view funnel. It verifies
the caller is a participant, records an append-only deduplicated fact, and returns the
unchanged canonical memory.

## 6. Funnel views

Server triggers record a private, fixed-enum funnel fact when a friendship request is
sent/accepted, a visit is requested/completed, a memory is created, or a participant
views a memory. Deletes of canonical social rows do not erase those aggregate facts.

Five `security_invoker` views implement the beta steps:

1. `funnel_friend_request_acceptance`;
2. `funnel_friend_to_first_visit`;
3. `funnel_visit_completion`;
4. `funnel_memory_view`;
5. `funnel_seven_day_repeat_visit`.

They have no `anon` or `authenticated` grant and are intended for the Supabase SQL
dashboard/operator role, not the product Data API.

## 7. Maintenance and tests

`pg_cron` calls `private.maintain_visits()` every minute. A daily job also runs visit
maintenance, deletes idempotency results older than 48 hours, and removes old fixed
rate windows. Both functions are idempotent. Mutation RPCs do not call global
maintenance while holding a social pair lock (that would create a cross-pair deadlock
risk); every authorization path checks request/lease timestamps synchronously, so cron
lateness can delay a new conflicting lease by at most one maintenance interval but can
never extend read or publish authorization.

pgTAP coverage is split into:

- `003_visits_realtime_test.sql`: timing, state/RPC behavior, idempotency, host
  uniqueness across competing requests, rates, invites, projection authorization,
  exact persisted Broadcast topics/payloads, cross-topic isolation, and revoked-state
  silence;
- `004_memories_metrics_test.sql`: safe derived params, canonical single settlement,
  replay, forged-row rejection, rate limits, memory-view instrumentation, and asserted
  security-invoker funnel outputs;
- `005_security_matrix_test.sql`: participant/stranger/blocked access, forged host and
  lease rejection, direct-write denial, block-time Broadcast revocation, Realtime table
  ACLs, and private-schema ACLs.

`api-e2e.mjs` issues concurrent Data API mutations over separate requests to exercise
duplicate replay, host contention, and request-versus-block ordering.
`realtime-e2e.mjs` uses a real private WebSocket channel to prove host join, stranger
denial, projection delivery, the final `visit_ended` frame, immediate silence on the
still-open cached channel after recall, and denial when that host tries to rejoin.
The pair-before-visit lock order is also asserted structurally and exercised under
normal concurrency, but the suite does not claim a deterministic scheduler-controlled
projection-versus-recall/unfriend/block interleaving proof.
