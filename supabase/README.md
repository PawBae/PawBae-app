# PawBae Line A backend

This directory owns PawBae's Supabase schema, database tests, and local backend
configuration. The production project is the existing project
`etqrnvhxvptnyhcdxgtb`. Do not create or link a replacement project.

## Client environment

Clients use exactly these canonical variables:

```dotenv
SUPABASE_URL=
SUPABASE_PUBLISHABLE_KEY=
```

Copy `.env.example` to an ignored local environment file and fill it with the
project URL and modern publishable key. Never put a secret key, service-role key,
database password, OAuth client secret, or access token in a client environment
file, migration, test fixture, or Ralph state file. Add legacy-named aliases only
in a consumer branch that still requires them.

## Local verification

Docker Desktop and the pinned Supabase CLI version from `cloud.yml` are required.
Run commands from the repository root:

```sh
supabase start
supabase db reset
supabase test db
supabase db lint --local --schema public,private --level warning --fail-on warning
supabase db advisors --local --type security --level info --fail-on info
supabase db advisors --local --type performance --level info --fail-on warn
supabase migration list --local
```

The Data API integration test uses the local status output without persisting its
keys:

```sh
export SUPABASE_URL="$(supabase status -o json | jq -r .API_URL)"
export SUPABASE_PUBLISHABLE_KEY="$(supabase status -o json | jq -r .PUBLISHABLE_KEY)"
node supabase/tests/api-e2e.mjs
node supabase/tests/realtime-e2e.mjs
```

The GitHub workflow also regenerates `packages/shared/src/database.types.ts` and
fails if it differs from the committed schema contract.

### Fresh-reset performance advisor notes

Immediately after `db reset`, the performance advisor reports only INFO-level
`unused_index` notices because `pg_stat_user_indexes` has no production workload
history. The reviewed indexes are retained intentionally:

- `rate_limits_cleanup_idx` and `idempotency_records_created_at_idx` bound scheduled
  retention deletes;
- `invite_codes_issued_by_idx`, `invite_redemptions_code_idx`, and
  `pet_projections_skin_id_idx` support foreign-key maintenance;
- `friendships_requester_idx`, `invite_codes_redeemable_idx`, and
  `pet_projections_updated_at_idx` support the documented social, operator, and
  projection read paths;
- the actor/counterpart/attempt/memory funnel indexes support the five beta funnel
  views as those fact tables grow.

Re-evaluate these with hosted query statistics after beta traffic exists. The CI gate
fails on performance warnings or errors; fresh-reset INFO notices are recorded but do
not fail the workflow.

## GitHub OAuth gate

The GitHub OAuth App must use these authorization callback URLs:

- Production: `https://etqrnvhxvptnyhcdxgtb.supabase.co/auth/v1/callback`
- Local: `http://localhost:54321/auth/v1/callback`

Creating the OAuth App and handing off its client ID and secret are deliberate
browser/user gates. Store the production secret in Supabase Auth provider settings,
not in this repository. Keep Realtime private-channel public access disabled and the
Auth JWT expiry at 600 seconds.

## Production migration procedure

Production DDL is manual and must run only from the `main` branch through the Cloud
workflow's preview and apply jobs. Before the first deployment:

1. Authenticate the CLI and confirm `etqrnvhxvptnyhcdxgtb` is visible.
2. Pull and inspect the existing remote schema, migration history, Auth settings,
   extensions, Realtime configuration, API exposure, and advisors.
3. Reconcile unknown remote objects non-destructively; never overwrite or drop them.
4. Configure protected `production` environment secrets `SUPABASE_ACCESS_TOKEN`,
   `SUPABASE_DB_PASSWORD`, and `SUPABASE_PROJECT_ID`.
5. Have a configured Line-B or Line-C reviewer approve the preview job, inspect its
   dry-run output, then approve the separate dependent apply job. GitHub evaluates
   the protected environment independently for each job, so production credentials
   remain environment-scoped and apply cannot begin before the second approval.
6. Run non-destructive Alice/Bob/stranger/blocked-user smoke tests and both advisors.

The workflow refuses a project ref other than `etqrnvhxvptnyhcdxgtb` and never
deploys migrations automatically on a push or pull request.
