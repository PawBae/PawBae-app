# 宠物日记 + 早安问候 v1 设计

> 阶段二第三项。战略依据：模板生成、零推理成本的"记忆"是最强依恋杠杆——几个月的共同工作史抄不走（护城河第二块砖）；Finch 的自我关怀日志验证了"被记住"的留存力。
> 关联：`docs/strategy/2026-07-07-startup-strategy.md` §4 阶段二、§7 护城河；`docs/superpowers/specs/2026-07-09-egg-dex-design.md`（时刻事件同源）。

## 0. 用户决策记录（2026-07-09）

| 岔路口 | 决定 |
|---|---|
| 内容模型 | **日结一篇 + 特殊时刻**：每天一段模板日结（完工/吃饭/金币计数），大事件单独成行；存结构化事件而非文本，切语言整本重渲染 |
| 问候形态 | **跨天首见气泡**：每天第一次见面按时段问候 + 昨日一句摘要；走 celebration 气泡通道，自动消失、零打扰、不可点击 |
| 日记 UI | **日记本 modal + Panel 入口**：Panel 加 📖 入口，独立 modal 按天倒序分组展示 |

## 1. 玩法规则

- **日结条目**：当天计数器 `{date, agentTasks, meals, coinsEarned}` 在跨天后第一次结算时折成一条日结。
  - `agentTasks`：真完工（`waiting === false`）且**与金币奖励同门**——走 `awardAgentStop` 的 60s 会话冷却，被去重的重复 Stop 事件不计数（复用孵蛋暖香的 Codex 教训，同一个 `stopAwards.length > 0` 门）。
  - `meals`：手动喂食与 token 餐两条路径都汇于 `consumeMeal`，在该点 +1。
  - `coinsEarned`：`commitCoins` 里正向 award 求和累加（花费不计，口径与进化 XP 一致）。
  - 全零天不写日结（自然留白，不刷"安静"）。跨多天时只有最后一个有计数的日期有条目（中间天计数器本来就是空的）。
- **时刻条目**（即时 append，与 celebration 入队同点位）：
  - `egg_found`（冒险带蛋回家）、`egg_hatched {species}`、`dex_completed`
  - `evolution {stageIndex}`、`achievement {id}`、`perfect_day`
  - `souvenir {id}` —— **仅稀有及以上**（rare/legendary）；普通纪念品频率太高会刷屏
  - `adopted` —— 领养日。新安装首次 hydrate 写入；老用户迁移按 `firstMeetAt` 的日期补记一条（缺失才补，幂等）。
- **早安问候**：持久化 `last_greet_date`。本地日期 ≠ lastGreetDate 时的第一次检查触发。**第一次检查由 store 在 hydration 完成后自行发起**（MascotView 挂载时 hydrate 仍在途，其立即检查会被 hydrated 守卫吞掉且无响应式重试——冷启动不能等 30s 定时器，Codex review）；MascotView 的 30s tick 负责接住开着跨零点的场景：
  1. 先结算日结（昨天的摘要才存在）；
  2. 再入队 greeting 气泡：`dayPartFor(hour)` 选时段文案（morning/day/evening/night 四套），昨天有日结则附一句摘要（"昨天我们完工 7 次！"），无日结则纯问候。
  - App 跨零点开着也适用：凌晨触发 night 文案（"这么晚还在忙"）。规则统一：每个本地日历日恰好一次。
- **持久化上限**：`diary` cap **500** 条，超限 FIFO 裁最老，但 `kind === 'adopted'` 永不裁（记忆护城河的第一块砖）。估算 60KB 封顶。
- **永不惩罚**：日记只增不减（cap 裁剪除外）；没有"漏写日记"的负反馈；问候永远是正面文案。

## 2. UI

### 2.1 DiaryModal（新组件）

- 复用皮肤工坊/周报卡的 modal 模式（遮罩 + 卡片 + 关闭按钮）。
- 按天倒序分组：天标题（"7 月 9 日 · 今天/昨天"相对标注）→ 该天的日结段 + 时刻行（emoji 前缀）。
- 日结模板 3–4 个句式变体，以日期字符串做种子伪随机选择（可复现——同一天每次打开看到同一句）。
- 空态："日记本还空着"引导文案。
- 时刻渲染缺字典条目时跳过该行（向前兼容：旧版本读到新 kind 不崩）。

### 2.2 Panel 入口

- pet-panel 里加 📖 日记按钮（纪念品架区块附近），点击开 DiaryModal。

### 2.3 问候气泡

- `CelebrationBubble` 新增 greeting 分支：样式允许换行（现有气泡 `nowrap`，问候句偏长），仍是 `pointer-events: none` 装饰层，沿用 3.2s 播放 beat。

## 3. 架构（无 Rust 改动）

- **`src/lib/utils/diary.ts`**（新，纯逻辑，vitest 全覆盖）：
  - 类型：`DiaryEntry`（`daySummary | moment` 判别联合，`at` 时间戳 + `day` 本地日期串）、`DiaryDayCounters`。
  - `bumpCounter(counters, key, today)`：跨天自动换新计数器。
  - `settleDiaryDay(counters, today)`：昨天（或更早）的计数器折日结；全零返回空。
  - `appendDiary(diary, entry)`：cap 500 裁剪，`adopted` 免裁。
  - `greetingFor(dayPart, yesterdaySummary, streak)`：返回 `{key, params}` 结构，不返回文本。
  - `summaryVariant(day)`：日期种子选句式变体序号。
  - `sanitizeDiary(raw)` / `sanitizeDiaryDay(raw)`：损坏数据回退。
- **petStore（pet.svelte.ts）**：新持久化 `diary: DiaryEntry[]`（key `diary`）、`diaryDay`（key `diary_day`）、`lastGreetDate`（key `last_greet_date`）；时刻挂钩放在现有 celebration 入队点旁；`greetDailyCheck()` 由 MascotView 的慢 tick 调用（date 变化即触发结算+问候）。
- **GrowthCelebration** union 加 `{ kind: 'greeting'; part: DayPart; tasks: number }`（渲染所需最小参数内联，避免 bubble 反查 store）。
- **MascotView**：慢 tick 处调 `petStore.greetDailyCheck()`（幂等，同日重复调用无操作）。
- **DiaryModal.svelte** 新组件；**Panel.svelte** 加入口。

## 4. Lore、i18n、遥测

- **Lore canon 第 12 条**（`docs/lore/yoonie.md`）：云上的邻居都有一本"香册"，记每天闻过的暖香；Yoonie 落地后把香册改成了日记——怕忘了跟你一起干过的活。
- **i18n**：`diary.*`（日结句式变体、时刻文案、天标注、空态、入口）+ `greet.*`（四时段 × 有/无昨日摘要），en+zh 各写一套，不逐字互译。
- **遥测**（补进 `docs/superpowers/specs/2026-07-08-telemetry-aptabase.md` 字典）：`diary_opened {}`。问候自动发生、无行为信息，不加事件。

## 5. 测试计划

- vitest：diary.ts 每条规则——计数器跨天换新、日结折算（含跳多天/全零天）、cap 裁剪保 adopted、greeting 的时段×摘要矩阵、变体种子可复现、sanitizers 损坏回退。
- 回归：vitest 全量、svelte-check、biome（无 Rust 改动，本地 clippy 跳过）。
- 自我验收（dev 探针，备份/恢复真实数据）：真实事件焐计数 → 探针改写持久化的 `diary_day.date` / `last_greet_date` 模拟跨天（不动系统时钟）→ 验证日结生成、问候气泡按时段出现、DiaryModal 渲染分组、冷重启持久化、adopted 补记幂等。

## 6. 风险与后续

- **模板文案质量是灵魂**——干巴了就成流水账。宠物第一人称、口语化，en/zh 分别打磨；句式变体防千篇一律。
- **cap 500 之外的远期记忆丢失**——后续候选"月度回忆压缩"（老条目折成月度一段），v1 不做。
- 后续候选：日记页分享导出（复用周报卡 canvas 路径）、按性格分支定制日结口吻、日记搜索。
