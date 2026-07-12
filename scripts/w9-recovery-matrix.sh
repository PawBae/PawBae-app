#!/usr/bin/env bash
# W9 恢复矩阵一键入口：对 `supabase start` 本地真实栈运行 SV §12「集成恢复」联调套件。
#
#   pnpm dlx supabase start        # 先起本地栈（需要 Docker）
#   bash scripts/w9-recovery-matrix.sh
#
# 与 w6-supabase-integration.sh 的差别：矩阵需要“时间快进”——把 visits 的时钟列
# 拨回过去并手动执行 private.maintain_visits()（与生产 cron 同一条代码路径），
# 因此额外注入 db 容器名。没有这些变量时该测试文件在普通 `pnpm test` 里自动跳过。
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

# db 容器名绑定本项目（supabase/config.toml 的 project_id）：本机同时跑多个
# supabase 栈时，全局抓第一个匹配可能把时钟快进打到别的项目的库上
PROJECT_ID="$(awk -F'"' '/^project_id[[:space:]]*=/ { print $2; exit }' supabase/config.toml)"
if [ -z "$PROJECT_ID" ]; then
  echo "supabase/config.toml 里找不到 project_id" >&2
  exit 1
fi
DB_CONTAINER="supabase_db_${PROJECT_ID}"
if [ "$(docker inspect -f '{{.State.Running}}' "$DB_CONTAINER" 2>/dev/null)" != "true" ]; then
  echo "本项目的 supabase db 容器 ${DB_CONTAINER} 未运行" >&2
  exit 1
fi

echo "stack=${API_URL} db=${DB_CONTAINER}"
PAWBAE_SUPABASE_URL="$API_URL" \
PAWBAE_SUPABASE_PUBLISHABLE_KEY="$PUBLISHABLE_KEY" \
PAWBAE_SUPABASE_DB_CONTAINER="$DB_CONTAINER" \
pnpm --filter @pawbae/desktop exec vitest run src/lib/platform/supabase-client.recovery.integration.test.ts
