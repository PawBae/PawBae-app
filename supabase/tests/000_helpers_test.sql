begin;
select * from no_plan();

select has_schema('private', 'private helper schema exists');
select has_table('private', 'rate_limits', 'fixed-window rate-limit state is private');
select ok(
  exists (
    select 1 from pg_policies
    where schemaname = 'private'
      and tablename = 'rate_limits'
      and policyname = 'rate_limits_no_client_access'
  ),
  'private rate-limit state has an explicit deny-all client policy'
);

select has_function(
  'private',
  'valid_private_snapshot',
  array['jsonb'],
  'private snapshot validator exists'
);
select has_function(
  'private',
  'valid_event_payload',
  array['text', 'jsonb'],
  'event dictionary validator exists'
);
select ok(
  private.valid_https_url('https://avatars.githubusercontent.com/u/101'),
  'HTTPS validator accepts a canonical provider avatar URL'
);
select ok(
  not private.valid_https_url('https://%'),
  'HTTPS validator rejects a malformed host'
);
select ok(
  not private.valid_https_url('https:///local/path'),
  'HTTPS validator rejects a missing host'
);
select has_function(
  'private',
  'consume_rate_limit',
  array['uuid', 'text', 'text', 'integer', 'interval'],
  'fixed-window rate-limit helper exists'
);
select has_function(
  'private',
  'is_blocked',
  array['uuid', 'uuid'],
  'bidirectional block lookup exists'
);

select ok(
  not has_schema_privilege('anon', 'private', 'usage'),
  'anon cannot use the private schema'
);
select ok(
  not has_schema_privilege('authenticated', 'private', 'usage'),
  'authenticated cannot use the private schema'
);
select ok(
  not has_function_privilege(
    'authenticated',
    'private.consume_rate_limit(uuid,text,text,integer,interval)',
    'execute'
  ),
  'authenticated cannot call the rate-limit helper directly'
);

select ok(
  private.valid_private_snapshot(
    '{"petId":"yoonie","spriteState":"idle","mood":"neutral","hunger":100,"level":1,"streak":0,"away":false}'::jsonb
  ),
  'canonical private snapshot is accepted'
);
select ok(
  private.valid_private_snapshot(
    '{"petId":"skin.one-2","spriteState":"compacting","mood":"focused","hunger":0,"level":100,"streak":3650,"away":true}'::jsonb
  ),
  'snapshot boundary values and approved identifiers are accepted'
);
select ok(
  not private.valid_private_snapshot(
    '{"petId":"yoonie","spriteState":"idle","mood":"neutral","hunger":100,"level":1,"streak":0,"away":false,"note":"secret"}'::jsonb
  ),
  'snapshot rejects extra free-text keys'
);
select ok(
  not private.valid_private_snapshot(
    '{"petId":"yoonie","spriteState":"idle","mood":"neutral","hunger":100,"level":1,"away":false}'::jsonb
  ),
  'snapshot rejects missing keys'
);
select ok(
  not private.valid_private_snapshot(
    '{"petId":"Yoonie","spriteState":"idle","mood":"neutral","hunger":100,"level":1,"streak":0,"away":false}'::jsonb
  ),
  'snapshot rejects non-canonical pet identifiers'
);
select ok(
  not private.valid_private_snapshot(
    '{"petId":"yoonie","spriteState":"unknown","mood":"neutral","hunger":100,"level":1,"streak":0,"away":false}'::jsonb
  ),
  'snapshot rejects unknown sprite states'
);
select ok(
  not private.valid_private_snapshot(
    '{"petId":"yoonie","spriteState":"idle","mood":"angry","hunger":100,"level":1,"streak":0,"away":false}'::jsonb
  ),
  'snapshot rejects unknown moods'
);
select ok(
  not private.valid_private_snapshot(
    '{"petId":"yoonie","spriteState":"idle","mood":"neutral","hunger":100.5,"level":1,"streak":0,"away":false}'::jsonb
  ),
  'snapshot rejects non-integral numbers'
);
select ok(
  not private.valid_private_snapshot(
    '{"petId":"yoonie","spriteState":"idle","mood":"neutral","hunger":101,"level":1,"streak":0,"away":false}'::jsonb
  ),
  'snapshot rejects out-of-range numbers'
);

select ok(
  private.valid_event_payload('task_completed', '{"source":"cc"}'::jsonb),
  'task_completed accepts cc'
);
select ok(
  private.valid_event_payload('task_completed', '{"source":"codex"}'::jsonb),
  'task_completed accepts codex'
);
select ok(
  private.valid_event_payload('task_completed', '{"source":"cursor"}'::jsonb),
  'task_completed accepts cursor'
);
select ok(
  private.valid_event_payload('egg_hatched', '{"rarity":"common"}'::jsonb),
  'egg_hatched accepts an approved rarity'
);
select ok(
  private.valid_event_payload('souvenir_found', '{"rarity":"legendary"}'::jsonb),
  'souvenir_found accepts an approved rarity'
);
select ok(
  private.valid_event_payload('streak_milestone', '{"days":3650}'::jsonb),
  'streak_milestone accepts the upper boundary'
);
select ok(
  not private.valid_event_payload('unknown', '{}'::jsonb),
  'unknown event kinds are rejected'
);
select ok(
  not private.valid_event_payload('task_completed', '{"source":"other"}'::jsonb),
  'unknown event enums are rejected'
);
select ok(
  not private.valid_event_payload('egg_hatched', '{"rarity":"epic"}'::jsonb),
  'unapproved rarity enums are rejected'
);
select ok(
  not private.valid_event_payload('task_completed', '{"source":"cc","note":"secret"}'::jsonb),
  'event payloads reject free-text and extra keys'
);
select ok(
  not private.valid_event_payload('streak_milestone', '{"days":0}'::jsonb),
  'event payloads reject low out-of-range numbers'
);
select ok(
  not private.valid_event_payload('streak_milestone', '{"days":3651}'::jsonb),
  'event payloads reject high out-of-range numbers'
);
select ok(
  not private.valid_event_payload('streak_milestone', '{"days":1.5}'::jsonb),
  'event payloads reject non-integral numbers'
);

delete from private.rate_limits
where actor_id = '00000000-0000-0000-0000-00000000f001'::uuid
  and action = 'test_action';

select lives_ok(
  $$select private.consume_rate_limit(
    '00000000-0000-0000-0000-00000000f001'::uuid,
    'test_action',
    'subject',
    2,
    interval '1 minute'
  )$$,
  'first request is inside the fixed window limit'
);
select lives_ok(
  $$select private.consume_rate_limit(
    '00000000-0000-0000-0000-00000000f001'::uuid,
    'test_action',
    'subject',
    2,
    interval '1 minute'
  )$$,
  'second request is inside the fixed window limit'
);
select throws_ok(
  $$select private.consume_rate_limit(
    '00000000-0000-0000-0000-00000000f001'::uuid,
    'test_action',
    'subject',
    2,
    interval '1 minute'
  )$$,
  'P0001',
  'rate_limit_exceeded',
  'the fixed-window helper rejects requests over the limit'
);

select * from finish();
rollback;
