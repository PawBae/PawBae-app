# PawBae Roadmap

> AI Agent 时代的开发者守护宠物。

## 代码库事实基线

当前 PawBae 是一个 Svelte 5 + Tauri 2 纯 SPA 桌面应用，核心能力已经不是从零开始：

- 宠物窗口：透明、置顶、可展开/收起的 mini window，已有 hover、拖拽、抛掷、物理运动、pet mode 大宠物窗口等基础。
- Agent 监控：已有 Claude Code、Codex、Cursor hook / socket / JSONL 解析链路，能聚合会话状态、工具调用、等待确认、token 与成本指标。
- 输入反馈：已有隐私开关、Rust 侧全局输入采集、`user-input` 批处理事件、前端一次性键盘/鼠标 reaction 动画。
- 养成系统：已有 hunger、affection、coins、daily gift、feed、pomodoro / agent completion / input milestone 奖励账本。
- 平台工程：已有 macOS / Windows 差异处理，包括 DPI、窗口坐标、全屏隐藏、local asset、hook 生成、更新机制和中英 i18n。

路线图的原则是复用这些已经落地的锚点，不重新设计一套并行架构。

## 愿景

现有 AI 工具大多在解决“怎么让 agent 更强”，PawBae 解决的是“开发者本人怎么样了”。它不是一个冷冰冰的 agent 状态面板，而是开发者和 AI agents 之间的情感缓冲层：把状态、成本、中断、焦虑和成就转译成一只轻量、可爱的桌面宠物。

## 行业背景

以下市场数字适合作为定位素材，但发布前需要补齐来源并复核日期：

- 开发者不完全信任 AI 输出，却越来越依赖它。
- 经验开发者在 AI 辅助下可能因为 prompt / review / context switching 变慢。
- 多 agent 并行、权限中断、token 浪费和通知疲劳正在变成真实工作流问题。
- 团队开始采用 multi-agent workflow，但缺少直觉化、低负担的全局状态视图。

## 竞品参考：Bongo Cat

Bongo Cat 验证了桌面宠物的几个关键留存点：

- 零摩擦陪伴：后台挂着，不打断工作流。
- 即时反馈：输入动作立刻换成视觉反馈。
- 收集驱动：饰品、稀有度、掉落和图鉴给长期目标。
- 社交扩散：多人同屏、皮肤和表情带来自传播。
- 持续进化：不断补乐器、成就、表情和互动道具。

PawBae 不应照搬按键计数，而是把 Bongo Cat 的即时反馈和收集循环，嫁接到开发者真实工作行为上。

## 差异化定位

| 维度 | Bongo Cat | PawBae |
| --- | --- | --- |
| 定位 | 通用桌面宠物 | 开发者专属 AI agent 伴侣 |
| 积分来源 | 按键计数 | Agent 完成、专注时长、输入里程碑、代码贡献 |
| 状态感知 | 无 | Claude Code / Cursor / Codex 实时监控 |
| 情绪表达 | 静态反馈 | 镜像认知负荷、等待确认、失败重试、token 消耗 |
| 养成系统 | 收集为主 | hunger / affection / coins / daily gift / feed |
| 平台 | Steam / Electron 生态 | Tauri 2 原生轻量桌面 |

## Phase 1：反馈循环

目标：建立已经被 Bongo Cat 验证过的核心爽点，同时让反馈来自更有意义的开发行为。

### 1.1 输入实时反馈动画

状态：已具备 MVP。

当前代码已有 Rust 侧输入采集、隐私开关、`user-input` 批处理事件、`reaction-machine` 和 `SpritePet` reaction row。下一步不是重做监听，而是扩展反馈表现。

- 键盘输入：播放 `react-keyboard`。
- 鼠标点击：播放 `react-mouse`。
- 忙碌保护：拖拽、抛掷、hover jump、摸头、设置面板打开时不打断当前动作。
- 验收：连续输入不会造成动画抖动；关闭隐私开关后不采集全局输入。

范围：`src-tauri/src/input/*`、`src/lib/components/MascotView.svelte`、`src/lib/utils/reaction-machine.ts`。

预估：小到中。

### 1.2 键盘控制宠物位置

状态：已完成（PR #25）。

用户在 PawBae 小窗获得焦点时，可以用 `W/A/S/D` 或方向键移动宠物位置，`Shift` 加速。该能力复用已有 `move_mini_by` Tauri command，因此继续继承 macOS / Windows 的坐标换算、屏幕边界 clamp、DPI 和菜单栏 / Dock 处理。

- `W` / `ArrowUp`：上移。
- `A` / `ArrowLeft`：左移。
- `S` / `ArrowDown`：下移。
- `D` / `ArrowRight`：右移。
- `Shift` + 方向：快速移动。
- 设置面板或会话面板展开时停用，避免影响列表、按钮和后续输入控件。
- 正在拖拽、下落、抛掷或反弹时停用，避免和物理状态机抢控制权。

后续增强：

- 长按方向时切换 walk / run 动画，而不是只移动窗口。
- 在 pet mode 中加入“键盘散步”小游戏，移动一定距离给少量 affection。
- 增加可选的 Vim 风格 `H/J/K/L` 或手柄方向键，但默认只保留 WASD / arrow keys，避免抢快捷键。

范围：`src/lib/components/MascotView.svelte`、`src/lib/utils/keyboard-control.ts`、`src/lib/stores/window.svelte.ts`、`src-tauri/src/commands/window/positioning.rs`。

预估：小。

### 1.3 积分来源扩展

状态：已具备基础账本，需要调权重和 UI 化。

当前 coins 已经不只来自 pomodoro，还能接 agent completion、focus input 和 input milestone。下一步重点是把“编码行为比无脑按键更有意义”写进默认权重和展示。

- Agent 完成任务：高积分。
- 连续专注时长：中积分。
- 累计输入里程碑：低积分。
- 喂食、每日礼物、番茄钟统一进入 reward ledger。

范围：`src/lib/stores/pet.svelte.ts`、`src/lib/utils/rewards.ts`、`Panel.svelte`。

预估：小。

### 1.4 成就 / 里程碑系统

状态：已完成 MVP（PR #26）。

24 个成就全部是对已持久化计数器（agent 完成、专注块、输入量、喂食、礼物、连续签到、陪伴天数、进化阶段）的纯谓词，无新增簿记，重复评估幂等。

- 解锁时宠物头顶弹出庆祝气泡（`CelebrationBubble.svelte`）。
- 持久化到 pet.json（unlock map：id → 时间戳）。
- Panel 宠物面板内置成就墙，隐藏成就显示 `???`。
- 后续：解锁专属动画、桌面通知可选项。

范围：`utils/achievements.ts`、`pet.svelte.ts`、`Panel.svelte`、i18n。

## Phase 2：Agent 情感化

目标：把冰冷的 agent 状态转化为直觉化的宠物行为，降低认知负荷。

### 2.1 Agent 焦虑可视化

宠物镜像的是开发者的认知负荷，而不是单个 agent 的内部状态。

- 同时运行 3+ agent：紧张 / 忙碌表情。
- Agent 连续失败或重试：皱眉、出汗。
- 所有 agent 空闲且开发者专注输入：安静陪伴。
- 长时间无操作：睡觉。

当前代码已有 `agentStore`、`sessionStore`、hook event processing、status 聚合和宠物状态映射。下一步需要补一个“工作负荷聚合器”，把 active sessions、waiting count、error count、tool running、最近完成数压缩成宠物情绪。

范围：`sessions.svelte.ts`、`agents.svelte.ts`、`MascotView.svelte`、宠物动画 manifest。

预估：中。

### 2.2 Token 花费感知化

Token 消耗转译成宠物饥饿度，把 FinOps 变成低认知成本的“喂食”隐喻。

- 每消耗 token，食物缓慢减少。
- 设置每日预算，超过阈值时宠物提醒。
- 便宜操作是小零食，贵操作是大餐。
- 只在本地展示，不上传成本信息。

当前代码已有 `AgentMetrics.totalTokens`、input / output / cache token 和 cost 字段，以及 hunger 系统。下一步是做 token delta，而不是用累计值反复扣 hunger。

范围：`src-tauri/src/commands/agent/metrics.rs`、`agents.svelte.ts`、`pet.svelte.ts`。

预估：中。

### 2.3 Agent 活动文字气泡

Agent 运行工具时，宠物头顶显示极短状态气泡。

- 读取文件：`正在看代码...`
- 编辑文件：`正在改代码...`
- 运行测试：`正在跑测试...`
- 等待确认：`需要你看一眼`

当前代码已经有 `currentTool`、`recentActions`、`toolInput` 类信息来源，前端也有 `VoiceBubble`。建议新建独立 `AgentBubble.svelte`，避免和语音输入状态混在一起。

范围：`AgentBubble.svelte`、`MascotView.svelte`、`agents.svelte.ts`、i18n。

预估：小。

## Phase 3：心流守护

目标：解决权限中断和通知疲劳，保护开发者深度工作。

### 3.1 心流保护模式

检测持续编码状态，智能延迟非紧急通知。

- 连续输入超过 5 分钟、无明显切换窗口：进入心流。
- 非紧急 agent 完成提醒进入队列。
- 用户停下后汇总播报：`刚才 3 个 agent 完成了，1 个需要你看看`。
- 等待权限、错误、预算超限仍可升级为紧急提醒。

当前代码已有输入事件、agent complete、waiting sound 和 auto expand 设置。下一步是增加 flow-state detector 和 notification queue。

范围：新 flow store、reward input tracker、completion popup / sound path。

预估：大。

### 3.2 Permission Fatigue 缓解

把权限审批变成“宠物帮你过滤低风险打扰”。

- 低风险只轻提示：读文件、列目录、运行安全命令。
- 高风险强提示：写 `.env`、删文件、改 shell 配置、联网安装依赖。
- 学习用户审批习惯，但必须可解释、可撤销。

这项涉及 agent 权限协议，必须在 hook / socket 层先明确事件格式，不能只做前端动画。

范围：`socket.rs`、hook event schema、设置面板、风险规则表。

预估：大。

### 3.3 多 Agent 任务总管

宠物是你的 agent 牧羊犬，把并行进程管理变成视觉叙事。

- 每个运行 agent 显示为一个轻量小图标。
- Agent 完成：图标收起，宠物叼回结果。
- Agent 卡住：宠物跑去推它。
- Agent 冲突：两个 agent 改同一文件时给出视觉冲突提示。

当前 repo 已经有本地和 OpenClaw 连接概念，适合先做只读可视化，再做冲突检测。

范围：新 agent visual layer、文件触碰摘要、宠物行为状态机扩展。

预估：大。

## Phase 4：收集系统

目标：借鉴 Bongo Cat 的收集驱动，给用户长期目标。

### 4.1 饰品 / 帽子框架

- 数据结构：`id`、`name`、`rarity`、`slot`、`asset`、`unlockRule`。
- 渲染层叠加在 `SpritePet.svelte` 上。
- 解锁逻辑：积分兑换、成就奖励、定时掉落。
- 需要按宠物 atlas 做锚点，避免帽子在不同角色上漂移。

范围：`types.ts`、`SpritePet.svelte`、新 collections store、资产规范。

预估：大。

### 4.2 收集图鉴面板

- 按稀有度分类显示全部饰品。
- 已解锁 / 未解锁状态。
- 佩戴管理：帽子、配件、背景小物。
- 图鉴只在用户主动打开时展示，不占用默认 mini 面板。

范围：新 Panel 子页面、i18n、Tauri store。

预估：中。

### 4.3 编码贡献可视化

对抗“我还是程序员吗”的身份焦虑，强调用户是指挥官，不是旁观者。

- 今日做了哪些决策。
- 审查了哪些代码。
- 纠正了 agent 几次。
- 哪些 commit 是人主导，哪些是 agent 辅助。

这项应谨慎做本地 git 分析，不要自动上传仓库信息。

范围：git 集成、贡献摘要 store、新 UI 面板。

预估：中。

## Phase 5：体验打磨

目标：补齐发布体验。

- Gaming Mode：锁定宠物位置、禁用交互、仅显示动画；全屏应用检测时自动进入。
- 开机自启：接入 Tauri autostart 插件，设置面板开关。
- 动画丰富化：打哈欠、伸懒腰、看鼠标、时间段状态、键盘散步。
- Windows 对齐：补齐 macOS efficiency hover、notch 感知、Speech Recognition 的等价体验。
- 测试补全：Vitest 覆盖 stores / utils；Rust 测试覆盖 session 解析、hook 生成、状态转换。
- i18n 完善：中 / 英 / 日等语言补齐。

## Phase 6：成长与世界

目标：给用户一个长期留下来的理由——进化是收集系统的"屋顶"，世界是社交与传播层。

### 6.1 进化系统

状态：已完成 MVP（PR #26）。

- XP = 终身赚取金币总和（来自 reward ledger totals，单调且已持久化；花费不扣 XP，喂食永远不会让宠物退化）。
- 五阶段：新生 → 幼苗（60）→ 少年（300）→ 大师（1200）→ 传说（4000）。阈值按"重度编码日 ≈ 250–300 XP、纯宠物日 ≈ 60 XP"校准。
- 工作风格分支（少年阶段起）：XP 主要来源决定分支——指挥官（agent 完成）/ 禅修者（专注+番茄钟）/ 贴心伙伴（养成行为）。分支以光环色调呈现。
- 进化瞬间：径向闪光 + 进化气泡；跨多级只庆祝实际到达的阶段。
- 后续：每阶段专属形态差分（贴纸/配件层）、进化分享卡片（PNG 导出）、与实时 agent 负载挂钩的临时"超载形态"。

### 6.2 每日礼物与连续签到

状态：已完成 MVP（PR #26）。

- `claimDailyGift` 首次获得 UI 入口（此前 store 已实现但无按钮）。
- 连续签到加成：50 → 80 金币，7 天封顶；断签从 1 重新开始，宠物不惩罚（拓麻歌子式死亡 = 流失）。
- 面板显示 🔥 连续天数与"在一起的第 N 天"。

### 6.3 网页世界（未来）

分三级火箭，DAU 验证留存后推进：

1. 宠物名片页 + GitHub README 徽章（成本最低的开发者病毒循环，只需 opt-in 上传聚合数据）。
2. 星露谷风格小镇广场：在线用户宠物同屏闲逛、表情互动、串门投喂（互相浇水式日活钩子）。
3. 团队牧场（to B）：团队宠物住同一牧场，晨会一眼看出谁的 agent 卡了、谁刚 ship。

### 6.4 开发事件彩蛋（未来）

测试由红转绿开香槟、检测到 force push 炸毛、凌晨三点端咖啡——每个都是免费的社交媒体素材。依赖新的事件源（git 状态、测试结果），需要 Rust 侧扩展。

## 推荐实施路线

```text
Phase 1: 反馈循环 + WASD 控制
  |
  +--> Phase 2: Agent 情感化
  |       |
  |       +--> Phase 3: 心流守护
  |
  +--> Phase 4: 收集系统
  |       |
  |       +--> Phase 5: 发布打磨
  |
  +--> Phase 6: 成长与世界（进化/签到已落地，世界待验证留存后推进）
```

优先级建议：

1. 先完成 Phase 1 的输入反馈、WASD 控制、coins 权重和成就 MVP，让用户每天都能感到宠物“活着”。
2. 再做 Phase 2 的 agent 情绪聚合和活动气泡，把 PawBae 和普通桌宠拉开差距。
3. Phase 3 不急着做自动审批，先做心流通知队列，避免碰权限安全边界太早。
4. Phase 4 的收集系统依赖稳定积分来源，应在 reward ledger 稳定后推进。
5. Phase 5 贯穿每个阶段，尤其是 Windows 和测试，不要等发布前集中补。

## 设计原则

- 陪伴优先，功能其次：宠物的首要价值是情感陪伴，不是信息密度。
- 减少认知负荷：每个新功能都应该让开发者更轻松，而不是多一个要管理的东西。
- 有意义的反馈大于无脑计数：积分来自真实编码行为，按键只能做低权重补充。
- 渐进式复杂度：新用户只看到一只可爱的宠物，深度用户才进入 agent 监控和心流保护。
- 轻量原生：继续发挥 Tauri 2 体积和性能优势，不把宠物做成重型 dashboard。
- 本地优先和隐私默认：输入、token、git、agent 状态默认只在本机处理。
