# 生产云端部署（Supabase）

生产项目：`etqrnvhxvptnyhcdxgtb`（us-west-1，Yining 个人账号下 PawBae org）。
所有云端变更走 GitHub Actions 的 **cloud deploy** 工作流（`.github/workflows/cloud-deploy.yml`），
不在任何人的笔记本上直连生产跑一次性命令——部署要可审计、可重放、有验收。

> 现状（2026-07-12 实测）：生产库是**空的**——仓库里的全部迁移从未部署上云
> （`join_waitlist` 对外 404）。首次部署就是走下面的流程。

## 一次性配置（唯一 secret）

1. Supabase Dashboard → 项目 → **Connect** → 选 **Session pooler** 连接串
   （形如 `postgresql://postgres.etqrnvhxvptnyhcdxgtb:[YOUR-PASSWORD]@aws-0-us-west-1.pooler.supabase.com:5432/postgres`），
   把 `[YOUR-PASSWORD]` 换成数据库密码（密码含特殊字符需 percent-encode）。
   **必须用 Session pooler**：GitHub runner 没有 IPv6，直连 `db.<ref>.supabase.co` 不通。
2. 存进 GitHub secrets（连接串含密码，只能住在这里）：

   ```bash
   gh secret set SUPABASE_DB_URL --repo PawBae/PawBae-app
   # 粘贴完整连接串，回车
   ```

验证用的 `SUPABASE_URL` / `SUPABASE_PUBLISHABLE_KEY` 是仓库 Variables（#61 时代已配好）。

## 部署流程

```bash
# 1. 预览：只打印将应用的迁移，不改任何东西
gh workflow run "cloud deploy" --repo PawBae/PawBae-app -f action=dry-run

# 2. 看 dry-run 的输出没问题后，真推
gh workflow run "cloud deploy" --repo PawBae/PawBae-app -f action=deploy
```

（Actions 页面的 Run workflow 按钮等价。）

deploy 模式自动做完整验收，任何一项不过整个 run 红掉：

- **迁移对齐**：`supabase migration list` 本地/远端逐条对齐，有未应用的即失败；
- **公开 RPC 面**：直接问库，17 个公开函数（waitlist/邀请码/好友/拉黑静音/六访问动作/
  记忆结算与浏览/心跳/投影）缺一即失败；
- **pg_cron**：`pawbae-maintain-visits`（每分钟租约状态机）+ `pawbae-cleanup-runtime`
  两个任务必须已注册；
- **REST 冒烟**：用 publishable key 打真实 `join_waitlist`（非法邮箱，校验先于写入，
  无副作用）——404 说明 PostgREST schema cache 没跟上，同样失败。

## 邀请码播种

```bash
gh workflow run "cloud deploy" --repo PawBae/PawBae-app \
  -f action=deploy -f seed_invites=10 -f invite_max_uses=1 -f invite_expires_days=30
```

- 库里只存 `sha256(upper(code))` 哈希（与 `redeem_invite` 同口径）；
- **明文码不进日志**，只进名为 `invite-codes` 的 workflow artifact（3 天后自动过期）——
  从 run 页面下载 `codes.txt` 后自行分发；
- 重复运行是纯追加（每次都是新码），不影响已发出的码。

## 部署之后

1. **真机双人冒烟**（M2/M5 人证）：两台机器、两个 GitHub 账号——登录 → `redeem_invite`
   → 互加好友 → 串门 → 召回 → 相册出记忆。`recall → returning → recalled` 在 ~75 秒内
   收敛同时证明生产 pg_cron 活着。
2. Dashboard → Advisors 过一眼安全/性能建议。
3. 内测指标看 SQL 视图：`funnel_friend_request_acceptance` / `funnel_friend_to_first_visit`
   / `funnel_visit_completion` / `funnel_memory_view` / `funnel_seven_day_repeat_visit`
   （SV §9 五步漏斗，随迁移已建）。

## 边界与归属

- 本工作流归 **cloud 域（A 线）**，与 `cloud.yml`（CI 测试）是姊妹件：`cloud.yml`
  对一次性本地栈验证迁移正确性，`cloud-deploy.yml` 把同一批迁移推上生产。
- 迁移本身照旧走 PR 进 `supabase/migrations/`（CI 全绿才合）；部署是合并后的
  **人工触发**动作——与发布链「打 tag → 人工 publish」同款放量闸门。
- 连接串轮换：Dashboard 改数据库密码后重新 `gh secret set SUPABASE_DB_URL` 即可。
