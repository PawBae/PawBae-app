# 每日任务板 + 宽容连胜 — 设计

**日期:** 2026-07-08
**背景:** 战略 30 天清单第 4 项后半（叼来审批单已完成）。风险表点名：Travel Frog 式新鲜感流失、陪伴 App D3<20% —— 解法是「照护倒置 + 宽容连胜」。本功能把两者接上。

## 用户决策（2026-07-08 敲定）

1. **任务 = 自动判定的固定任务集** —— 零输入负担，从现有信号自动打勾。用户手写 todo 是另一个独立功能（Drop-to-Do [M]），不在此。
2. **宽容 = 自动护盾** —— 每连续打卡 7 天自动 +1 盾（上限 2），断天自动消耗，零操作。不卖钱不焦虑化，纯送。
3. **统一连胜** —— 任务板连胜吸收现有礼物连胜；全局只有一条连胜，streak_3/7/30 成就与礼物加成无缝迁移。
4. **打卡 = 完成任意 1 项**；4 项全勤 → 额外奖励。
5. **顺手修 UTC 日界线**：`todayStr()` 从 UTC 改为本地日期（原来 PDT 用户「新的一天」在下午 4-5 点开始）。

## 任务集（v1 固定 4 项）

| id | 任务 | 判定信号（埋点位） |
|---|---|---|
| `gift` | 🎁 领今日礼物 | `claimDailyGift` 成功 |
| `headpat` | 🤚 摸摸头 | `applyHeadpat` |
| `meal` | 🍖 让宠物吃一顿 | `consumeMeal`（手动喂食与 agent 带餐共用的私有入口） |
| `agent` | 🤖 陪 agent 完成 1 个任务 | `claude-task-complete` 且非 waiting |

刻意不放审批响应——agent 不等审批的日子该任务不可能完成，挂「今天做不到」的格子违背宽容原则。任务必须任何一天都可由用户主动达成（agent 项是核心循环的例外，写代码的日子自然完成）。

## 连胜规则

- **打卡**：当日第一项任务完成的瞬间续连胜。
- **续接**：昨天打过卡 → streak+1。断 N 天 → N ≤ 持有盾数则消耗 N 盾原地续上（streak+1）；盾不够则**安静地从 1 重来，盾保留**（没救成这次，留着救下次——绝不双重惩罚）。
- **铸盾**：打卡后 streak 是 7 的倍数 → +1 盾，上限 2。
- **时钟回拨**：streak 不动、不重置（防御性 clamp，照 approval-note 先例）。
- **全勤**：4 项全完成 → +15 金币（新账本来源 `task_board`）+ 一次庆祝动画（复用 GrowthCelebration 队列，新 kind `perfect_day`）。
- **礼物照旧**：50 + 5×(streak-1) 金币，7 天封顶 80——只是 streak 现在读统一连胜。
- **永不惩罚**：断连胜没有难过表情、没有挽回弹窗；显示归零就是全部后果。

## 实现形态（照抄成就系统 / approval-note 模式）

- **纯 reducer** `src/lib/utils/daily-board.ts`：`BoardState {boardDate, boardDone, streak, streakDate, shields}` + `markTask(state, taskId, today)` 返回 `{state, taskCompleted, checkedIn, shieldsSpent, shieldEarned, perfectDay}`；另有 `displayStreak(state, today)`（连胜仍可被盾救活时照常显示，否则 0）。日期翻转、去重、护盾、时钟回拨全在 reducer 内，全部单测。
- **PetData 新增** 5 字段（boardDate/boardDone/streak/streakDate/shields）。**迁移**：hydrate 时若无 `streakDate` 字段，用 `currentGiftStreak(lastDailyGift, today, giftStreak)` 播种。旧字段 `giftStreak`/`lastDailyGift` 保留：lastDailyGift 继续记领奖日（防重复领取），giftStreak 冻结不再推进。
- **store 胶水** `markBoardTask(task)`：跑 reducer → 写回 petData → perfectDay 发金币+庆祝 → checkedIn 发遥测。埋点四处各一行。
- **成就**：`AchievementContext.giftStreak` 改喂统一连胜（字段随之更名 `streak`）；streak_3/7/30 定义不动、ID 不变。
- **CoinSource** 增 `'task_board'`。
- **UI**：Panel 成就区上方新增「📋 今日任务」分区：4 行勾选态、🔥×连胜、🛡️×盾数；en+zh 文案。
- **遥测**（进事件字典）：`board_checkin {streak_bucket: "1-2"|"3-6"|"7-29"|"30+"}`、`board_perfect_day`（无属性）。匿名分桶正好交叉验证 D1/D7/D30。
- **UTC 修复**：`todayStr()` 改本地日期。`yesterdayOf`/`nextGiftStreak` 等做的是抽象日期字符串运算，不受影响。一次性副作用：切换当天每日重置点位移，礼物可能可再领一次——无害。

## 非目标（v1）

- 不做任务自定义、不做补签、不做护盾商店。
- 不做连胜里程碑弹窗（streak_3/7/30 成就已覆盖）。
- 不改番茄钟/专注等其他每日候选任务——4 项跑通留存数据后再扩。
