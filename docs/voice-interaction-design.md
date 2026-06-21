# 语音互动功能设计方案（Voice Interaction）

> 目标：让宠物不仅能"听到"语音并把文字显示在头顶聊天框（**已实现**），还能"听懂"
> 并做出表情 / 动画 / 回话 / 好感度反应（**本次新增**）。

## 实现现状（与设计的差异，2026-06）

落地过程中部分设计被现实约束修正，**以下为当前真实行为**，本文其余章节为原始设计意图：

- **识别语言：中文单识别器**。`recognizer_locales()` 默认只跑 `zh-CN`（`voice_set_locale`
  可单独 pin 到其它语言）。**不做**中英文并发自动识别——macOS 的 `SFSpeechRecognizer`
  无法可靠地同时运行多个识别器（第二个会被饿死，无论本地或服务器）。真正的双语自动识别
  需要"录音缓存 + 顺序识别"（有延迟），目前未实现。
- **设置开关默认关闭**（隐私默认）：前端 `voiceEnabled` 与后端 `VOICE_ENABLED` 均默认
  `false`；关闭时快捷键不开麦，**录音中关闭会立即停麦**并发 `voice-status{recording:false}`；
  延迟到达的 final transcript 在关闭后不分类、不回话、不加好感。
- **聊天框智能翻转**：默认在宠物头顶上方，当宠物窗口贴近屏幕顶部时自动翻到下方（宠物可贴顶）。
- 设备端识别需要系统**听写（Dictation）**开启，否则走服务器识别（需联网）。

## 0. 现状基线（已实现，复用而非重做）

| 能力 | 实现 | 文件 |
| --- | --- | --- |
| 语音识别（macOS） | `SFSpeechRecognizer` + `AVAudioEngine`，实时部分结果，30s 上限 / 8s 静音自动停 | `src-tauri/src/speech.rs` |
| 录音触发 | 全局快捷键 `Ctrl+Shift+V`，`voice_toggle` / `voice_is_recording` 命令 | `setup.rs`, `commands/misc.rs` |
| 事件回传 | `voice-status {recording,error}` / `voice-transcript {text,is_final}` | `speech.rs` → `Main.svelte` |
| 头顶聊天框 | `VoiceBubble.svelte`（pet 模式气泡 + mini 模式录音红点） | `MascotView.svelte` |
| 一次性反应状态机（参考范式） | 键鼠输入 → sprite 反应，纯逻辑可单测 | `utils/reaction-machine.ts` |

**结论**：识别与"显示听到的文字"已闭环；本设计只新增「意图识别 → 宠物反应 → 回话」一层，
并补齐设置开关、跨平台与少量底座修复。

## 1. 目标与非目标

**目标**
1. 宠物对最终识别文本（`is_final`）做**意图识别**，触发对应**表情/动画**。
2. 新增宠物**回话气泡**（区别于"听到的文字"回显气泡），让互动有来有回。
3. 命中正向意图时联动现有**好感度（affection）**账本，带速率限制，幂等安全。
4. 增加**设置开关**（启用语音、隐私说明、快捷键提示），默认关闭，符合"本地优先/隐私默认"。
5. 全流程可测：纯逻辑单测 + Rust 单测 + 端到端手测脚本。

**非目标（本期不做）**
- 不做云端 ASR / 不上传音频（隐私默认本地）。
- 不做大模型对话生成（回话用规则模板，避免成本与隐私问题；可作为后续 Phase）。
- 不强求 Windows 语音同步上线（列为独立 Phase，给出方案但不阻塞主线）。

## 2. 架构与数据流

```
                       ┌─────────────────────────── Rust (src-tauri) ───────────────────────────┐
  麦克风 ──► AVAudioEngine ──► SFSpeechRecognizer ──► emit "voice-transcript"{text,is_final}
                                                  └─► emit "voice-status"{recording,error}
                       └────────────────────────────────────────────────────────────────────────┘
                                                  │  (已存在)
                                                  ▼
   ┌──────────────────────────── Frontend (Svelte 5) ─────────────────────────────┐
   │ Main.svelte  listen(voice-transcript/voice-status)                            │
   │     │  is_final 文本                                                          │
   │     ▼                                                                          │
   │ ★ utils/voice-intent.ts  (新增·纯函数·可单测)                                │
   │     classifyIntent(text) ─► { intent, emotion, replyKey, affectionDelta }     │
   │     │                                                                          │
   │     ▼                                                                          │
   │ ★ MascotView 消费意图：                                                       │
   │     - reactionSprite = emotion(happy/sleep/eat/angry/...) 复用 overlay 槽位   │
   │     - petReply = i18n(replyKey)  ─► ★ PetReplyBubble.svelte (新增)            │
   │     - petStore.addAffection(delta)（带 cooldown）                             │
   └──────────────────────────────────────────────────────────────────────────────┘
```

**关键设计原则**：把"听懂"做成**纯函数** `classifyIntent(text, ctx)`，零 Svelte/Tauri 依赖，
完全镜像 `reaction-machine.ts` / `physics/state-machine.ts` 的范式，便于无 GUI、无 macOS 单测。

## 3. 模块设计

### 3.1 新增 `src/lib/utils/voice-intent.ts`（纯逻辑核心）

```ts
export type VoiceIntent =
  | 'greet' | 'praise' | 'headpat' | 'feed' | 'sleep'
  | 'play' | 'scold' | 'callName' | 'unknown';

export interface IntentResult {
  intent: VoiceIntent;
  /** 复用 CodexPetState 字符串（idle/happy/sleep/eat/angry/...），可能为 null（不切动画） */
  emotion: string | null;
  /** i18n key，宠物回话气泡文案；unknown 时为侧头疑惑 '?' */
  replyKey: string;
  /** 好感度增量，仅正向意图 > 0；上层做速率限制 */
  affectionDelta: number;
}

export interface IntentContext {
  /** 宠物名，用于 callName 命中（来自 pet store） */
  petName: string;
  /** 当前语言，影响关键词表与回话 */
  lang: 'zh' | 'en';
}

// 关键词表（zh + en，全部小写归一化后匹配；按优先级从高到低）
const RULES: Array<{ intent: VoiceIntent; keywords: string[]; emotion: string | null;
                     replyKey: string; affection: number }> = [
  { intent: 'headpat', keywords: ['摸摸','摸摸头','rua','摸你','pat','头'],        emotion: 'happy', replyKey: 'voice.reply.headpat', affection: 1 },
  { intent: 'praise',  keywords: ['好可爱','真棒','乖','厉害','么么','good','cute','nice'], emotion: 'happy', replyKey: 'voice.reply.praise',  affection: 1 },
  { intent: 'feed',    keywords: ['吃饭','喂你','饿','零食','eat','hungry','food'],  emotion: 'eat',   replyKey: 'voice.reply.feed',    affection: 0 },
  { intent: 'sleep',   keywords: ['睡觉','晚安','困了','sleep','good night'],         emotion: 'sleep', replyKey: 'voice.reply.sleep',   affection: 0 },
  { intent: 'play',    keywords: ['玩','出来玩','陪我','play'],                        emotion: 'happy', replyKey: 'voice.reply.play',    affection: 1 },
  { intent: 'greet',   keywords: ['你好','哈喽','早安','早上好','hello','hi','hey'],   emotion: 'happy', replyKey: 'voice.reply.greet',   affection: 0 },
  { intent: 'scold',   keywords: ['笨','坏','讨厌','bad','stupid'],                    emotion: 'angry', replyKey: 'voice.reply.scold',  affection: 0 },
];

export function classifyIntent(raw: string, ctx: IntentContext): IntentResult {
  const text = raw.trim().toLowerCase();
  if (!text) return unknown();
  // 1) 叫名字优先级最高（宠物名出现且无其他强意图时）
  if (ctx.petName && text.includes(ctx.petName.toLowerCase()) && !hasStrong(text)) {
    return { intent: 'callName', emotion: 'happy', replyKey: 'voice.reply.callName', affectionDelta: 0 };
  }
  for (const r of RULES) {
    if (r.keywords.some((k) => text.includes(k))) {
      return { intent: r.intent, emotion: r.emotion, replyKey: r.replyKey, affectionDelta: r.affection };
    }
  }
  return unknown();
}

function unknown(): IntentResult {
  return { intent: 'unknown', emotion: null, replyKey: 'voice.reply.unknown', affectionDelta: 0 };
}
```

设计要点：
- **大小写/空白归一化**，中文不分词，英文按子串匹配（关键词足够短即可命中）。
- **优先级**：叫名字 > headpat > praise > … > greet > scold；同一句多命中取最高优先。
- **emotion 复用现有 sprite 状态字符串**（`idle/happy/sleep/eat/angry/walk/...`），不引入新动画资产；
  若某宠物缺少该状态，`MiniPetMascot` 已有回退到 `idle` 的逻辑，安全降级。
- **affectionDelta** 只产出数值，**速率限制在上层**（见 3.3），保证纯函数无副作用、可测。

### 3.2 新增 `src/lib/components/PetReplyBubble.svelte`（回话气泡）

- 与 `VoiceBubble` 分离：`VoiceBubble` 显示"我听到你说什么"（橙色），`PetReplyBubble`
  显示"宠物回你什么"（区分配色，如宠物主题色 / 白底），避免两种语义挤在一个气泡。
- props：`{ visible, text }`；进入/退出做淡入 + 轻微弹跳；`max-width` 截断同 VoiceBubble。
- 显示时机：仅 `is_final` 文本分类后；展示 ~2.5s 后淡出；与庆祝气泡 `CelebrationBubble`
  做优先级互斥（庆祝 > 回话 > 活动气泡，沿用 `AgentBubble.svelte` 里既有的优先级注释约定）。

### 3.3 `MascotView.svelte` / `Main.svelte` 接线（消费意图）

在 `Main.svelte` 的 `voice-transcript` 监听里，仅当 `is_final` 时跑分类，并把结果下发：

```ts
addListener<{ text: string; is_final: boolean }>('voice-transcript', (e) => {
  voiceText = e.payload.text;                 // 既有：回显听到的文字
  if (e.payload.is_final && settings.voiceEnabled) {
    const r = classifyIntent(e.payload.text, { petName: pet.name, lang: i18n.lang });
    voiceIntent = r;                           // 新增：下发给 MascotView
  }
});
```

`MascotView` 消费 `voiceIntent`：
- `emotion !== null` → 复用既有 overlay 槽位（`reactionSprite`/`overlaySprite`），播放 `REACTION_MS`
  之后回退（与键鼠反应同一套定时回退，避免新建并行机制）。
- `replyKey` → `petReply = t(replyKey)`，传给 `PetReplyBubble`。
- `affectionDelta > 0` → 调 `petStore.addAffection(delta)`，但加 **cooldown（如 10s）**，
  防止用户连续说话刷好感；cooldown 状态放在 store，复用既有 reward ledger 幂等思路。

**忙碌保护**：复用现有 `busy` 判定（拖拽/抛掷/hover/headpat/物理非静止时）——忙碌时
**仍显示回话气泡**（纯 UI），但**不抢动画**（不切 emotion），与 `reaction-machine` 的 busy-guard 一致。

### 3.4 设置面板（新增开关）

在 `settings/` 下（参考 `PrivacySection.svelte` / `SoundSection.svelte`）加"语音互动"：
- `voiceEnabled`（默认 **false**）：总开关，关闭时不注册录音、不分类。
- 隐私说明文案：音频仅本机处理、不上传、识别由系统 Speech 框架完成。
- 显示当前快捷键 `Ctrl+Shift+V` 与"需要在 系统设置→隐私→语音识别/麦克风 授权"。
- `voiceAffectionEnabled`（可选）：是否允许语音加好感度。

持久化进现有 settings store（`stores/settings.svelte.ts`）。

## 4. 底座缺口修复（小而必要）

1. **静音自动停的判定**（`speech.rs:154-164`）：`silence_elapsed` 用 `elapsed - last_result/1000`，
   `last_result` 是"上次出结果距开始的秒数"；逻辑可用但边界 off-by-one，建议补单测或改为
   "距上次结果的真实间隔"。低风险，列为 P2。
2. **Windows / 非 macOS**：`voice_toggle` 直接返回错误。给出两条后续路线（独立 Phase，不阻塞）：
   - Web Speech API（`webkitSpeechRecognition`，WebView2 支持有限，需联网，隐私差）；
   - `whisper.cpp` / `vosk` 本地 sidecar（离线、隐私好，体积/性能成本高）。
   推荐离线 sidecar，但先发 macOS。
3. **录音中无可见"取消"入口**：mini 模式只有红点，建议点红点即 `voice_toggle` 停止。

## 5. 开发流程（分阶段，遵循 CLAUDE.md：永不直推 main，走 PR）

> 分支命名：`feat/voice-intent-engine` 等；每阶段一个 PR，先单测后接线。

- **Phase A — 纯逻辑（无 UI 风险，先合）**
  1. `feat/voice-intent-engine`：新增 `utils/voice-intent.ts` + `voice-intent.test.ts`（见 §6.1）。
  2. i18n：`zh.json` / `en.json` 补 `voice.reply.*` 文案键。
  - 验收：`pnpm test` 全绿；无 UI 改动。

- **Phase B — 回话气泡组件**
  3. `feat/pet-reply-bubble`：新增 `PetReplyBubble.svelte`；Storybook/手测渲染。
  - 验收：气泡进出动画正常，超长文本截断，与庆祝气泡互斥。

- **Phase C — 接线 + 好感度**
  4. `feat/voice-intent-wiring`：`Main.svelte` 分类下发、`MascotView` 消费 emotion/reply、
     好感度 cooldown。
  - 验收：§6.4 手测脚本逐条通过。

- **Phase D — 设置开关**
  5. `feat/voice-settings`：设置面板"语音互动"分区 + 持久化 + 默认关闭。
  - 验收：关闭时完全静默；开启后快捷键生效。

- **Phase E（可选/后续）** — 底座修复（静音判定单测）、Windows ASR sidecar 调研。

每个 PR 必须：`pnpm test` + `pnpm check`（svelte-check）+ `cargo test`（若动 Rust）全绿，
并在 PR 描述里附手测结果。

## 6. 全套测试用例

### 6.1 单元测试 — `voice-intent.test.ts`（Vitest，纯函数，核心）

| # | 输入 text | ctx | 期望 intent | emotion | affectionDelta |
| --- | --- | --- | --- | --- | --- |
| U1 | `"你好呀"` | zh | greet | happy | 0 |
| U2 | `"hello"` | en | greet | happy | 0 |
| U3 | `"摸摸头"` | zh | headpat | happy | 1 |
| U4 | `"你好可爱"` | zh | **headpat 或 praise**（验证优先级确定且稳定） | happy | 1 |
| U5 | `"我饿了想吃饭"` | zh | feed | eat | 0 |
| U6 | `"晚安睡觉啦"` | zh | sleep | sleep | 0 |
| U7 | `"出来陪我玩"` | zh | play | happy | 1 |
| U8 | `"你好笨啊"` | zh | scold（验证含"你好"也不被 greet 抢） | angry | 0 |
| U9 | `"<petName>"`（如 `"homie"`） | petName=homie | callName | happy | 0 |
| U10 | `"homie 你好"` | petName=homie | greet（叫名 + 强意图时让位强意图） | happy | 0 |
| U11 | `"今天天气不错"` | zh | unknown | null | 0 |
| U12 | `""` / `"   "` | any | unknown | null | 0 |
| U13 | `"HELLO"` / `"  Hi  "` | en | greet（大小写/空白归一化） | happy | 0 |
| U14 | `"摸摸摸摸摸"`（重复） | zh | headpat（幂等，单次结果） | happy | 1 |
| U15 | 超长 500 字含"睡觉" | zh | sleep（长文本不崩、命中子串） | sleep | 0 |
| U16 | emoji/标点 `"你好！😀"` | zh | greet（标点不影响匹配） | happy | 0 |

断言：`classifyIntent` 为**纯函数**——同输入多次调用结果全等（深比较），无副作用。

### 6.2 单元测试 — 好感度 cooldown（store 逻辑）

| # | 场景 | 期望 |
| --- | --- | --- |
| C1 | 首次正向意图 → addAffection(1) | 好感 +1 |
| C2 | cooldown 窗口内再次正向意图 | 好感**不变**（被限流） |
| C3 | cooldown 过后再次正向意图 | 好感 +1 |
| C4 | unknown / 中性意图（feed/sleep/greet/scold） | 好感不变 |
| C5 | `voiceAffectionEnabled=false` | 任何意图都不加好感 |

（用可注入的时钟，禁止真实计时；遵循仓库"纯逻辑可单测"约定。）

### 6.3 Rust 单元测试 — `speech.rs`

> 与 macOS 框架强耦合，拆出可测纯函数后再测，避免真录音。

| # | 目标 | 用例 |
| --- | --- | --- |
| R1 | 静音/超时判定 | 抽出 `fn should_autostop(elapsed, last_result_ms, now)`，覆盖：未出结果即静音、刚出结果不停、超 8s 静音停、超 30s 强停、off-by-one 边界 |
| R2 | `is_recording` 状态机 | Start→true，Stop→false，重复 Start 幂等 |
| R3 | `nsstring_to_string(null)` | 返回空串不崩 |
| R4 | 平台门控 | 非 macOS 下 `voice_toggle` 返回 `Err("...not supported...")` |

### 6.4 集成 / 手动 E2E 脚本（macOS，真机）

前置：系统设置授权 麦克风 + 语音识别；设置面板开启"语音互动"。

| # | 操作 | 期望 |
| --- | --- | --- |
| E1 | 按 `Ctrl+Shift+V` | mini 出现录音红点 / pet 模式出现录音气泡，`voice-status.recording=true` |
| E2 | 说"你好" | 气泡实时回显文字；最终宠物切 happy 动画 + 回话气泡（如"汪~你好！"） |
| E3 | 说"摸摸头" | happy 动画 + 回话；好感度 +1（面板可见） |
| E4 | 连续说"摸摸"两次（10s 内） | 仅第一次加好感（cooldown 验证） |
| E5 | 说"睡觉" | 切 sleep 动画 + 回话；不加好感 |
| E6 | 说"你好笨" | angry 动画（不被 greet 抢） |
| E7 | 说一句无意图的话 | unknown：侧头/"?" 回话，不切表情、不加好感 |
| E8 | 录音中拖拽宠物（busy） | 回话气泡仍出，但**不打断**拖拽/物理动画 |
| E9 | 30s 不停说 / 说完静默 8s | 自动停止录音，`recording=false`，气泡 2s 后清空 |
| E10 | 关闭"语音互动"开关后按快捷键 | 不录音、不分类（完全静默） |
| E11 | 未授权麦克风 | `voice-status.error` 文案提示去系统设置授权，气泡红色错误样式 |
| E12 | pet 模式 vs mini 模式切换 | 两种模式气泡/红点表现符合 `VoiceBubble` 既有规则 |

### 6.5 回归测试

- 既有键鼠 `reaction-machine` 反应不受影响（语音 emotion 与键鼠反应共用 overlay 槽位，
  验证二者不互相清除、定时回退不冲突）。
- 庆祝气泡 / Agent 活动气泡与回话气泡的优先级互斥正确。
- 关闭语音时，CPU/麦克风占用为 0（无后台录音）。

### 6.6 非功能 / 边界

- **隐私**：抓包确认无音频/文本外传；关闭开关后无任何麦克风访问。
- **i18n**：中/英切换时关键词表与回话文案随 `lang` 切换，无硬编码中文漏网。
- **性能**：`classifyIntent` O(规则数×句长)，单次 < 1ms；高频 final 事件不卡 UI。
- **健壮**：空串、超长串、纯标点/emoji、非中英文输入均返回 unknown 不抛异常。

## 7. 风险与权衡

| 风险 | 说明 | 缓解 |
| --- | --- | --- |
| 关键词召回有限 | 规则匹配无法覆盖自然表达 | 先发规则版（零成本/隐私好），后续 Phase 接本地小模型或 LLM 意图分类 |
| Windows 无语音 | 框架仅 macOS | 独立 Phase，离线 sidecar；不阻塞 macOS 主线 |
| 好感度被刷 | 语音连说 | cooldown + 可关开关 |
| 误触发 | 环境噪声被识别 | 仅 `is_final` 才分类；unknown 安全降级 |

## 8. 涉及文件清单

**新增**：`src/lib/utils/voice-intent.ts`、`voice-intent.test.ts`、
`src/lib/components/PetReplyBubble.svelte`、`settings/VoiceSection.svelte`（或并入 Privacy）。

**修改**：`Main.svelte`（分类下发）、`MascotView.svelte`（消费 emotion/reply）、
`stores/pet.svelte.ts`（好感 cooldown）、`stores/settings.svelte.ts`（开关）、
`i18n/zh.json` + `en.json`、`speech.rs`（静音判定抽函数+单测，P2）。
