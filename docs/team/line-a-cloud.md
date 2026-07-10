# 交接文档 · A 云端线（负责人：@zhihao-acc）

日期：2026-07-09 · 你的领地：`supabase/` + `packages/shared/` · 你不碰：`apps/desktop/`、`apps/website/`

## 0. 一分钟上下文

PawBae 是把 coding agent（Claude Code / Codex / Cursor）变成桌面宠物的 App，正在平台化：v1 = 平台账号 + 好友 + 异步串门 + 共同记忆，**三人十周到封闭内测**。你负责整个服务端：Supabase 上的一切 SQL、RLS、RPC、Realtime，以及三端共用的契约包 `packages/shared`。

必读（按顺序，全在本仓库）：

1. [v1 发布计划](../strategy/2026-07-09-v1-release-plan.md) —— 总纲，尤其 §5.1 服务端模式速查
2. [P1 服务端地基 spec](../superpowers/specs/2026-07-09-platform-foundation-design.md) —— 你 W1-2 的实施依据
3. [异步串门与共同记忆规划](../superpowers/specs/2026-07-09-social-visiting-design.md)（下称 SV）—— 产品 spec，你的 P4 实施 spec 要与它逐条对齐
4. [平台愿景](../strategy/2026-07-09-platform-vision.md) —— 为什么这么做

## 1. 使命与边界

**使命**：anon key 随桌面二进制公开分发，**RLS + SECURITY DEFINER RPC 是整个平台的全部安全边界**——这个边界的质量就是你这条线的质量。RLS 完整矩阵测试是 v1 的硬验收门槛。

**两条红线**（violate = 阻断发布）：

- **隐私**：服务端永不接受自由文本（唯一例外：`profiles.handle`/`display_name`，独立校验通道）；events 的 kind/params 超出字典即拒；好友可见数据一律走独立 public projection 表，**绝不放宽 `pets.snapshot` 的 SELECT RLS**；任何面向好友的数据剥离 `source` 等 agent 厂商信息。
- **never-punish**：不设计任何「失败扣除」——拒绝/召回/过期/拉黑不掉资源、不降关系；无社交/串门 streak、无亲密度数值接口（既有的个人连胜事件 `streak_milestone` 属于私有 events，不面向好友暴露）。

**明确不负责**：桌面 UI、上传管道的客户端实现（B 线）、官网（C 线）。你交付的形式永远是：迁移文件 + RPC + `packages/shared` 类型/构造器 + 测试。

## 2. 契约 v0（你拥有；变更需 B/C review 你的 PR）

这是三线并行的解耦核心：**契约先冻结，实现各自跑**。下面是 v0 草案，你在 W2 的 PR-B 里把它落成 `packages/shared` 代码，在 W3-4 的 P4 实施 spec 里定稿扩展。

```ts
// packages/shared —— 发布名 @pawbae/shared，对外类型（B/C 均消费）
export type ProjectionStatus = 'idle' | 'working' | 'waiting' | 'compacting' | 'offline'

export interface PublicPetProjection {
  v: 1                      // schema 版本，向后兼容演进
  petId: string
  displayName: string       // 来自 profiles.display_name ?? handle
  skinId: string            // 内置皮肤 id（白名单枚举）
  status: ProjectionStatus
  updatedAt: string         // ISO 8601
}

export type VisitStatus =
  | 'requested' | 'accepted' | 'traveling' | 'visiting' | 'returning'   // 未结束态
  | 'completed' | 'declined' | 'cancelled' | 'expired' | 'recalled' | 'blocked' // 终止态

export interface VisitLease {
  id: string
  visitorUserId: string     // 访客宠物的主人
  hostUserId: string
  status: VisitStatus
  startedAt: string | null  // accepted 之前为 null
  endsAt: string | null     // 固定 30 分钟租约
}

export interface FriendEntry {  // 随 W3-4 好友域 PR 进 shared（B 的 FriendsPanel 与 C 的 mock 在 W5 消费）
  userId: string
  handle: string
  displayName: string | null
  relation: 'pending_in' | 'pending_out' | 'accepted'
  muted: boolean
}

// 事件字典 v1（P1 spec §2.3）——构造器只收枚举/数值，无自由文本位
// task_completed{source} · egg_hatched{rarity} · souvenir_found{rarity} · streak_milestone{days}
```

**RPC 清单 v0**（全部 SECURITY DEFINER、钉死 `search_path`、带限速与 `is_blocked()` 检查；写操作带 `idempotency_key`）：

| 域 | RPC |
|---|---|
| 好友 | `send_friend_request` / `accept_friend_request` / `unfriend` / `block_user`（同事务删好友行）/ `mute_user` |
| 串门 | `request_visit` / `accept_visit` / `decline_visit` / `cancel_visit` / `recall_visit` / `end_visit` |
| 其他 | `connector_heartbeat`（更新 `pets.connector_seen_at`，限频）/ `update_projection`（**投影的唯一写入路径**；P4 实施 spec 定稿为 RPC 还是 RLS 限定的 owner UPDATE，B 的 W5-6 上传管道对着它写）/ `redeem_invite` / `join_waitlist` |

**Realtime 契约**：投影走**私有 Broadcast 频道**，topic = `pet:{ownerUserId}:{leaseId}`（owner = 访客宠物的主人；host 在租约有效期内订阅）。只有数据库触发器发布，客户端只读。

## 3. 逐周交付

### W1 —— 准备（注意：PR-A 排他窗口——**PR-A 合并前不开任何 PR**，「3 天」只是目标，以合并事件为准）

- [ ] 本地环境：Docker + supabase CLI，`supabase start` 跑通
- [ ] 建 Supabase 云项目（org 权限找 Yining）；把 `SUPABASE_URL`/`SUPABASE_ANON_KEY` 交给 B/C
- [ ] 提醒 Yining 建 GitHub OAuth App 并填进 Supabase Auth（B 线 W3 登录依赖此项）
- [ ] 在本地起草 PR-B 迁移（不开 PR）：`profiles`（含 `display_name`）/ `pets`（含 `connector_seen_at`）/ `events`
- [ ] 读完四份必读文档

### W2 —— PR-B：地基落地（M1）

> 前置：**PR-A 已合并**（`supabase/`、`packages/shared/` 目录由它创建）。若 PR-A 在 W1 第 3 天仍未合并：从 PR-A 分支切出继续开发，PR-B 以 stacked PR 开在其上，不干等。

- [ ] 迁移进 `supabase/migrations/`，`supabase db reset` 可重放
- [ ] `connector_heartbeat` RPC + 限频（**必须随本 PR**——B 的 W3-4 心跳上报是 M2 项，等的就是它；列已在同 PR 的 `pets` 建好）
- [ ] RLS：只写自己的行、匿名不可读写；`events` kind CHECK + params 字典校验（触发器）
- [ ] `packages/shared`：契约 v0 类型 + 事件字典构造器 + 快照 sanitizer（白名单 + 固定键序，参考 `apps/desktop/src/lib/utils/stage-bridge.ts` 的思路）
- [ ] vitest：构造器拒绝未知 kind / 超字典参数 / 自由文本
- [ ] RLS 测试脚本首版（`supabase/tests/`）+ CI job（keyless 可跑：shared 单测 + 迁移 dry-run）——写在你专属的 `.github/workflows/cloud.yml`（B 的 PR-A 已预创建骨架，合并后此文件归你，避免 CI 大改周里踩 B 的领地）

### W3-4 —— P4 实施 spec + 好友域（M2 周）

- [ ] **起草 P4 实施 spec**（`docs/superpowers/specs/` 新文件）：全部 SQL、RPC 签名、频道授权、投影白名单映射——与 SV 逐条对齐，交 Yining review
- [ ] `friendships`：单行表 `CHECK (user_a < user_b)` + 复合主键；拒绝/解除 = DELETE（不留墓碑）
- [ ] `blocks` 方向性表 + `is_blocked(a,b)` SECURITY DEFINER 双向检查
- [ ] 好友域 RPC 全套 + `rate_limits` 固定窗口计数表（与动作同事务）
- [ ] `waitlist` 表（insert-only + 限速）→ **W3 内交付**：C 的官网 W4 公开上线必须接真实表（stub 只许用于开发；公开收集报名禁止 no-op——静默丢报名是最贵的事故）
- [ ] `FriendEntry` 类型进 shared（契约 §2 已定义字段；B 的 FriendsPanel 与 C 的 mock 在 W5 消费）

### W5-6 —— 访问域（M3 周）

> 节奏硬约束：**visits schema + 全部访问 RPC 骨架 + Broadcast 授权在 W5 末合并**（硬检查点——B 的 W6 真实 PlatformClient 与 C 的 W7 换线都压在它上面）；W6 做投影管道、触发器与清理作业的细化。B 依据 W3-4 冻结的 P4 实施 spec 契约实现，不等你的代码合并。

- [ ] `visits` 表 + 状态机转换校验触发器
- [ ] `invite_codes` + `redeem_invite`（提前自 W7——不依赖访问域，而 C 的 W7 接线与 M6 注册闸门都等它）
- [ ] 部分唯一索引：`on visits (visitor_id) where status in ('requested','accepted','traveling','visiting','returning')`（谓词覆盖**全部未结束态**，SV 状态机里没有 `active`；不能用 `now()`——谓词必须 immutable）；host 侧同理
- [ ] 幂等：`unique (actor_id, idempotency_key)`，insert-on-conflict 后回读旧结果
- [ ] `pet_projections` 表 + 「私有快照 → 公开投影」映射（只进白名单字段，剥离一切统计与厂商信息）
- [ ] 私有 Broadcast：`realtime.messages` SELECT 策略（EXISTS 活跃租约 + 非拉黑 + 单索引查询）；数据库触发器发布，**发送端先查活跃租约再发**（真正的撤销点在发送端——删行不会踢已订阅者！）；JWT 短过期兜底 + `visit_ended` 礼貌广播
- [ ] pg_cron：每 30-60s 翻转过期租约到终止态；每日清理旧限速窗口 / 陈旧 pending（幂等键保留 ≥24h）

### W7-8 —— 安全矩阵 + 邀请码（M4 周）

- [ ] **RLS 完整矩阵**：Alice / Bob / 陌生人 / 被拉黑 × 访问前 / 中 / 后；伪造 owner / host / ends_at / memory participants 全部拒绝
- [ ] 限速全覆盖测试（换客户端不可绕过）
- [ ] W8 后半：`shared_memories` 结算 RPC（提前自 W9：模板键 + 安全参数如 `played_together`/时长档位/昼夜时段，**不存预渲染文本**，每次访问最多一条主记忆、重放安全）+ 模板键 **fixture 载荷进 shared**——C 的记忆卡渲染与文案走查靠 fixture 独立进行，不等你

### W9 —— 指标 + 缓冲（M5）

- [ ] funnel SQL 视图（SV §9 五步漏斗）——内测指标看 Supabase，不进 Aptabase
- [ ] 记忆域收尾 + 三线联调缓冲（`shared_memories` 主体已在 W8 落地，本周只吸收 B/C 联调发现）

### W10 —— 冻结，只修 bug

## 4. 独立开发策略（你如何不牵制别人 / 不被牵制）

- 你的日常产出都在 `supabase/` + `packages/shared/` + 专属的 `.github/workflows/cloud.yml`——与 B/C 的改动零交集（注意：这些目录本身由 B 的 W1 PR-A 创建，所以你的 W2 前置于 PR-A 合并），唯一同步点是契约 v0 的变更 PR。
- **交付节奏优先级 = 别人等的先做**：W1 云项目凭据（B 等）→ W2 契约 v0 + `connector_heartbeat`（B/C 等）→ W3 `waitlist`（C 的 W4 官网上线等）→ W5 末访问 RPC 骨架 + Broadcast 授权（B 的 W6 等）→ W5-6 `invite_codes`（C 的 W7 与 M6 等）。
- 诚实的关键路径声明：**Broadcast 授权在 B→C→M4 的关键路径上**（B 的 `subscribeGuestProjection` 包的就是它），所以定在 W5 末硬检查点；RLS 矩阵才是唯一不在别人关键路径上的深水区。
- B/C 全程用 `supabase start` 本地栈对着你的迁移开发——你合并迁移 PR 即交付，无需联调会。
- 需要桌面侧配合验证时（如心跳），写一个最小 Node/Deno 测试脚本模拟客户端，不等 B。

## 5. 工作流规范

- **不可直推 main**（仓库规则强制）；小步 PR，CI 全绿才合；Codex bot 会自动 review，逐条处理或说明。
- 验证命令：`pnpm test`（vitest，基线 360+ 全绿）、`pnpm check`（svelte-check 0 errors）、`pnpm lint`（biome）、`supabase db reset`（迁移可重放）。
- 基线不允许倒退：你的 PR 不应触碰 svelte-check 13 warnings 之外的任何既有告警。
- 密钥纪律：anon key 公开无妨（安全边界=RLS）；**service_role key 永不进仓库、永不进客户端**，只放 GitHub Actions secrets 与本地 `.env`（已 gitignore）。
