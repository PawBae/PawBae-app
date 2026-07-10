# 平台 P1：服务端地基 + 单仓库改造 设计

日期：2026-07-09 · 愿景：[docs/strategy/2026-07-09-platform-vision.md](../../strategy/2026-07-09-platform-vision.md)

## 0. 用户决策（2026-07-09 岔路口）

| 岔路口 | 决定 |
|---|---|
| 后端形态 | Supabase（托管 Postgres + Auth + Realtime + Edge Functions） |
| 账号体系 | GitHub OAuth 首发（App Store 发布时补 Sign in with Apple） |
| 手机框架 | Tauri 2 移动端（本期不实施，地基保证兼容） |
| 仓库结构 | 单仓库 pnpm workspace 改造 |

## 1. 单仓库改造（PR-A，零行为变化）

目标结构：

```
apps/desktop/        ← 现有 src/ src-tauri/ public/ 等整体平移（git mv 保留历史）
packages/shared/     ← 新建：事件字典 + 快照 schema + 纯工具（本期落地字典 v1）
supabase/            ← PR-B 落地（migrations + config）
```

- 根 `pnpm-workspace.yaml`；根 package.json scripts 转发到 apps/desktop（`pnpm tauri dev` 手感不变）。
- CI 全量更新：ci.yml / release.yml / CodeQL 的路径过滤与 working-directory；tauri.conf.json 内相对路径核对（icon、frontendDist、beforeDevCommand）。
- 验收标准：`pnpm tauri dev`、vitest（360 基线）、svelte-check（13 warnings/0 errors 基线）、biome、clippy、rustfmt 全部照常通过；`git log --follow` 能追到平移前历史。
- **排他窗口**：本 PR 移动所有文件，落地前不得有其他开着的 PR（当前无）。

## 2. Supabase 地基（PR-B）

### 2.1 项目与环境

- Supabase 云项目（免费档起步）+ supabase CLI 本地栈（`supabase start`）用于开发与迁移测试；迁移文件进 `supabase/migrations/`。
- 前端环境变量 `SUPABASE_URL` / `SUPABASE_ANON_KEY`（anon key 公开、随二进制分发，与 Aptabase key 同性质；安全边界完全由 RLS 承担）。

### 2.2 数据模型（v1 最小）

- `profiles`：id（uuid = auth.users.id）、handle（唯一，初值取 GitHub login，可改）、avatar_url、created_at。
- `pets`：user_id（pk）、snapshot jsonb、updated_at。snapshot 是手机镜像的唯一数据源，字段白名单：petId、spriteState/mood 枚举、hunger、level、streak、away 等**数值/枚举**（思路镜像 stage-bridge 的 sanitize + 固定键序）。
- `events`：id、user_id、kind（枚举）、params jsonb、occurred_at。append-only；之后喂好友时间线与投喂记录。
- `friendships` / 投喂表**留到 P4 spec**，本期不建。

### 2.3 隐私门（双层）

- **第一层（客户端，packages/shared）**：事件字典 v1 —— `task_completed{source}`、`egg_hatched{rarity}`、`souvenir_found{rarity}`、`streak_milestone{days}`。构造函数只接受枚举/数值，无自由文本参数位；快照 sanitizer 同理。桌面端只能通过字典构造器产出上传载荷。
- **第二层（服务端）**：RLS（只能写自己的行）+ kind CHECK 约束 + params 按字典白名单校验（触发器或 edge function），超字典即拒。
- 一切默认关：不登录、不开上传开关时，桌面 App 行为与今日完全一致（P2 才接线）。

### 2.4 Auth 与 Realtime

- GitHub OAuth（Supabase Auth 内置 provider）。桌面端登录 UI 属 P2；本期用 supabase CLI / 测试脚本验证 auth 流程与 RLS。
- `pets` 行开启 Realtime，手机端订阅即得秒级镜像（P3 消费）。

## 3. 测试计划

- packages/shared：vitest 单测——字典构造器拒绝越界输入（未知 kind/超字典参数/自由文本）、快照 sanitizer 白名单与固定键序。
- supabase 本地栈：迁移可重放（reset 后 re-apply）；RLS 测试脚本——他人行不可写、匿名不可读写、超字典 kind/params 被拒。
- CI：新增 job 跑 shared 单测 + supabase migration dry-run（不依赖云端密钥，keyless 可跑）。

## 4. 风险与后续

- Supabase 免费档配额（Realtime 并发 / 行数 / edge 调用）——到量升付费档，成本随用户数线性可见。
- 滥用与限速：服务端每用户事件速率限制（P2 上传端同时节流）。
- anon key 的安全边界 = RLS 质量 → **RLS 测试是本期验收硬门槛**。
- Tauri 2 移动端推送插件（APNs/FCM 社区插件）成熟度在 P3 期验证，不阻塞本期。
- 后续 spec 队列：P2 桌面 Connector（登录 + opt-in 上传）、P3 手机 App v1（远程镜像）、P4 三件套（好友/看宠物/投喂）。
