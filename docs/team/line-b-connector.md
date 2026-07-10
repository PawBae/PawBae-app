# 交接文档 · B 连接线（负责人：@miaomeng1）

日期：2026-07-09 · 你的领地：`apps/desktop/` 的账号与数据管道、`src-tauri/` Rust 层、`.github/workflows/` 发布链（`cloud.yml` 除外，归 A）· 你不碰：`supabase/` 的 SQL（A 线）、`apps/website/` 与串门渲染（C 线）

> 路径约定：本文所有 `src/`、`src-tauri/` 均指 W1 单仓库改造后 `apps/desktop/` 下的对应目录（改造前 = 仓库根目录同名路径）。

## 0. 一分钟上下文

PawBae 是把 coding agent（Claude Code / Codex / Cursor）变成桌面宠物的 App，正在平台化：v1 = 平台账号 + 好友 + 异步串门 + 共同记忆，**三人十周到封闭内测**。你负责把现有桌面 App 变成「平台的最强接入端」：单仓库改造、GitHub 登录、opt-in 数据管道、以及让 App 能真正发到陌生人手里的整条发布链（签名/公证/更新验签/崩溃上报）。

必读（按顺序，全在本仓库）：

1. [v1 发布计划](../strategy/2026-07-09-v1-release-plan.md) —— 总纲，尤其 §5.2 发布就绪差距清单
2. [P1 服务端地基 spec](../superpowers/specs/2026-07-09-platform-foundation-design.md) —— PR-A 单仓库改造是你的 W1
3. [异步串门与共同记忆规划](../superpowers/specs/2026-07-09-social-visiting-design.md)（下称 SV）—— §2.4 与 §5.4 直接约束你的管道
4. [A 线交接文档](line-a-cloud.md) §2 —— 契约 v0（你消费它，也拥有桌面侧的 PlatformClient 接口）

## 1. 使命与边界

**使命**：用户「登录 → 逐项打开上传开关」之前，App 行为与今日完全一致（默认全关是产品承诺）；打开之后，只有白名单里的枚举/数值离开这台电脑。你是这个承诺的实现者。

**两条红线**（violate = 阻断发布）：

- **隐私**：上传管道只能通过 `packages/shared` 的字典构造器产出载荷——不存在「顺手多传一个字段」的代码路径；心跳只证明存活，不带会话内容/项目标识/工作计数；投影状态只有 5 个枚举值。
- **更新安全**：更新器验签落地前，不给任何外部用户开自动更新。

**明确不负责**：SQL/RLS（A 线）、串门渲染与官网（C 线）。

## 2. 你拥有的接口：PlatformClient（C 线消费，变更需 C review）

桌面内部 B↔C 的解耦核心。你实现真实版，C 拿接口写 mock 先行开发串门 UI，W7 换线。接口文件建议放 `apps/desktop/src/lib/platform/types.ts`：

```ts
import type { PublicPetProjection, VisitLease, FriendEntry } from '@pawbae/shared'

export interface PlatformClient {
  // 会话
  session(): PlatformSession | null            // null = 未登录（App 必须照常工作）
  onSessionChange(cb: (s: PlatformSession | null) => void): Unsubscribe

  // 串门（C 消费）——覆盖 A 线 RPC 清单的全部六个访问动作
  requestVisit(hostUserId: string, idempotencyKey: string): Promise<VisitLease>
  respondVisit(leaseId: string, action: 'accept' | 'decline', key: string): Promise<VisitLease>
  cancelVisit(leaseId: string, key: string): Promise<VisitLease>   // 访客撤回仍处 requested 的邀请
  recallVisit(leaseId: string, key: string): Promise<VisitLease>
  endVisit(leaseId: string, key: string): Promise<VisitLease>      // 任一方提前结束访问
  onLeaseChange(cb: (lease: VisitLease) => void): Unsubscribe
  subscribeGuestProjection(lease: VisitLease, cb: (p: PublicPetProjection) => void): Unsubscribe

  // 邀请码（C 的 onboarding 消费）
  redeemInvite(code: string, key: string): Promise<void>

  // 好友（你自己的 FriendsPanel 消费）
  friends(): Promise<FriendEntry[]>
  // …好友请求/拉黑/静音，签名对齐 A 线 RPC 清单
}

export interface PlatformSession {
  userId: string
  handle: string
  displayName: string | null
  avatarUrl: string | null
}
export type Unsubscribe = () => void
// FriendEntry 的权威定义在 @pawbae/shared（A 线契约 §2，随其 W3-4 好友域 PR 交付）
```

实现要点：包一层 supabase-js（auth + RPC + 私有 Broadcast 订阅），所有 RPC 调用自动附带幂等键与错误归一化。**C 的 MockPlatformClient 也实现这个接口**——接口一变两边都断，所以变更走 PR 且 C 必须 review。**接口文件本身是你 W3 的第一项交付**（interface-only、能对着 `@pawbae/shared` 编译、无实现；C review 合并即冻结）——C 的 W5 mock 完全依据它构建，这个日期不能滑。

## 3. 逐周交付

### W1 —— PR-A 单仓库改造（排他窗口，目标 3 天内合并）

全仓库移动到 `apps/desktop/` + 新建 `packages/shared/`、`supabase/` 占位 + 根 `pnpm-workspace.yaml`（目前不存在）。**窗口期内 A/C 不开 PR，你独占主干。**

雷区清单（逐项核对，来自代码库审计）：

- [ ] `git mv` 保历史；验收 `git log --follow apps/desktop/src/lib/stores/pet.svelte.ts` 能追到移动前
- [ ] **ci.yml PR 路径过滤**：`src/**`→`apps/desktop/src/**` 等——漏改 = CI 静默停跑、PR 裸奔合并（最大的坑）
- [ ] ci.yml / release.yml 的 `rust-cache` `workspaces: src-tauri` → `apps/desktop/src-tauri`；各 job 的 `--manifest-path`
- [ ] release.yml 的 tauri-action 加 `projectPath: apps/desktop`
- [ ] `biome.json` `files.includes`（漏改 = lint 静默空转）；`package.json` 的 `lint` 脚本路径
- [ ] `$lib` 别名三处同步：`vitest.config.ts` / `vite.config.js` / `tsconfig.json`
- [ ] `tauri.conf.json` 相对路径：icon、frontendDist、beforeDevCommand
- [ ] 根 package.json scripts 转发（`pnpm tauri dev` 手感不变）
- [ ] CI 给 `packages/shared/**`、`supabase/**` 加路径触发（为 A 铺路）
- [ ] 预创建 keyless 的 `.github/workflows/cloud.yml` 骨架（shared 单测 + 迁移 dry-run 的空壳；合并后此文件归 A，避免 A 在你的 CI 大改周里动 `ci.yml`）
- [ ] 验收：vitest 360 / svelte-check 0 errors 13 warnings / biome / clippy / rustfmt 全绿 + 本地 `pnpm tauri dev` 正常

### W2 —— 发布安全双件（M1；完全独立，不等任何人）

- [ ] **更新器 minisign 验签**：生成密钥对（私钥→GitHub secrets，公钥编译进二进制）；`latest.json` 加每资产签名字段；`src-tauri/src/commands/update.rs` 安装前校验，验签失败即中止。现状是 HTTPS 裸信任、下载即执行——这是 P0 安全债
- [ ] **崩溃上报**：Rust panic hook + webview `onerror`/`unhandledrejection`（Sentry 或最小自建）；隐私一致性：堆栈不带用户路径等敏感串
- [ ] Intel Mac 更新槽位修复：清单加 `macos-x64` 槽 + `check_for_update` 按 arch 取（现状 Intel 用户会收到 arm64 DMG）

### W3-4 —— 桌面登录 + 心跳（M2 周）

依赖（W1 末应已就绪，若缺立即催）：A 的云项目凭据；Yining 建的 GitHub OAuth App；A 的 PR-B（W2 末，`@pawbae/shared` 由它产生）。

- [ ] **W3 第一项：落地 `src/lib/platform/types.ts`**（PlatformClient 完整接口 + PlatformSession/Unsubscribe，interface-only 无实现，能对着 `@pawbae/shared` 编译）——C review 合并即冻结，C 的 W5 mock 靠它开工
- [ ] GitHub OAuth 桌面流（loopback 端口或 deep-link 回调），supabase-js 会话持久化
- [ ] 设置页「账号」区：**新建组件** `AccountSection.svelte`（登录/登出/handle 展示/opt-in 开关组，默认全关）——挂载进 `src/lib/components/settings/SettingsPanel.svelte`（设置区块都住这里，**不是** `Panel.svelte`）的那一行由 owner C 在**同一周**合入：M2「桌面登录打通」要从真实 UI 走通，挂载不能攒到后面
- [ ] `connector_heartbeat` 定时上报（运行中每 ~60s；节流、失败静默重试）
- [ ] opt-in 事件上传：现有奖励/孵蛋/纪念品/连胜时刻 → shared 字典构造器 → events（逐项开关）

### W5-6 —— 数据管道 + 好友 UI（M3 周）

- [ ] PlatformClient 真实实现（§2 接口全量）——实现依据是 A 在 W3-4 冻结的 P4 实施 spec 契约，**不等 A 的访问域代码合并**；A 承诺 W5 末合并访问 RPC 骨架 + Broadcast 授权（硬检查点），你 W6 对着 `supabase start` 接真实栈
- [ ] 投影发布管道：`agent-activity` 聚合信号 → `ProjectionStatus` 5 枚举 → 防抖/去重上传（复用 `stage-bridge.ts` 的 snapshotKey/snapshotsEqual 思路）；仅在存在活跃租约时发布
- [ ] **新建组件** `FriendsPanel.svelte`：好友列表、请求收发、静音/拉黑（设计基调见发布计划 §5.3 玻璃卡片体系）；挂载进 `Panel.svelte` 的行由 owner C 在**同一周**合入（M3 好友 e2e 等它）
- [ ] i18n：只在 `en.json`/`zh.json` 里你自己的前缀区块（`account.*`、`friends.*`）追加

### W7-8 —— 发布链（M4 周）

- [ ] macOS 签名+公证：release.yml 注释块接线（等 Yining 的 Apple 账号 6 个 secrets）；hardened runtime + `com.apple.security.device.audio-input` entitlement（语音功能）；上线后删 `update.rs` 的 `xattr -cr` hack
- [ ] Windows 签名：Azure Trusted Signing（`bundle.windows.signCommand` + `trusted-signing-cli`）；账号未就绪则准备好接线点，首波邀请可带说明先行
- [ ] 版本号单源 bump 脚本（tauri.conf.json / package.json / Cargo.toml 三处现在手动同步，会漂移）；CI 自动生成更新清单 JSON（发布仍手动 = 放量闸门）

### W9 —— 联调与异常矩阵（M5）

- [ ] SV §12「集成恢复」测试执行：requested/traveling/visiting/returning 各阶段断网、跨租约结束重启、双端同时召回/拉黑/到期竞态——不出分身、不重复记忆、不卡访问态

### W10 —— 签名安装包出厂（M6）

## 4. 独立开发策略

- W1-2 **零依赖**：改造与发布安全不需要 A/C 的任何产出。
- W3 起的依赖都在时间线上先行就绪：云项目（A 的 W1）、契约 v0（A 的 W2）、OAuth App（Yining 的 W1）。缺了立即在群里催，不要绕。
- 服务端没好的部分：对着 `supabase start` 本地栈 + A 的迁移开发，不等云端。
- UI 集成走 **stub-first**：你在新文件里开发完整组件，热点文件（`Panel.svelte`、`MascotView.svelte`）里的挂载行由 owner（C）合入——你俩的 PR 永不冲突。

## 5. 工作流规范与热点文件所有权

- **不可直推 main**；小步 PR，CI 全绿才合；Codex bot 自动 review，逐条处理。
- 验证命令：`pnpm test` / `pnpm check` / `pnpm lint` / `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --all-targets -- -D warnings` / `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`（与 ci.yml 完全一致；改造前 manifest 路径为 `src-tauri/Cargo.toml`）。
- 你是 owner：`pet.svelte.ts`、`src-tauri/src/lib.rs` invoke_handler、`commands/mod.rs`——别人改这些必须你点头；反之 `MascotView.svelte`/`Panel.svelte`/`settings/SettingsPanel.svelte` 是 C 的，`packages/shared` 与 `.github/workflows/cloud.yml` 是 A 的（其余 workflows 归你）。
- 密钥纪律：minisign 私钥、Apple 证书、Trusted Signing 凭据全部只进 GitHub secrets；泄露即轮换。
