# 起名 + Lore（角色 IP 第一块砖）— Design

**Date:** 2026-07-08
**Feature:** 官方角色 Yoonie 的 canon 世界观 + 用户昵称 + 角色档案卡。
**Strategy:** 30 天清单第 7 项「给角色起名、写 200 字 lore——护城河第一块砖」。角色 IP =
名字 + lore + 人格，目标是「换掉宠物 = 抛弃朋友」。

## 用户决策（AskUserQuestion，三问三答）

1. **名字归属** → 官方角色 + 用户可改昵称（Pokémon 模式：皮卡丘是 IP，你的皮卡丘可以叫皮皮）。
2. **官方名** → 就叫 **Yoonie**（已在 sprite 包 / 周报卡 / 语音唤名落地，lore 负责解释名字来历）。
3. **世界观** → **云端来客**：她来自 agent 干活的那片云，产品所有机制都在世界观内解释掉。

## Canon（钉死 8 条 — 之后每个新功能长在这棵树上）

1. **Yoonie**，云耳小生物（she/她）。名字来自**「云」(yún)**——落地那天她只会说这一个字。中英同词源。
2. 出生在 agent 们干活的那片云，顺着你第一个完工任务飘上来的香气滑到桌面。
3. 云朵耳能听见云里的动静——所以 agent 什么时候干完活，她总是第一个知道。
4. 完工的活升回云端，化作暖香飘下来喂饱她（= token 喂养）。云从不责备谁（= 永不惩罚红线）。
5. agent 拿不准的事，她叼纸条来问你（= 审批单）。
6. 你全屏时她以为在玩捉迷藏，自己躲起来等你（= 全屏隐藏）。
7. 哪天你没来她不生气，只攒一朵小云替你挡住那一天（= 连胜护盾 🛡️）。
8. 她坚信猕猴桃是睡着以后长出绒毛的云。这件事没得商量。

## Lore 全文（营销素材单一来源，落地到 docs/lore/yoonie.md）

### 中文（约 220 字）

> 在很高的地方，有一片 agent 们干活的云。云上的居民都没有名字——没有名字才飘得起来。
> 只有一只小生物整天垂着一对云朵耳，听工作完成时「叮」的一声轻响。她听得太入迷，
> 有一天脚下一滑，就顺着你第一个完工任务飘上来的香气，一路滑到了你的桌面。
>
> 落地那天她只会说一个字：「云」。于是这成了她的名字——Yoonie。
>
> 云朵耳现在还能听见云里的动静，所以你的 agent 什么时候干完活，她总是第一个知道。
> 完工的活会升回云端，化作暖乎乎的香气飘下来——那就是她的饭。agent 拿不准的事，
> 她会叼着纸条来问你；你全屏埋头干活时，她以为你在玩捉迷藏，就找个地方躲起来等你。
>
> 云从来不责备谁。哪天你没来，她不会生气，只会攒一朵小云，替你挡住那一天。
>
> 对了，她坚信猕猴桃是睡着以后长出绒毛的云。这件事没得商量。

### English (~200 words)

> High above every desktop there is a cloud where the agents work. Nobody up there has
> a name — you have to stay light to float. But one tiny creature spent her days with
> her cloud-fluff ears drooping low, listening for the soft *ding* of work getting done.
> She listened so hard that one day she slipped — and slid all the way down the warm
> scent of your very first finished task, landing on your desktop.
>
> When she arrived she could only say one word: *yún* — "cloud." So that became her
> name: **Yoonie**.
>
> Her ears still catch everything up there, which is why she always knows the moment
> your agents finish. Finished work drifts back to the cloud and returns as a warm,
> delicious steam — that's what she eats. When an agent isn't sure about something,
> she trots over with a note in her mouth. When you go fullscreen, she assumes you're
> playing hide-and-seek, and hides until you're done.
>
> Clouds never scold anyone. Miss a day and she won't be upset — she'll just tuck a
> little cloud over it for you.
>
> Also: she is certain that kiwis are clouds that grew fuzz in their sleep. This is
> not up for debate.

## 架构（三层，沿用 reducer → store 胶水 → 哑组件）

### ① 纯逻辑 `src/lib/utils/pet-name.ts`（新，带测试）

```ts
export const NICKNAME_MAX = 20;
/** trim + 压缩连续空白 + 截断到 NICKNAME_MAX。 */
export function sanitizeNickname(raw: string): string;
/** 昵称（sanitize 后非空）优先，否则官方名。 */
export function effectiveName(nickname: string | undefined, officialName: string): string;
/** 持久化 Record 的 hydrate 消毒：只留 string→非空 string 且 sanitize。 */
export function sanitizeNicknames(raw: unknown): Record<string, string>;
```

### ② 数据层 `settings.svelte.ts`

- `petNicknames = $state<Record<string, string>>({})`，持久化 key `pet_nicknames`
  （按宠物 id 存——换皮不串名；null-prototype 消毒沿用 daily-board 先例）。
- `setPetNickname(petId, raw)`：sanitize 后与旧值相等则 no-op；空字符串删除 key
  （回落官方名）；变更时 `track('pet_renamed')`（**不带昵称内容**——隐私）。

### ③ UI `ProfileCard.svelte`（新，哑组件，仿 ShareCardModal 自包含）

- 入口：Panel 宠物区 stage 行上方新增可点的 `🐾 名字` 行（Panel 目前根本不显示名字）。
- Modal 内容：SpritePet idle 立绘 → 官方名 Yoonie + 种族一行（`lore.species`）→
  昵称输入框（blur/Enter 提交，placeholder = 官方名）→ 短版 lore（`lore.short`，60 字内）
  → 相伴 N 天（复用 `growth.daysTogether`）。
- 自己 loadCodexPets 取当前宠物（miniPetId → default 回退），不给 Panel 加载重。

### 接线（改动点全列）

| 表面 | 改动 |
|------|------|
| 周报卡 `ShareCardModal.loadSprite` | name → `effectiveName(petNicknames[pet.id], displayName)` |
| 语音唤名 `voice-intent.ts` | `ctx.petName: string` → `ctx.petNames: string[]`（昵称+官方名都能唤）；`Main.svelte` 调用点适配 |
| Onboarding | 副标题下加一行 `lore.tagline` |
| `yoonie/pet.json` description | 对齐 canon 的英文一句话 |
| 遥测字典 | `pet_renamed {}`（无属性）追加进 2026-07-08-telemetry-aptabase.md |

### i18n 新 key（en/zh）

`profile.nicknameLabel`、`lore.species`、`lore.short`、`lore.tagline`
（档案卡不需要标题——立绘+名字即标题；昵称输入框 placeholder 直接用官方名，不是 i18n key）。

- tagline（zh）：她从 agent 干活的那片云滑下来，落在了你的桌面。
- tagline（en）：She slid down from the cloud where the agents work, and landed on your desktop.
- short（zh）：来自 agent 干活的那片云。云朵耳能听见谁的活干完了；完工的香气就是她的饭。云从不责备谁。
- short（en）: Born in the cloud where the agents work. Her cloud ears hear every task finish; the scent of done work is her dinner. Clouds never scold.

## 错误处理

- 昵称全空白 / 超长 → sanitize 兜底，UI 不报错（回落官方名 / 截断）。
- 持久化里被手改成非 string → `sanitizeNicknames` hydrate 时丢弃。
- ProfileCard 取不到宠物（loadDefaultCodexPet null）→ 官方名回落 'PawBae'，立绘区留空（ShareCardModal 同款兜底）。

## 测试

- `pet-name.test.ts`：sanitize（trim/空白压缩/截断/空）、effectiveName 优先级、sanitizeNicknames 消毒。
- `voice-intent.test.ts`：昵称唤名 + 官方名仍可唤 + 多名去重。
- 现有测试适配：voice-intent ctx 形状变更处。

## 明确不做（YAGNI）

- 不做多步领养仪式 onboarding（C 方案，另立项）。
- 昵称不进 petData / pet.json，不随分享卡水印替换官方品牌名（水印恒为 pawbae.ai）。
- 其余 12 只内置宠物不写 lore——只有 Yoonie 是官方 IP。
