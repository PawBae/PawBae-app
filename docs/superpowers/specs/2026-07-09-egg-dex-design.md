# 孵蛋与物种图鉴 v1 设计

> 阶段二第二项。战略依据：Claude Buddy 有 18 物种，内置竞争必须正面防御；13 只现成内置图集零新增美术即可变成收集玩法；金币经济目前只进不出（完工 +20、每日礼 +50+、喂食仅 -5），孵蛋补上第一个真消耗端。
> 关联：`docs/strategy/2026-07-07-startup-strategy.md` §4 阶段二；`docs/superpowers/specs/2026-07-09-skin-workshop-design.md`（画廊/图鉴同址）。

## 0. 用户决策记录（2026-07-09）

| 岔路口 | 决定 |
|---|---|
| 门槛模型 | **孵蛋解锁内置邻居**：12 只非 Yoonie 内置变成收集目标，未认识 = 图鉴剪影不可切换；Yoonie 与自定义皮肤（UGC 红线）永远自由 |
| 蛋经济 | **买蛋 + 暖香孵化**：金币买蛋，靠"完工暖香"焐热破壳（照护倒置再加一环）；长冒险小概率直接带蛋回来 |
| 图鉴 UI | **图鉴 = 工坊画廊升级**：剪影 tile 融入现有画廊，蛋与孵化进度放 Panel（纪念品架模式），零新页面 |

## 1. 玩法规则

- **物种池**：内置清单减 `DEFAULT_PET_ID`（yoonie）= 12 只「云上邻居」。池是数据驱动的（从 builtin 清单推导）——将来 IP 角色移出默认包（皮肤工坊 spec §10）时池自动收缩，无需改逻辑。自定义皮肤永不入池、永不上锁。
- **买蛋**：`EGG_COST_COINS = 150`（可调）。同时只能有一颗蛋；全部认识后购买入口关闭（"都认识了"）。金币不足按钮禁用。
- **孵化**：蛋需 `EGG_HATCH_WARMTH = 8` 点暖香破壳。暖香来源：agent 真完工 +1（复用 handleTaskComplete 的真完工判定——ESC/killed 会话不算，与喂食/冒险同门），喂食 +1（pet 模式用户的唯一路径，喂食本身 -5 金币）。暖香封顶于阈值。
- **破壳揭晓**：暖香焐够后蛋进入待揭晓状态，**点击才破壳**（保住拆盲盒瞬间，不在用户不在场时静默孵化）。点击时从「未认识」池 roll（注入式随机，纪念品同款）→ 记入已认识 → 自动切换到新邻居出场 → 图鉴点亮。**每颗蛋保证新面孔**（roll 永远只在未认识池内，无重复、无保底需求）。
- **冒险惊喜**：长冒险（≥10 min，复用 `LONG_TRIP_MS`）在**无蛋在孵且仍有未认识邻居**时，`ADVENTURE_EGG_CHANCE = 10%` 概率带回免费蛋（暖香 0）**替代**纪念品掉落。
- **永不惩罚**：认识状态永不回收；蛋永不腐坏、不过期；启动时若当前 `miniPetId` 是未认识的内置邻居（老用户迁移场景），自动记为已认识——绝不没收正在用的宠物。
- **揭晓防御**：点击揭晓时若未认识池已空（迁移等边角在购买后补记了最后一位）→ 退还蛋价并清蛋，不报错。

## 2. UI

### 2.1 工坊画廊 = 图鉴（SkinWorkshopModal）

- 未认识的内置邻居：**剪影 tile**——沿用 `tileFrameStyle` 首帧裁切，CSS `filter: brightness(0)` + 降透明度（零新美术），名字显示 "???"，点击不切换、只抖动一下 + 提示文案。
- 已认识 / Yoonie / 自定义：行为不变（点击即切换）。
- 标题栏加「已认识 x/13」计数（yoonie 恒计入分子分母；自定义皮肤不计入）。

### 2.2 Panel 蛋区块（纪念品架同款手风琴）

- 无蛋：`🥚 买蛋 150🪙` 按钮（金币不足 / 全认识 → 禁用 + 原因文案）。
- 在孵：蛋 tile + 暖香进度 x/8（进度条）。
- 焐够：tile 发光脉动，点击 → 破壳动画（CSS）→ 揭晓新邻居（名字 + 首帧）→ 自动切换。

## 3. 架构（无 Rust 改动）

- **`src/lib/utils/eggs.ts`**（新，纯逻辑，vitest 全覆盖）：常量（`EGG_COST_COINS` / `EGG_HATCH_WARMTH` / `ADVENTURE_EGG_CHANCE`）、`hatchablePool(builtinIds)`（减 yoonie）、`unmetNeighbors(pool, met)`、`rollNeighbor(unmet, rand)`（注入熵）、`addWarmth(egg)`（封顶）、`shouldDropEgg(elapsedMs, egg, unmetCount, rand)`（长冒险掉蛋门）。
- **`petStore`（pet.svelte.ts）**：新增持久化状态 `metNeighbors: string[]`（store key `met_neighbors`）与 `egg: { warmth: number; since: number } | null`（store key `egg`）；`buyEgg()`（扣币 + 记账，走现有 ledger 模式）、暖香挂钩（handleTaskComplete 与 feed 各 +1）、`revealEgg()`（roll + met + 清蛋 + 返回新邻居 id）。待揭晓 = `warmth >= EGG_HATCH_WARMTH`（派生，不另存字段）。
- **冒险挂钩**：handleTaskComplete 的纪念品 roll 处，命中掉蛋门时以免费蛋替代 `addSouvenir`。
- **迁移**：petStore init 时检查 `settingsStore.miniPetId`——是内置非 yoonie 且未认识 → 补记认识。
- **SkinWorkshopModal**：tile 增加 locked 态（剪影 + "???" + 抖动）；标题计数。
- **Panel**：蛋区块（手风琴）+ 破壳揭晓内联 UI。

## 4. Lore、i18n、遥测

- **Lore canon 第 11 条**（`docs/lore/yoonie.md`）：云上的邻居讲究「闻香识门」——想请一位还没见过的邻居，Yoonie 先带回一颗云蛋（邻居寄放的到访信物）；用完工的暖香把它焐热，香味攒够了，邻居就顺着香找到家门。
- **i18n**：`egg.*`（买蛋/在孵/暖香进度/待揭晓/揭晓标题/都认识了/金币不足）+ `dex.*`（计数/锁定提示），en + zh。
- **遥测**（补进 `docs/superpowers/specs/2026-07-08-telemetry-aptabase.md` 字典）：`egg_bought {}`、`egg_hatched {species}`（内置 id，非用户内容）、`dex_completed {}`。

## 5. 测试计划

- vitest：eggs.ts 每条规则（池推导减 yoonie、roll 只出未认识且注入熵可复现、暖香封顶、掉蛋门的三个前置条件、池空行为）+ petStore 蛋状态持久化往返与 buyEgg 扣币。
- 回归：vitest 全量、svelte-check、biome（无 Rust 改动，clippy 跳过）。
- 自我验收（dev 探针）：买蛋 → 完工/喂食焐蛋 → 破壳揭晓 → 剪影点亮 → 自动切换 → 冷重启持久化 → 迁移场景（settings 预置未认识内置 id）→ 全认识后购买禁用。

## 6. 风险与后续

- **端游戏（12 只集齐）后金币再度无消耗**——接受；后续付费皮肤包 / 在线皮肤索引会持续扩池（池是数据驱动的）。
- **IP 角色在池内**（naruto/nezuko/wukong/doro）：本 PR 不动内置列表；Steam 前移出默认包时池自动收缩（见 §1）。
- 后续候选：孵化中的蛋在桌面上有实体（MascotView 蛋精灵）、按邻居性格定制破壳动画、图鉴条目双语小传（pet.json 增加可选 `intro` 字段，创作者规范同步）。
