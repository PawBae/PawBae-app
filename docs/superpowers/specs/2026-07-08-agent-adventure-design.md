# Agent 冒险 + 纪念品架 — Design

**Date:** 2026-07-08
**Feature:** 长任务时宠物背包去冒险，完工带回随机纪念品；纪念品累积成图鉴架。
**Strategy:** 阶段一最后一个功能项「Agent 冒险 + 纪念品架 [M]」——Travel Frog 的
「期待感」循环（3000 万下载验证：期待感 > 义务感）。

## 用户决策（AskUserQuestion，三问三答）

1. **触发方式** → 长任务自动触发：某会话持续忙碌 ≥ 3 分钟 → 自动出发；该任务真完成 → 回来带纪念品。
2. **离开视觉** → 走出屏幕留标记：播 run-left 走出 → 原位留 ⛺「去冒险了」→ 完成时走回 + 🎁。
3. **纪念品目录** → 精选图鉴 24 件：3 档稀有度，未获得剪影 + ???，重复获得 ×N。

## 架构核心：资格与视觉解耦（方案 A，已确认）

- **资格计时机**（纯逻辑）：每个会话从首次进入忙碌态计时；真完成时若已 ≥ 3 分钟 → 掉纪念品。
- **「不在家」是展示层派生状态**：有跑满 3 分钟的忙碌会话、且没有更高优先级的事时，宠物才离开。
- 关键性质：**视觉被打断永远不丢纪念品**。审批单把她叫回家送纸条，任务后来完成了，纪念品照发。
- 边界自动正确：ESC / 会话被杀 → Rust 不发完成事件 → 无掉落，宠物安静回家，零内疚（永不惩罚）。

## Canon 先行（追加 docs/lore/yoonie.md 第 9 条）

> 任务跑得久的时候，她会背上小包回云上转一圈。云上没有商店，只有交换——
> 她总能用几声捡来的「叮」换回点小玩意儿。

纪念品全部是她从云上换回来的东西；风味文案全部长在云端来客 canon 上。

## 纪念品全目录（24 件 · 单一来源，i18n 落地 en/zh）

### 常见 ×12（Common）

| id | emoji | 中文名 · 风味 | English name · flavor |
|----|-------|--------------|----------------------|
| cloud_fluff | 🌫️ | 一撮云绒 · 她说枕着睡特别香，就是容易打喷嚏。 | A pinch of cloud fluff · Great for napping on. Slightly sneezy. |
| ding_echo | 🔔 | 一声「叮」的回声 · 装在耳朵里带回来的，摇一摇还能响半声。 | An echo of a ding · Carried home in her ear; shake it for half a ring. |
| rounded_pebble | 🪨 | 云打磨过的小石子 · 在云上滚了很多年，圆得没有脾气。 | A cloud-polished pebble · Rolled around up there for years; round beyond all argument. |
| steam_candy | 🍬 | 蒸汽软糖 · 用完工任务的暖香凝的，闻着饱，吃不着。 | A steam gumdrop · Condensed from the scent of finished work; filling to smell, impossible to eat. |
| mist_ribbon | 🎀 | 一段雾做的缎带 · 打不了结，但飘起来很好看。 | A ribbon of mist · Refuses to hold a knot. Floats beautifully though. |
| star_crumbs | ✨ | 星星碎屑 · 云上居民扫地扫出来的，说是不值钱，她还是捡了一小把。 | Star crumbs · Swept up by the cloud folk, who insisted they were worthless. She took a handful anyway. |
| rain_seed | 💧 | 一粒雨的种子 · 埋在花盆里长不出雨，她还是想试试。 | A rain seed · Won't actually grow rain in a flowerpot. She wants to try anyway. |
| stray_feather | 🪶 | 不知道是谁的羽毛 · 云上没有鸟，所以这根羽毛很有讨论价值。 | Somebody's feather · There are no birds on the cloud, which makes this one highly discussable. |
| tiny_ladder | 🪜 | 迷你云梯 · 只有三格，够一只很小的东西爬一段很小的高度。 | A miniature cloud ladder · Three rungs: enough for something very small to climb something very low. |
| washed_note | 📃 | 一张旧纸条 · 上面的字被雨洗掉了，她坚持说写的是好话。 | An old note · The rain washed the words away. She insists they were kind ones. |
| wind_knot | 🌀 | 一个风打的结 · 风路过时随手打的，解开就没有了，所以不能解。 | A knot tied by the wind · Undo it and it's gone. So it must never be undone. |
| warm_button | 🔘 | 晒暖的纽扣 · 不知道是谁的，但握在手里一直是温的。 | A sun-warmed button · Owner unknown. Stays warm in the paw regardless. |

### 稀有 ×8（Rare）

| id | emoji | 中文名 · 风味 | English name · flavor |
|----|-------|--------------|----------------------|
| rain_smell_jar | 🫙 | 雨前的味道 · 装在小瓶里，开盖只能闻一次。她建议留给很难的日子。 | The smell of before-the-rain · Bottled. One sniff, ever — save it for a hard day. |
| moon_shaving | 🌙 | 月亮刨花 · 月亮每个月瘦下去的那部分。云上居民收了她三声「叮」。 | A moon shaving · The part the moon loses each month. Cost her three dings. |
| dud_thunder | ⚡ | 没炸响的小雷 · 哑火的。抱着睡会轻轻震，像打呼。 | A little thunder that never went off · A dud. Hums softly when hugged, like a snore. |
| cloud_bell | 🛎️ | 云铃铛 · 只在没有人听的时候响，所以没人能证明它不响。 | A cloud bell · Only rings when nobody's listening — so no one can prove it doesn't. |
| fog_marble | 🔮 | 雾芯玻璃珠 · 里面卷着一小团雾，天气要变时会自己转圈。 | A fog-core marble · A wisp of fog curled inside; spins on its own before the weather turns. |
| sky_postcard | 💌 | 天空寄给自己的明信片 · 收件人和寄件人都是天空，邮票是一小块晚霞。 | A postcard the sky mailed to itself · Sender and recipient: both "the sky." The stamp is a scrap of sunset. |
| dream_cotton | ☁️ | 梦的棉花 · 从睡着的云身上轻轻摘的。据说塞进枕头能多做一个梦。 | Dream cotton · Gently picked from a sleeping cloud. One extra dream per pillow, allegedly. |
| ding_whistle | 🎐 | 会吹「叮」的哨子 · 吹出来的不是哨声是叮声。云上工作狂们的最爱。 | A whistle that dings · Plays dings instead of whistles. A favorite among the cloud's workaholics. |

### 传说 ×4（Legendary）

| id | emoji | 中文名 · 风味 | English name · flavor |
|----|-------|--------------|----------------------|
| whole_kiwi | 🥝 | 一整颗猕猴桃 · 云上居民公认睡得最久、绒毛长得最好的那朵。她抱回来的路上谁都没敢碰。 | An entire kiwi · The cloud folk agree: the longest-sleeping, best-fuzzed cloud of all. Nobody dared touch it on the way home. |
| first_ding | 🏮 | 世界上第一声「叮」 · 封在一盏琥珀色的小灯里。之后所有的「叮」都是它的孩子。 | The world's first ding · Sealed in a small amber lamp. Every ding since is one of its children. |
| cloud_key | 🗝️ | 云的钥匙 · 打不开任何门，因为云上没有门。但拿着它，去哪儿都算回家。 | The key to the cloud · Opens nothing — clouds have no doors. But carry it, and anywhere counts as home. |
| bottled_aurora | 🌈 | 瓶装极光 · 极光冬天路过云顶时留下的一绺，晚上会自己变颜色。 | Bottled aurora · A lock left behind when the aurora passed the cloud's roof. Changes color by itself at night. |

## 掉落规则

- 概率随行程加长变好（期待感变现）：行程 < 10 分钟 → 常见 78% / 稀有 19% / 传说 3%；
  ≥ 10 分钟 → 60% / 32% / 8%。同稀有度内均匀。
- `rollSouvenir(elapsedMs, rand)` 调用方注入随机数 → 测试可确定。
- 重复获得 = `count + 1`（×N 角标）；`firstAt` 保留首次时间。
- 多会话并发各自计时，各自掉落（按 sessionId 天然去重；真实工作不设上限）。

## 架构（三层）

### ① 纯逻辑 `src/lib/utils/adventure.ts`（新，带测试）

```ts
export const ADVENTURE_MIN_MS = 180_000; // 3 分钟
export interface AdventureState { pending: Map<string, number>; } // sessionId → 首见忙碌 epoch ms
export function initialAdventureState(): AdventureState;
/** busyIds = processing|tool_running|compacting；aliveIds = 会话列表全量。
 *  新忙碌会话打点；从列表消失的删除（waiting 保留——等审批是同一个任务的一部分）。
 *  返回 away：是否存在当前忙碌且已跑满 minMs 的行程。 */
export function stepAdventure(s, busyIds, aliveIds, now): { away: boolean };
/** 真完成时消费该会话的行程；返回耗时（clamp ≥0）或 null（没有行程记录）。 */
export function consumeTrip(s, sessionId, now): number | null;
```

### ② 纯逻辑 `src/lib/utils/souvenirs.ts`（新，带测试）

`SOUVENIR_CATALOG`（24 件 `{id, emoji, rarity}`，名字/文案走 i18n）、`LONG_TRIP_MS`、
`rollSouvenir`、`addSouvenir`（返回新 null-prototype record）、`sanitizeSouvenirs`
（hydrate 消毒：count ≥1 整数、firstAt 有限正数，未知 id 保留向前兼容，`__proto__` 无害）。

### ③ 数据层 `pet.svelte.ts`

- `souvenirs = $state<Record<string, SouvenirOwned>>({})`，持久化 pet.json 新 key `souvenirs`。
- 冒险机私有实例 + `adventureAway = $state(false)`；`stepAdventure(busy, alive, now)` 入口供 MascotView 调用。
- `handleTaskComplete` 真完成分支：`consumeTrip` → `≥ ADVENTURE_MIN_MS` 则 roll →
  `addSouvenir` → celebrations 入队 `{kind:'souvenir', id}` → `track('souvenir_found', {rarity})` → 立即持久化。

### ④ UI

- **types.ts**：`GrowthCelebration` 增 `{ kind: 'souvenir'; id: string }`。
- **CelebrationBubble**：souvenir 分支 → `🎁 {emoji} {名字}`（3.2s 节拍复用）。
- **MascotView**：busyKey/aliveKey 折叠轮询数组身份（waitingKey 先例，含 bare-statement 教训）
  + 10s 补步 interval（阈值跨越没有集合变化，需要时间驱动）；
  `awayNow` 派生 = coding 模式 && adventureAway && 无 waiting 会话 && 无 celebration
  && currentAction ≠ eat && 设置面板关 && 无语音活动 && 物理静止；
  四相 `tripPhase: home|departing|away|returning`（1.1s 过渡计时器）；
  departing/returning 播 run-left/run-right（行缺失则纯淡出）+ CSS translateX；
  away 相隐藏 MiniPetMascot，原位渲染 ⛺ + 「去冒险了」小字（在原 hitbox 区域内，右键开面板不受影响）；
  reduced-motion 关动画。
- **stroll.ts**：`StrollGateInput` 增 `away`；away → `{runLoop:false, pushStrollMode:false}`
  （出门期间原生漫步同步停，窗口停在原地给 ⛺）。
- **Panel**：成就区正下方新增纪念品架：标题 `🎒 纪念品架 7/24`，格子复用 ach-grid 样式；
  已获得 = emoji + ×N 角标 + tooltip（名字 — 文案），未获得 = ❓ + '???'（不剧透 emoji）；
  稀有度 tile 边框着色（稀有=矢车菊蓝 / 传说=金）；全空时显示一行提示语（发现性）。
- **DEV 钩子**（import.meta.env.DEV 剥离）：`__pawbaeAdventureDemo()` 强制 away 数秒后
  发一次 4 分钟行程掉落——不用等真实 3 分钟任务也能手测视觉链路。

### i18n 新 key（en/zh）

`adventure.awayNote`（去冒险了）、`adventure.shelfTitle`（纪念品架）、
`adventure.shelfHint`（空架提示：任务跑久一点，她就会背上小包去冒险——回来总会捎点什么。）、
`souvenir.<id>.name` + `souvenir.<id>.flavor` ×24（上表全文落地）。

## 遥测字典 +1

`souvenir_found | rarity（仅稀有度，不带具体 id）| 冒险循环是否转起来的直接信号`

## 错误处理

- 时钟回拨 → 耗时 clamp ≥ 0（审批单先例）；非有限 now → step 空转。
- pet.json `souvenirs` 被手改坏 → sanitize 丢弃坏条目，未知 id 保留。
- 会话中途被杀/ESC → Rust 不发 claude-task-complete → 行程无声作废（永不惩罚）。
- App 重启丢计时 → 首见时间从重启后重建，行程偏短——可接受（宁短勿假）。

## 测试

- `adventure.test.ts`：打点/waiting 保留/消失删除/away 阈值/consumeTrip 消费与未知 id/时钟回拨。
- `souvenirs.test.ts`：两张概率表边界、同稀有度均匀、addSouvenir 计数、消毒（含 `__proto__`）。
- `pet-souvenir.test.ts`（store 胶水）：种行程 → 真完成 → 图鉴+庆祝+持久化；waiting 完成不掉落；
  未跑满不掉落。
- `stroll.test.ts` 适配 away 输入。

## 明确不做（YAGNI）

- 不做冒险日记/明信片（阶段二宠物日记的地盘）、不做行程进度条/倒计时。
- 纪念品不可出售/消耗/交易——不碰金币经济。
- 全部宠物共用同一目录（图鉴属于产品不属于皮肤）；不做按皮肤专属掉落。
- 不做 Rust 侧改动（离屏移动复用不需要——DOM 层隐藏即可）。
