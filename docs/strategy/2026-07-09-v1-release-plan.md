# PawBae v1 发布计划：三人十周到封闭内测

日期：2026-07-09
关系：[平台愿景 P0](2026-07-09-platform-vision.md) 定方向，[P1 服务端地基 spec](../superpowers/specs/2026-07-09-platform-foundation-design.md) 定地基，[异步串门与共同记忆规划](../superpowers/specs/2026-07-09-social-visiting-design.md)（下称 SV）定产品；本文是**工程执行计划**：范围、三人分工、十周里程碑、发布就绪与验收。每条线内部仍走既定节奏：spec → 实现 → 验证（vitest / svelte-check / biome / clippy / rustfmt）→ PR → 当日合并。

## 0. 已确认决策（2026-07-09）

| 岔路口 | 决定 |
|---|---|
| 形态 | 桌面 App（现有 Tauri 应用）为产品本体 + pawbae.ai 官网；**不含手机 App**（v1.1 紧随，服务端本期全部铺好） |
| 发布 | 邀请制封闭内测 + 官网候补名单；串门需要成对用户，邀请链接天然成对拉新 |
| 节奏 | 约 10 周到封闭内测，含 P4-A/B/C 全量；P4-C 是内置缓冲，可滑出首发（符合 SV 的分阶段门槛） |
| 团队 | 含创始人在内 3 名全栈开发者，按功能线垂直切分 |
| 设计 | 严格参考 moneycoach.ai：系统字体 + 玻璃卡片 + 云朵配色（要点见 §5.3） |
| 域名 | pawbae.ai = 主域（官网 + 候补名单 + 更新清单 `latest.json`，更新器已硬编码此域名）；pawbae.app = 未来宠物主页，v1 仅 301 跳转到主域 |

## 1. v1 范围

**用户旅程一句话**：官网领邀请码 → 下载已签名的桌面 App → GitHub 登录 → 接入自己的 agent（CC / Codex / Cursor，已建成）→ 加好友 → 宠物去好友家串门 → 回家双方获得共同记忆。

| 包含 | 不包含（首发后排队） |
|---|---|
| 平台账号：GitHub OAuth、handle、可选 display_name | 手机 App（v1.1，本期把 Realtime/投影全部铺好） |
| 桌面 Connector：opt-in 上传 + 心跳，默认全关 | 宠物主页 pawbae.app/@handle |
| 好友关系：请求 / 接受 / 解除 / 静音 / 拉黑（P4-A） | 聊天、信件、PK、排行榜、广场、陌生人匹配 |
| 异步串门：30 分钟租约、双宠同屏、远程状态映射（P4-B） | 多访客、公共房间 |
| 共同记忆：模板键结算、双方日记、共同相册（P4-C） | 独立投喂系统（已融入串门的免费待客点心） |
| pawbae.ai 官网 + 候补名单 + 邀请码体系 | 邮箱登录、Sign in with Apple |
| 发布就绪：macOS 签名公证、更新器验签、崩溃上报 | Linux 包、MSI、Windows ARM64 |

两条红线全程有效：**隐私双层白名单门**（永不上传代码内容或自由文本；好友可见数据一律走独立 public projection，绝不放宽 `pets.snapshot` 的 RLS）与 **never-punish**（拒绝、召回、离线、错过都不掉任何东西，无每日任务/streak/亲密度进度条）。

## 2. 三条开发线

三人都偏全栈 → 按功能垂直切，每人端到端负责一条线（前端 + Rust + SQL 都碰），线间只通过契约（SQL 迁移 + RPC 签名 + `packages/shared` 的 TS 类型）交互。分配（2026-07-09 定）：**A = @zhihao-acc，B = @miaomeng1，C = Yining**。各线自包含交接文档：[A 云端线](../team/line-a-cloud.md) · [B 连接线](../team/line-b-connector.md) · [C 舞台线](../team/line-c-stage.md)。

### A · 云端线：Supabase 服务端 + shared 契约

- **PR-B 地基**：profiles / pets（含 `connector_seen_at` 心跳列）/ events + 事件字典 v1 + RLS + 本地栈迁移可重放 + keyless CI job。
- **好友域（P4-A 服务端）**：canonical 单行 friendship 表 + 方向性 blocks 表 + `is_blocked()` + SECURITY DEFINER RPC 全套状态转换 + 限速表。
- **访问域（P4-B 服务端）**：visits 租约状态机 RPC、public projection 表、私有 Broadcast 频道授权、pg_cron 过期作业、幂等键。
- **记忆域（P4-C 服务端）**：shared memory 结算 RPC（每次访问最多一条、重放安全）。
- **邀请码 + 候补名单表**；funnel 的 SQL 视图（内测指标看 Supabase，Aptabase 继续只管桌面产品遥测）。
- **W3-4 起草 P4 实施 spec**（SQL/API/频道契约——SV 是产品 spec，实施 spec 目前是空缺）。
- **RLS 完整矩阵测试是本线硬验收**（Alice/Bob/陌生人/被拉黑 × 访问前/中/后）。

### B · 连接线：桌面账号、数据管道、发布链

- **PR-A 单仓库改造**（W1 排他窗口，见 §4）。
- **桌面登录**：GitHub OAuth 桌面流（loopback/deep-link）+ 设置页「账号」区 + 逐项 opt-in 上传开关（默认全关，不登录 = 与今日行为完全一致）。
- **数据管道**：心跳上报；`agent-activity` 聚合信号 → 投影枚举（`idle/working/waiting/compacting/offline`）→ 云端发布；快照 sanitizer 复用 `stage-bridge.ts` 思路。
- **好友 UI（P4-A 客户端）**：好友列表、请求收发、静音/拉黑入口（玻璃卡片体系，见 §5.3）。
- **发布链**：macOS 签名+公证接线（release.yml 的注释块已就位）、更新器 minisign 验签、崩溃上报、Windows 签名、Intel Mac 更新槽位修复、版本号单源脚本。

### C · 舞台线：官网、串门体验、共同记忆

- **pawbae.ai 官网**：monorepo 内 `apps/website`（SvelteKit 静态导出，与团队技术栈一致），moneycoach 骨架 + 云朵品牌（§5.3）；候补名单表单 + 邀请码说明页；Vercel 托管（顺带承载更新清单 `/update/latest.json`）。
- **串门客户端（P4-B）**：租约状态机客户端（幂等键、`ends_at` 本地推导）、旅行过场、访客宠物渲染（`MiniPetMascot` + `SpritePet` 已是现成基座）、空窝状态牌、召回入口、断网/离线恢复。
- **远程状态映射**：SV §3.4 的映射表 → 双宠确定性互动（碰鼻子、并排坐、一起庆祝）。
- **共同记忆（P4-C 客户端）**：模板键 → 双方日记（`diary.ts` 加 `visit` moment kind，无需迁移）、记忆卡/共同相册 UI。

### 我（Claude）的角色

三条线各自的 Claude Code 会话继续现有节奏；我在主会话做跨线集成审查（契约一致性、隐私门、热点文件冲突）与每周里程碑对账。

## 3. 十周里程碑

| 周 | A 云端线 | B 连接线 | C 舞台线 | 里程碑 |
|---|---|---|---|---|
| W1 | Supabase 云项目 + 本地栈；PR-B 迁移初稿 | **PR-A 排他窗口**（目标 3 天内合并） | 官网设计稿 + 品牌素材 | 创始人：Apple Developer 注册（今天，审批要几天）、DNS 指向 Vercel |
| W2 | PR-B 落地（schema+RLS+字典+CI） | 更新器验签 + 崩溃上报 | 官网搭建 | **M1 地基完成** |
| W3-4 | 好友域 + P4 实施 spec + 候补名单表 | 桌面 GitHub 登录 + opt-in 设置 + 心跳 | 官网上线收候补；邀请码兑换 UI 设计 | **M2 桌面登录打通、官网上线** |
| W5-6 | 访问域（租约/投影/Broadcast/pg_cron） | 好友 UI + 投影发布管道 | 串门客户端：租约状态机 + 双宠渲染基座 + 空窝 | **M3 好友关系 e2e**（两个账号互加成功） |
| W7-8 | RLS 完整矩阵 + 邀请码 + 限速 | 发布链（签名产物、Windows 签名、Intel 修复） | 远程状态映射 + 双宠互动 + 召回/异常恢复 | **M4 串门 aha**（团队内部真实对测） |
| W9 | 记忆结算 RPC + funnel 视图 | 联调 + SV §12 异常矩阵测试 | 记忆卡 + 日记/相册集成 | **M5 共同记忆闭环** |
| W10 | 冻结修复 | 签名安装包出厂 | 首批邀请对 | **M6 封闭内测发布** 🚀 |

滑动规则：任何线落后先吃 W9-10 的缓冲；再不够就把 P4-C 滑出首发（SV 本来就规定 P4-B 验证复访后才丰富化）——**P4-B 串门是不可妥协的发布底线**。

## 4. 协作规则（防三线相撞）

代码库分析结论：三方并行的冲突点非常集中，用所有权规则化解。

- **PR-A 排他窗口**：W1 由 B 一次性完成移动 + 修全 CI。已知雷区：ci.yml 的 PR 路径过滤移动后不更新会**静默停跑 CI**；`biome.json` includes、vitest/vite/tsconfig 三处 `$lib` 路径、release.yml 的 `rust-cache workspaces` 与 tauri-action `projectPath` 全部要改；根 `pnpm-workspace.yaml` 目前不存在，需新建。验收 = 全基线照常通过 + `git log --follow` 可追历史。窗口期内 A/C 不开 PR（做云项目搭建与设计稿）。
- **热点文件所有权**（改动必须由 owner 合入或点头）：`pet.svelte.ts`、`src-tauri/src/lib.rs` invoke_handler、`commands/mod.rs` → B；`MascotView.svelte`、`Panel.svelte` → C；`packages/shared` 全部类型与字典 → A。`types.ts` 随平台工作逐步瘦身迁往 shared。
- **i18n JSON**：`en.json`/`zh.json` 每条线只在自己的 feature 前缀区块内追加，冲突全是 append 型，rebase 即解。
- **契约先行**：A 每个服务端能力先交「迁移 + RPC 签名 + shared TS 类型」，B/C 对着契约 + `supabase start` 本地栈开发，不等云端。
- **PR 纪律**：小步 PR、CI 全绿才合、不推 main（仓库规则强制）、Codex bot review 照常处理。

## 5. 三条线的技术基调（研究结论速查）

### 5.1 A：服务端模式（RLS 是全部安全边界，anon key 随二进制公开）

- friendship 单行表：`CHECK (user_a < user_b)` + 复合主键，重复/反向请求结构上不可能；拒绝/解除 = 直接 DELETE（不留 declined 墓碑，避免泄露与卡死重加）。
- blocks 方向性独立表，只有 blocker 可见；`is_blocked(a,b)` SECURITY DEFINER 双向检查，**每条社交策略都调它**（好友请求、邀请、Realtime、投影读取）；拉黑 RPC 内同事务删好友行。
- 状态转换全走 SECURITY DEFINER RPC（`REVOKE UPDATE`），转换校验 + 拉黑检查 + 限速在同一事务；所有 definer 函数钉死 `search_path`。
- 唯一活动访问：部分唯一索引 `on visits (visitor_id) where status in ('requested','accepted','traveling','visiting','returning')`——谓词必须覆盖 SV 状态机**全部未结束态**（状态机里没有 `active` 这个状态），且必须 immutable（用 status 列，不能用 `now()`）；host 侧同理（各状态是否计入 host 占用由 P4 实施 spec 精确定义）。pg_cron 把过期租约翻转到 `expired` 等终止态，RLS 读路径额外查 `now() < ends_at`，正确性不依赖 cron 准时。
- 幂等：`unique (actor_id, idempotency_key)` **按用户限定作用域**，insert-on-conflict 后回读旧结果。
- Realtime：投影走**私有 Broadcast 频道**、租约限定 topic `pet:{owner_id}:{lease_id}`，`realtime.messages` 上写 SELECT 策略（EXISTS 活跃租约 + 非拉黑）；**只允许数据库触发器发布**（客户端不可信）。关键认知：策略在订阅和 JWT 刷新时评估并缓存，删行不会踢人——**真正的撤销点在发送端**（发布触发器先查活跃租约，租约结束即静默），配短 JWT（5-10 分钟）兜底 + 礼貌性 `visit_ended` 广播。
- 限速：`rate_limits` 固定窗口计数表放在同一 definer RPC 里（换客户端绕不过）；pg_cron 每 30-60 秒翻转过期租约、每日清理旧窗口和陈旧 pending 请求（幂等键保留 ≥24h）。

### 5.2 B：发布就绪差距（按优先级）

| 级 | 差距 | 说明 |
|---|---|---|
| P0 | macOS 签名 + 公证 | Apple Developer $99/年，**审批要几天，第一天就注册**；release.yml 签名块已写好只待 6 个 secrets；hardened runtime 需加 `com.apple.security.device.audio-input` entitlement（语音功能）；上线后删掉 update.rs 里的 `xattr -cr` 免签 hack |
| P0 | 更新器验签 | 现状：HTTPS 裸信任，下载即执行。先给自定义清单加 minisign 签名字段并安装前校验（1 天，保留 `ui` 配置直发能力）；迁移 tauri-plugin-updater 排到内测后 |
| P1 | 崩溃上报 | 目前完全没有——内测崩溃无从知晓。Sentry（Rust panic hook + webview onerror）或最小自建上报 |
| P1 | Windows 签名 | Azure Trusted Signing ~$10/月（个人验证可用，Tauri 有 `signCommand` 集成）；首波邀请若全是开发者，可带安装说明先行 |
| P2 | Intel Mac 更新槽位 | 清单只有一个 `macos` 槽，Intel 用户会收到 arm64 DMG——加 `macos-x64` 槽或改universal binary |
| P2 | 发布自动化 | 版本号三文件手动同步易漂移 → 单源 bump 脚本；CI 自动生成清单 JSON（发布仍手动，作为放量闸门） |

### 5.3 C：设计语言（moneycoach 解码 → PawBae 化）

- 零自定义字体：系统字体栈（Apple 上即 SF Pro）+ 大号紧字距粗标题（`4xl→7xl`），这是最便宜的「苹果原生感」；个性全部交给宠物美术。
- moneycoach 的暗/亮条带节奏 → PawBae 改成**天空条带**：淡蓝/薰衣草云朵辉光渐变与白色交替；只保留一个深色段落——宠物在开发者深色终端旁发光的「agent 工作时刻」。
- 双层色板：slate/gray 承载全部文字与骨架，粉彩云朵色（天蓝/蜜桃/薄荷）只出现在渐变、圆角胶囊和宠物本体。可爱住在图像里，不住在正文色里。
- Hero 不放设备全家福，放**宠物趴在真实编码桌面上**的一张图——一眼证明「伴侣 + agent 监控」。
- 功能区块 = 截图/动画 + H2 + 三条加粗微标题（各配一句话）；短促句式文案：*「你的 agent。你的宠物。你的桌面。」「本地。私密。属于你。」*；点名 Claude Code / Codex / Cursor——具体性即高级感。
- 玻璃卡片体系（`rounded-2xl` 半透明白 + backdrop-blur + 低扩散阴影）站内站外共用：好友列表行、记忆卡都用它；头像圆胶囊 + 状态点（薄荷=工作中 / 蜜桃=空闲 / 灰=离线）。
- 记忆卡按「App Store 精选卡」做：一张主视觉 + 短句标题（*「一起，发布了。」*）+ 时间戳，收藏品能量。
- 串门的 UI 是「无 UI」：朋友的宠物走进桌面本身就是产品，界面只留一个玻璃 toast（「Mochi 来串门了 👋」）——宠物表演，面板噤声。
- CTA 免费优先（「下载 macOS 版」），顶部公告胶囊连 changelog（活着且在发货）；mega-footer 从第一天铺 SEO 深页（教程/对比/用例）。

## 6. 本次一并修复的跨文档矛盾（研究员审计结果）

已随本提交修入 P0/P1 文档：

1. `connector_seen_at` 心跳列补入 P1 schema（SV §5.4 硬性要求，原 P1 缺失）；
2. `profiles.display_name` 补入（SV §2.4 的公开展示名通道，原 P1 只有 handle）；
3. P1 中 CodeQL 迁移项删除（仓库根本没有 CodeQL workflow，PR-A 无需迁移）；
4. P1 明确：面向好友的数据必须剥离 `events.source` 等 agent 厂商信息，且一律走独立 projection，不放宽 `pets.snapshot` RLS；
5. 愿景 P4 从「三件套（好友/看宠物/投喂）」更新为 SV 的三切片（投喂融入待客点心）。

移交 **P4 实施 spec**（W3-4 由 A 起草）：投影表与「私有快照 → 公开投影」的白名单映射 sanitizer；社交字典扩展（visit/memory 模板键、时长档位、昼夜时段）与高频状态的短生命周期通道设计；限速从「风险注记」升级为带测试的交付物；邀请码体系。

## 7. 验收与内测门槛

- **工程验收**：P1 基线全绿（vitest 360+ / svelte-check 0 errors / biome / clippy / rustfmt）；**RLS 完整矩阵测试为硬门槛**；SV §12 测试地图全覆盖（状态机、恢复竞态、伪造拒绝）；最终验收 = **两位真实用户从互加好友到完成串门，全程无开发工具介入**（SV §12.4）。
- **产品门槛**（封闭 beta 决策线，SV §9）：好友请求接受 ≥ 40% · 好友首访发起 ≥ 35% · 接受访问完成 ≥ 70% · 7 天内同关系复访 ≥ 25% · 骚扰举报 < 1%。只完成一次不复访 → 修共同记忆的情感质量，不加奖励。

## 8. 风险

| 风险 | 缓解 |
|---|---|
| Apple Developer 审批拖慢签名 | 注册放在 W1 第一天（本计划最长前置项）；签名接线本身只要 1-2 天 |
| Realtime 撤销语义被误解（删行不踢人） | 发送端守门模式已定为标准（§5.1），RLS 矩阵测试覆盖访问前/中/后 |
| 三线在热点文件相撞 | §4 所有权规则 + 契约先行 + 小步 PR |
| 10 周含 P4-C 偏紧 | P4-C 即缓冲，可滑出首发；P4-B 是底线 |
| Supabase 免费档配额 | 封闭内测量级远够；到量升付费档，成本线性可见 |
| 「分享状态」滑向泄露 | 双层白名单 + 独立投影 + 字典构造器无自由文本位；任何新共享字段必须过 P4 实施 spec 评审 |

## 9. 创始人本周待办（只有你能做的）

1. **Apple Developer 注册**（$99/年，今天提交，审批数天）；
2. Porkbun 上把 pawbae.ai DNS 指向 Vercel（要做时我给逐步指引）；
3. （可选）Azure Trusted Signing 账号（~$10/月，Windows 签名）；
4. 物色 W10 首批对测的两对真实好友；
5. 把另外两位开发者拉进 repo，告诉我谁拿 A 谁拿 B，我给每条线出交接文档。
