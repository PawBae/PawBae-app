# 交接文档 · C 舞台线（负责人：Yining）

日期：2026-07-09 · 你的领地：`apps/website/`（全新目录）+ `apps/desktop/` 的串门体验与记忆 UI、`MascotView.svelte`/`Panel.svelte`/`settings/SettingsPanel.svelte` · 你不碰：`supabase/` SQL（A 线）、登录/上传管道/发布链（B 线）

> 路径约定：桌面侧相对路径均指 PR-A 改造后 `apps/desktop/src/...` 下的对应位置。

## 0. 一分钟上下文

v1 = 平台账号 + 好友 + 异步串门 + 共同记忆，三人十周到封闭内测。你负责用户能「看见和感动」的一切：pawbae.ai 官网、宠物离家串门的完整体验、共同记忆卡。串门是整个 v1 的 aha moment 和不可妥协的发布底线——这条线是产品灵魂所在，也是创始人品味的用武之地。

必读：[v1 发布计划](../strategy/2026-07-09-v1-release-plan.md)（§5.3 设计语言是你的美术基调）、[串门规划 SV](../superpowers/specs/2026-07-09-social-visiting-design.md)（§3 核心循环与 §6 异常规则是你的产品剧本）、[B 线文档](line-b-connector.md) §2（PlatformClient 接口——你的 mock 要实现它）。

## 1. 使命与边界

**使命**：让「朋友的宠物出现在我桌面上」感觉像交到朋友，而不是多了个 NPC。SV §15 的一句话验收由你守门：*可拒绝、可召回、无损失、无分身，且让用户想让两只宠物再次见面。*

**红线**：串门 UI 是「无 UI」——宠物表演，面板噤声（一个玻璃 toast，不开大窗口）；never-punish 在文案层也生效（无责备文案、无缺席焦虑、无「还差 N 次」）；记忆卡渲染只吃模板键 + 安全参数，文案在客户端按 locale 生成。

**明确不负责**：登录与好友列表 UI（B 线做组件，你只合挂载行）、一切 SQL 与服务端。

## 2. 逐周交付

### W1 —— 设计与创始人事务（排他窗口，不开 PR）

- [ ] **Apple Developer 注册**（$99/年，审批数天，今天提交——全计划最长前置项，B 的 W7 等它）
- [ ] Vercel 先行、DNS 后切：先建 Vercel 项目并托管 `/update/latest.json`（照抄当前线上清单）、验证可达，**然后再**把 pawbae.ai DNS 指过去——更新器硬编码这个 URL，顺序反了老用户的更新检查会 404；pawbae.app 设 301 跳转
- [ ] 建 GitHub OAuth App（PawBae org 下），client id/secret 填进 A 的 Supabase Auth（B 的 W3 登录等它）
- [ ] 官网设计稿：hero =「宠物趴在真实编码桌面上」一图证明「伴侣 + agent 监控」；moneycoach 骨架 + 天空条带（详见发布计划 §5.3 全部 9 条）
- [ ] 串门体验分镜：离家过场 → 空窝状态牌 → 好友家双宠 → 归家 → 记忆卡（对着 SV §3 画）

### W2 —— 官网脚手架（M1 周）

> 前置：**PR-A 已合并**（`apps/website/` 目录与 `pnpm-workspace.yaml` 由它创建）；若未合并，从 PR-A 分支切出先建，PR stacked 其上。

- [ ] `apps/website/`：SvelteKit + static adapter（与团队栈一致），Tailwind，系统字体栈零 webfont
- [ ] 官网站点接进 W1 已就绪的 Vercel 项目（`/update/latest.json` 已在 W1 迁好并验证）

### W3-4 —— 官网上线 + 邀请入口（M2）

- [ ] 首页全量：hero、三条微标题功能区块、隐私区块（「本地。私密。属于你。」——把双层白名单讲成卖点）、FAQ、mega-footer SEO 深页骨架
- [ ] 候补名单表单：开发期可用 stub，但**公开上线那一刻必须真实收集**——A 承诺 `waitlist` 表 W3 内交付，W4 上线直接接 supabase-js insert；若 A 延误，改用可导出的表单服务顶上，**禁止 no-op 上线**（静默丢真实报名是最贵的事故）
- [ ] App 内 onboarding 邀请码输入 UI 设计（兑换走 `PlatformClient.redeemInvite`；A 的 RPC 与 B 的实现都在 W5-6 就绪，你 W7 接线）
- [ ] **合入 B 的 `AccountSection` 挂载行**（进 `settings/SettingsPanel.svelte`，你是 owner）——M2「桌面登录打通」必须从真实 UI 走通，本周内合，不攒

### W5-6 —— 串门客户端，mock 驱动（M3 周）

**先写 `MockPlatformClient`**（实现 B 线 §2 的接口，放 `apps/desktop/src/lib/platform/mock.ts`）：脚本化的租约生命周期 + 投影状态序列（`idle→working→waiting→…`），可加速时间、可注入断网/过期/召回剧本。此后到 W7 你完全不需要真实后端。

- [ ] 租约状态机客户端：`endsAt`（契约字段；数据库列才叫 `ends_at`）本地推导到期（不依赖必达的结束消息）、幂等键生成、reconnect 收敛
- [ ] 离家/归家过场 + 空窝状态牌（「去 Momo 家玩了」+ 召回入口；托盘保留设置/审批入口）
- [ ] `GuestPet.svelte`：基于现成的 `MiniPetMascot.svelte` + `SpritePet.svelte`（props 驱动、无 store 依赖，直接可用）；访客保留自己的名字/皮肤/主人归属
- [ ] 双宠同屏进 `MascotView.svelte`（你自己的 owner 文件，随便改）
- [ ] **合入 B 的 `FriendsPanel` 挂载行**（进 `Panel.svelte`）——M3「好友 e2e」等它，本周内合

### W7-8 —— 状态映射 + 真实联通（M4：团队内部真实对测）

- [ ] SV §3.4 映射表全量动画：idle=一起玩耍 / working=小帐篷 / waiting=叼纸条张望 / compacting=整理云朵 / offline=客房睡觉；completion=一起庆祝**由客户端从 working→idle 的投影转变推导**（它不是 ProjectionStatus 枚举值，契约里没有单独的 moment 通道）
- [ ] 确定性双宠互动：碰鼻子、并排坐、追纸飞机、一起庆祝（用 `physics/state-machine.ts`，纯函数可单测）
- [ ] 待客三动作：摸摸 / 合照 / 免费点心（零数值变化）
- [ ] **换线**：`MockPlatformClient` → B 的真实实现（接口相同，理论上是一行 DI 切换）；异常剧本重放一遍（挂载行已分别在 W3-4 / W5-6 合入，本周没有攒下的集成债）

### W9 —— 共同记忆（M5）

- [ ] `diary.ts` 加 `visit` moment kind（它的 sanitizer 本来就保留未知 kind，无迁移负担）
- [ ] 记忆卡：App Store 精选卡式——一张主视觉 + 短句标题（「一起，发布了。」）+ 时间戳；共同相册网格
- [ ] 中英文案全量走查：像宠物回忆，不像审计日志（SV §12.4 验收项）

### W10 —— 发布周（M6）

- [ ] 组织两对真实好友对测（全程无开发工具，SV §12.4）；官网放邀请说明；funnel 指标核对（A 的 SQL 视图）

## 3. 独立开发策略

- 官网是 greenfield 目录，与桌面代码零交集；候补名单用 stub 起跑。
- 串门开发的解耦支点是 **MockPlatformClient**：接口文件是 B 的 **W3 第一项交付**（interface-only，你 review 后合并即冻结），距你 W5 开写 mock 还有两周余量；冻结后你的 W5-8 与服务端进度脱钩，mock 的异常剧本还能测到真实环境很难复现的竞态。
- 你是 `MascotView.svelte`/`Panel.svelte`/`settings/SettingsPanel.svelte` 的 owner：B 的组件以新文件交付、你在**承诺周内**合挂载行（W3-4 AccountSection、W5-6 FriendsPanel——M2/M3 里程碑等它们），三线 PR 互不冲突。
- W9 记忆卡不等 A：模板键 fixture 载荷随 A 的 W8 进 shared，你的渲染与中英文案走查全程对 fixture 进行。
- i18n 只在你的前缀区块（`visit.*`、`memory.*`、`website` 独立）追加。

## 4. 工作流规范

- 不可直推 main；小步 PR、CI 全绿、Codex review 照常。
- 验证：`pnpm test` / `pnpm check` / `pnpm lint`；改 `MascotView` 后手动跑 `pnpm tauri dev` 走一遍宠物基础行为（喂食/漫步/气泡）不回归。
- 每周里程碑对账时，你同时戴产品验收帽：M2 官网、M4 串门 aha、M6 内测发布是你拍板的三道门。
