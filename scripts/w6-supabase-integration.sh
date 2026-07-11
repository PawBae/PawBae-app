#!/usr/bin/env bash
# W6 联调一键入口：对 `supabase start` 本地真实栈运行 SupabasePlatformClient 集成测试。
#
#   pnpm dlx supabase start        # 先起本地栈（需要 Docker）
#   bash scripts/w6-supabase-integration.sh
#
# 做三件事：发现本地栈地址与 publishable key → 以 postgres 身份播种一张一次性
# 邀请码（redeemInvite 覆盖用）→ 注入环境变量运行 vitest 集成文件。
# 没有这些变量时该测试文件在普通 `pnpm test` 里自动跳过。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

STATUS_RAW="$(pnpm --silent dlx supabase status -o json 2>/dev/null)" || {
  echo "supabase 本地栈未运行——先执行: pnpm dlx supabase start" >&2
  exit 1
}
read -r API_URL PUBLISHABLE_KEY < <(node -e '
  const raw = process.argv[1];
  const s = JSON.parse(raw.slice(raw.indexOf("{")));
  if (!s.API_URL || !s.PUBLISHABLE_KEY) throw new Error("supabase status 输出缺少 API_URL/PUBLISHABLE_KEY");
  console.log(s.API_URL, s.PUBLISHABLE_KEY);
' "$STATUS_RAW")

DB_CONTAINER="$(docker ps --filter name=supabase_db_ --format '{{.Names}}' | head -1)"
if [ -z "$DB_CONTAINER" ]; then
  echo "找不到 supabase db 容器（docker ps --filter name=supabase_db_）" >&2
  exit 1
fi
# 不用 </dev/urandom | head 组合：head 关管道会让 pipefail 下的脚本吃 SIGPIPE(141) 静默死掉
INVITE_CODE="W6-$(node -e 'console.log(require("node:crypto").randomUUID().replaceAll("-","").slice(0,12).toUpperCase())')"
docker exec "$DB_CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 -q -c \
  "INSERT INTO public.invite_codes (code_hash, max_uses, expires_at)
   VALUES (extensions.digest(upper('${INVITE_CODE}'), 'sha256'), 10, now() + interval '1 hour');"

echo "stack=${API_URL} invite=${INVITE_CODE}"
PAWBAE_SUPABASE_URL="$API_URL" \
PAWBAE_SUPABASE_PUBLISHABLE_KEY="$PUBLISHABLE_KEY" \
PAWBAE_INVITE_CODE="$INVITE_CODE" \
pnpm --filter @pawbae/desktop exec vitest run src/lib/platform/supabase-client.integration.test.ts
