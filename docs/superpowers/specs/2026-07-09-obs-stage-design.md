# OBS 直播舞台 v1 设计

> 阶段二最后一个 S 级项。战略依据：Neuro-sama 单靠人格月入 $40 万+，vibe coding 主播就是目标用户——宠物上了直播画面，每一场直播都是获客素材（人格即流量、主播即渠道）。
> 关联：`docs/strategy/2026-07-07-startup-strategy.md` §4 阶段二；`docs/superpowers/specs/2026-07-09-pet-diary-design.md`（镜像文案与气泡同源）。

## 0. 用户决策记录（2026-07-09）

| 岔路口 | 决定 |
|---|---|
| 捕获方案 | **绿幕舞台窗**：独立小窗纯色背景（绿/蓝/品红三选防皮肤撞色），不置顶可被遮挡，OBS 窗口捕获 + 色度键。localhost 浏览器源（真 alpha）留作 v2 升级路线 |
| 舞台内容 | **镜像宠物 + 气泡**：主窗口的哑渲染镜像——皮肤/动画/朝向/气泡文案实时同步；主窗口仍是唯一大脑，舞台零副作用 |
| 开关入口 | **设置→显示**：开关 + 背景色三选，配置持久化，重启自动恢复 |

## 1. 玩法规则

- **舞台窗口**：标题 `PawBae Stage`（OBS 窗口列表里好认），label `stage`，加载 `index.html#/stage`。
  - 默认 480×270（16:9），可自由拉伸，宠物居中、尺寸随窗口自适应（`min(w,h)` 比例缩放）。
  - **无边框**（`decorations(false)`）——捕获画面即纯色幕布+宠物，OBS 不用裁标题栏；整窗背景是拖拽区，边缘可拉伸（Tauri 无边框窗口原生支持）。
  - **不置顶**（区别于主窗口）、创建时不抢焦点、不跳任务栏（任务栏/Dock 有入口，埋住了也找得回）——刻意可以埋在 IDE/浏览器后面：OBS 的窗口捕获（macOS ScreenCaptureKit / Windows WGC）对被完全遮挡的窗口照样采集，主播单屏也能用。
  - 桌宠照常在桌面活动，舞台只是镜像，互不干扰。开关舞台的唯一入口是设置开关（无边框窗没有关闭按钮）；若窗口被系统途径关掉，设置开关同步回落（防御性同步）。
- **背景色**：绿 `#00FF00` / 蓝 `#0000FF` / 品红 `#FF00FF` 三选，默认绿。皮肤本体含绿时主播换蓝或品红。背景色随快照下发热更新，不重建窗口。
- **镜像内容**：皮肤（含自定义皮肤）、基础动画状态（idle/working/走路原地踏步等）、朝向、反应 sprite、当前气泡的**最终渲染文案**（agent 状态、庆祝、早安问候、审批便签提示）。v1 宠物居中不在舞台内漫游（VTuber overlay 常态），舞台内漫游留作后续。
- **永不打扰红线不破**：舞台窗口纯输出、无任何交互元素；不抢焦点、不置顶、不发通知。
- **持久化**：settings.json 新增 `stream_stage_enabled`（bool，默认 false）、`stream_stage_bg`（`green|blue|magenta`，默认 green）。开着舞台退出，下次启动自动恢复。

## 2. UI

### 2.1 设置→显示（DisplaySection）

- 「OBS 直播舞台」开关 + 背景色三选（开关开启时才显示色选）。
- 一句教学文案：OBS 里添加「窗口捕获」选 PawBae Stage，再加「色度键」滤镜选对应颜色即可。

### 2.2 StageApp（舞台端）

- `main.ts` 按 `location.hash` 以 `#/stage` 分流，挂载极简 `StageApp.svelte`（不挂 Main 树）。
- 纯色背景铺满 + 居中 `MiniPetMascot`（现成 props 驱动渲染器，`suppressHover`）+ 顶部一条通用气泡（样式沿用 CelebrationBubble 的视觉语言，允许换行）。
- 文案由主窗口渲染好随快照下发，舞台端零 i18n、零 store 副作用。

## 3. 架构

### 3.1 镜像协议（关键决策：主窗口是唯一大脑）

- 舞台 webview **不跑 petStore、不写任何持久化**——从根上避免双重计数/双重持久化竞态（#48 暖香、#49 日记的同款教训）。
- **快照下行**：MascotView 在舞台开启时用 `$effect` 构造渲染快照，经 `emit('stage-state', snapshot)` 全窗口广播；快照去重（序列化相等不重发）。
- **握手上行**：StageApp 挂载后 `emit('stage-ready')`，主窗口收到即推当前快照（覆盖舞台后开/刷新场景）。
- 快照结构（`utils/stage-bridge.ts` 定义）：`{ petId, baseState, facing, reactionSprite, bubble: { kind, text } | null, bg }`——背景色也随快照走，单通道无独立配置事件。

### 3.2 分层

- **`src/lib/utils/stage-bridge.ts`**（新，纯逻辑，vitest 全覆盖）：快照类型、`buildStageSnapshot(...)` 构造、`snapshotsEqual(a, b)` 去重、`sanitizeStageBg(raw)` 背景色校验回退。
- **Rust（lifecycle.rs + lib.rs 注册）**：`open_stage_window` / `close_stage_window` 两个命令（有 `spawn_demo_mascot` 先例）；创建参数：`resizable(true)`、`decorations(false)`、`transparent(false)`、`always_on_top(false)`、`focused(false)`、`skip_taskbar(false)`。本次动 Rust，clippy/rustfmt 必跑。
- **settingsStore**：`streamStageEnabled` / `streamStageBg` 两个持久化字段；toggle 时 invoke 开/关命令 + track 遥测；启动时若 enabled 自动开舞台。
- **MascotView**：舞台开启时的快照 `$effect`（收敛在一处，复用它已有的气泡/皮肤/动画状态）。
- **StageApp.svelte**（新）+ `main.ts` hash 分流 + DisplaySection 开关 UI。
- 舞台端皮肤解析：StageApp 只读复用 skinsStore 的 resolve（加载皮肤清单是只读操作，无持久化写入）。

## 4. Lore、i18n、遥测

- **Lore canon 第 13 条**（`docs/lore/yoonie.md`）：云上居民都爱看戏。Yoonie 有块小舞台，站上去就知道有人看着——有观众的时候，干活格外来劲。
- **i18n**：`stage.*`（开关名、教学文案、背景色标签），en+zh 各写一套，不逐字互译。舞台窗口本身零文案（气泡文案随快照下发）。
- **遥测**（补进 `docs/superpowers/specs/2026-07-08-telemetry-aptabase.md` 字典）：`stream_stage_toggled { on: on/off }`——主播获客渠道假设是否成立的最早信号。

## 5. 测试计划

- vitest：stage-bridge 每条规则——快照构造、去重（相同不重发/任一字段变化重发）、背景色校验回退。
- 回归：vitest 全量、svelte-check、biome；**本次动 Rust：cargo clippy + rustfmt**。
- 自我验收（dev 真机）：开舞台 → 第二窗口渲染镜像宠物；模拟 agent 完工 → 舞台气泡同步；切背景色 → 热更新；换皮肤 → 舞台跟随；重启 → 舞台自动恢复；关开关 → 窗口销毁；系统途径关窗 → 设置开关同步回落。

## 6. 风险与后续

- **色度键色边**：纯色背景 + 抗锯齿边缘在色度键下可能有轻微色边——教学文案给推荐滤镜参数；根治方案是 v2 的 localhost 浏览器源（真 alpha，OBS 标准 overlay 工作流）。
- **皮肤撞色**：绿皮肤撞绿幕——三色可选已覆盖；极端撞色皮肤由主播自行换色。
- 后续候选：舞台内漫游（宠物在舞台里踱步）、状态徽标（working/idle + 今日完工数）、localhost 浏览器源、舞台布局预设（全身/半身特写）。
