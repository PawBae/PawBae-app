# PawBae v1 收口计划：两人到封闭内测

日期：2026-07-22<br>
关系：本计划是 [v1 发布计划](2026-07-09-v1-release-plan.md) 的收口附录；产品边界继续以[异步串门与共同记忆规划](../superpowers/specs/2026-07-09-social-visiting-design.md)为准，云端安全契约继续以 [Line-A P4 实施 spec](../superpowers/specs/2026-07-10-line-a-p4-implementation.md) 为准。它不重写已合入的 Line-A 地基，只负责把已合代码变成可用、可部署、可验收的封闭内测产品。

## 0. 已确认事实与本轮决策

### 0.1 当前事实（2026-07-22）

- `origin/main` 已合并 Line-A、真实 PlatformClient、恢复矩阵、共同记忆和发布链代码；主线 CI 与本地 Cloud 验证均为绿色。
- 这不等于 v1 已上线：生产 Cloud 部署从未运行，生产库尚无本仓库的 RPC/迁移验证，且没有非草稿 beta release。
- `pawbae.ai` 尚未切换到仓库中的 PawBae 官网；线上候补名单和下载不能作为 v1 入口验收。
- GitHub PKCE + loopback 登录底层、租约/轮询/私有 Broadcast、投影和记忆管道已有实现；主要缺口是首次引导接线、社交操作 UI、桌面双宠渲染和真实 2D 资产。

### 0.2 本计划采用的产品决定

| 岔路口 | 本轮决定 |
| --- | --- |
| 目标 | 先交付邀请制封闭内测，不宣称公开正式版。 |
| 最小用户旅程 | 官网/邀请码 → 下载 → GitHub 登录 → 兑换邀请码 → 加好友 → 串门 → 双宠同屏 → 结束/召回 → 共同记忆。 |
| 联机模型 | 保持数据库租约为正确性来源；轮询和私有 Broadcast 只用于及时呈现。不开 WebRTC、同步坐标、聊天或多人房间。 |
| 宠物资产 | Solu 是首只完整官方 2D 宠物与 beta 门槛。Muru/Riffi/Luma 在完成正式 atlas、`pet.json` 和 QA 前必须不可选择，不能回退为 Yoonie。 |
| 服务端改动 | 不重做社交 schema/RPC；只允许为账号资格恢复、批准皮肤或现有契约缺口做小型、经审查的迁移与共享类型变更。 |
| 发布 | 所有生产 DDL 仍只走人工触发的 cloud deploy；不自动推送生产迁移或发布。 |

## 1. v1 收口范围

**用户旅程一句话**：一对真实好友可在不打开开发工具的情况下登录、相互加好友、让一只一致的 2D 宠物去朋友桌面串门，并在结束后看到共同记忆。

| 包含 | 不包含（后续版本） |
| --- | --- |
| GitHub 登录、会话恢复、邀请资格、离线本地模式 | 邮箱/Apple 登录、手机 App |
| 精确 handle 加好友、接受、解除、静音、拉黑 | 用户广场、推荐、陌生人匹配、排行榜 |
| 一对一、30 分钟租约的异步串门、召回、到期收敛 | 多访客、公共房间、共同操控、实时坐标 |
| Solu 的完整 spritesheet、访客宠物、确定性互动 | 未经 QA 的其余官方宠物、Live2D/3D 重写 |
| PawBae 官网、候补名单、签名 beta release | 营销自动化、付费、完整内容站 |

两条红线继续有效：任何共享数据只能来自白名单 projection；拒绝、召回、离线、过期和拉黑都不能惩罚用户或泄露私有 agent 数据。

## 2. 两条开发线

两名工程师都按小步 PR、测试先行、CI 全绿、review 后合并工作。预计每条线约 4–5 个工程周；创始人配置凭据、域名切换和真人测试的等待时间不计入工程工时。

### A · 账号、社交与上线线

负责人：工程师 A<br>
主领地：`supabase/` 的收口改动、`packages/shared/`、`apps/desktop/src/lib/platform/`、账号/访问 store、`Onboarding.svelte`、`SocialHome.svelte`、`FriendsPanel.svelte`、`Main.svelte`。

1. **生产 Cloud 与账号前置**
   - 在 `SUPABASE_DB_URL` 配置后执行 cloud deploy dry-run，再由人工确认执行 deploy、迁移对齐、17 个公开 RPC、两个 cron job 和无副作用 REST 冒烟。
   - 核验 GitHub OAuth App 的生产回调与 loopback 回调；不新增第二种认证协议。
   - 将 `Main.svelte` 的登录回调传给 `Onboarding`，修复首次引导中的 GitHub 登录按钮不可用。
   - 收敛账号状态为单一来源：`initializing`、`signedOut`、`signingIn`、`signedIn`、`error`；登录后使用服务端 canonical profile，metadata 只作展示 fallback。

2. **邀请码与好友闭环**
   - 为邀请码兑换补产品 UI，并实现可在重启后恢复的邀请码资格读取；不得把资格仅存为本地布尔值。
   - 扩展 `PlatformClient`，接入精确 handle 查询、发送/接受好友请求、解除好友、静音和拉黑；组件不直接调用 Supabase。
   - 将 FriendsPanel 分为待接受、已发送、已成为好友，启用目前 disabled 的搜索和邀请操作。

3. **串门操作、状态与验证**
   - 接通请求、接受、婉拒、取消、召回、提前结束等已有访问状态机；发起前展示一次投影授权说明。
   - 用真实频道状态取代硬编码的 `connected`；断线时可保留最后安全画面，但必须由本地 `ends_at` 和轮询结束访问。
   - 主导生产两账号、两机器验收和签名 beta release 候选。

交付验收：两个真实 GitHub 账号可完成登录、重启恢复、邀请码、加好友、串门、召回与拉黑撤权；陌生人、过期租约和伪造身份均被服务端拒绝。

### B · 2D 舞台与官网线

负责人：工程师 B<br>
主领地：`SpritePet.svelte`、`GuestPet.svelte`、`MascotView.svelte`、`HomePetArtwork.svelte`、`apps/desktop/public/assets/`、`apps/website/`。

1. **Solu 资产纵切**
   - 按 [official pets pipeline](../superpowers/plans/2026-07-10-official-pets-pipeline.md) 交付透明帧、atlas、`pet.json`、manifest 与桌面 QA；旧路径需适配 monorepo 的 `apps/desktop/`。
   - 最小动作集：`idle`、`running`、`working`、`waiting`、`happy`、`sleep`、`arrival`、`return`。每态少量一致帧优先于大量不稳定帧。
   - 增加资产校验：图集尺寸、帧范围、必需状态、manifest 完整性，以及所有可选宠物精确解析为自身 ID。
   - 与 A 配合将 Solu 加入服务端批准皮肤白名单；否则远端 projection 不能可靠显示 Solu。

2. **双宠串门舞台**
   - 本机、Home 与访客全部通过同一 `PetAssetResolver`/SpritePet 解析皮肤；移除仅靠海报裁切表达宠物身份的路径。
   - 将已有 `GuestPet.svelte` 挂入 `MascotView`。出访方显示空窝、倒计时和召回入口；接待方显示本宠与访客。
   - 用 `(leaseId, localStatus, guestStatus, timeBucket, reducedMotion)` 驱动确定性互动，如抵达、并坐、碰鼻、庆祝和休息；不广播逐帧位置。
   - 租约到期、召回、拉黑、解除好友或失效时立即撤下访客。

3. **官网与视觉工程**
   - 选择性移植 `design/website-v1` 的视觉方向到当前 monorepo；保留当前 waitlist RPC 与更新清单契约，不直接合并旧分支。
   - Hero 和功能区使用真实桌面截图、透明宠物素材与双宠画面，不使用假终端/emoji 作为产品证明。
   - 建立语义设计 token、375/768/1440 响应式检查、键盘焦点、reduced-motion 和视觉截图回归。
   - 完成 Vercel 部署候选、PawBae 域名切换与线上 waitlist 验证。

交付验收：同一只 Solu 在 onboarding、Home、自身桌面与好友桌面一致可见；访问结束立即收敛；官网公开呈现 PawBae 而非旧产品。

## 3. 四周里程碑

| 阶段 | A · 账号、社交与上线 | B · 2D 舞台与官网 | 里程碑 |
| --- | --- | --- | --- |
| G0：生产前提 | 准备 Cloud dry-run、OAuth 核验与测试账号清单 | 冻结 Solu 资产规格、官网媒体清单 | 创始人已配置 DB secret、OAuth 回调、Vercel/DNS 权限和两名测试用户 |
| W1 | 首次登录接线、账号状态收敛、生产 dry-run/deploy | 禁止选择占位宠物；Solu atlas、manifest、资产测试 | **M1：访问与宠物身份真实化** |
| W2 | 邀请码、资格恢复、好友请求/接受/解除/拉黑 UI | Home 与桌面统一使用真实 SpritePet；Solu QA | **M2：两账号成为好友，宠物身份一致** |
| W3 | 串门操作、真实连接状态、断线与租约收敛联调 | GuestPet、空窝、互动、撤场 | **M3：双人串门 aha** |
| W4 | 生产双人测试、恢复矩阵复跑、签名 beta release 候选 | 官网切换、真实媒体、视觉回归 | **M4：可邀请的封闭内测候选** |

滑动规则：生产部署、登录、好友、双宠串门是不可滑出的 v1 底线；未完成的 Muru/Riffi/Luma 作为内容扩展滑出 beta，不得以占位或错误回退方式进入首批用户旅程。

## 4. 协作规则与热点文件

- A 独占 `Main.svelte`、`Onboarding.svelte`、`platform/`、账号/访问 store、好友/访问操作 UI 与生产部署。B 不直接修改这些入口文件；需要的舞台能力先以 props/类型提出。
- B 独占 `MascotView.svelte`、`GuestPet.svelte`、`SpritePet.svelte`、`HomePetArtwork.svelte`、宠物资产与 `apps/website/`。
- `packages/shared` 或 Supabase 契约由 A 提出变更，B 必须 review；每次契约变更都必须附本地 Supabase 测试。
- 两人不并行大改同一 Home 文件。A 先合功能；B 的视觉改动在功能接口稳定后单独 PR。
- 从 `origin/main` 建独立 worktree 开发；不在带有用户未跟踪文件的旧 worktree 中工作。
- 不直推 `main`，不把 secrets、服务角色 key、邀请码明文写入仓库、测试快照或 PR。

## 5. 技术基调与验收策略

### 5.1 登录与隐私

- 继续使用现有 Supabase PKCE + loopback 方案；浏览器打开只允许受信任 HTTPS URL，错误、取消、超时和重试均有可见状态。
- 登录不是社交数据授权的替代品；投影必须遵循既有 opt-in 总闸和白名单，只共享宠物外观、名称与批准的有限状态。
- 不新增自由文本上云路径；好友可见数据不读取 `pets.snapshot` 或 agent 事件原文。

### 5.2 联机正确性

- 数据库访问租约与 `ends_at` 是唯一正确性来源；Broadcast 是低延迟提示，不能作为租约、撤权或宠物位置的唯一来源。
- 每次发送投影都重新检查活跃租约、好友关系和拉黑状态。撤销后停止发送，客户端以轮询和本地到期兜底。
- 所有可重试写操作保留用户意图级 idempotency key；重试返回同一 canonical 结果。

### 5.3 2D 与网页素材

- 官方宠物保持固定角色母版和人工 QC；图像生成可辅助背景、构图、姿势候选，但不独立生成连续动画帧。
- 营销素材优先真实 App capture + 已批准透明宠物图层；输出 AVIF/WebP，保留 PNG fallback，并标注尺寸与加载策略。
- 网站与桌面共享语义色板、圆角、阴影、动效原则，不强行共享布局组件。

### 5.4 测试地图

- 每个行为变更先补失败测试；合并前运行 `pnpm test`、`pnpm check`、lint、clippy、rustfmt 与相关数据库测试。
- 登录：首次登录、取消、超时、重试、重启恢复、登出、未配置降级。
- 社交：匿名/跨用户/伪造 actor、重复请求、并发请求、拉黑、解除好友、过期租约。
- 串门：请求/接受、出访空窝、访客可见、状态变化、断网重连、召回、到期、拉黑即时撤权、共同记忆结算。
- 视觉：Solu atlas 无 404、无跳帧/尺寸抖动，375/768/1440 网站截图与 reduced-motion 场景通过。

## 6. v1 已落地的定义

只有以下全部满足，才可以把 v1 标记为 landed：

1. 生产 Supabase 的迁移、17 个公开 RPC、两个 cron job 和 REST 冒烟均有当前证据。
2. GitHub OAuth、邀请码、好友和串门可由两台真实机器、两个真实账号完成，全程不用开发工具。
3. Solu 在 onboarding、Home、本机桌面和好友桌面均以同一批准皮肤显示；未完成宠物不可选。
4. 召回、到期、断网恢复、解除好友和拉黑均安全结束访问，且不泄露私有数据。
5. `pawbae.ai` 是 PawBae 官网，候补名单生产可用，更新清单保持可访问。
6. 已发布一个非草稿、已签名的封闭 beta release，并完成安装与更新冒烟。
7. CI、数据库/RLS 矩阵、恢复矩阵和两人真人验收均为绿色。

## 7. 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| DB secret 或 OAuth 回调未配置 | 将 G0 设为硬门槛；未完成时只做本地设计/测试，不伪造生产通过。 |
| 旧交接文档把代码合并误写成 v1 已完成 | 本计划的 §6 是唯一 landed 判定；PR #76 合并前需按当前生产证据修正。 |
| 四只官方宠物资产拖慢 beta | 只以 Solu 作为 beta 资产门槛；其余三只隐藏并独立排期。 |
| A/B 同改组合入口导致冲突 | §4 的文件所有权 + 先定义 props/契约 + 小 PR。 |
| Realtime 断线或撤权存在残影 | 以租约时间、轮询和发送端守门为正确性基础；Broadcast 仅作加速。 |
| 官网先切而生产 RPC 未部署 | 固定顺序：Cloud 验证 → waitlist 冒烟 → 官网/域名切换；更新清单始终可访问。 |

## 8. 创始人待办（工程师不能代做）

1. 在 GitHub 配置 `SUPABASE_DB_URL`，并确认首次 cloud deploy 的 dry-run 与 deploy。
2. 确认 GitHub OAuth App 的生产/loopback 回调配置。
3. 授权 Vercel 与 DNS 切换；确认切换窗口。
4. 确认 beta 采用「Solu 完整、其余三只暂不可选」的范围决定；若改为四只同时首发，需增加相应资产周期。
5. 安排两位真实好友使用两个真实 GitHub 账号进行最终验收，并决定首批邀请码数量。
