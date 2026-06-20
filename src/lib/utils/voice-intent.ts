// Voice intent classification (PawBae voice-interaction Phase A).
//
// Pure logic, zero Svelte/Tauri/i18n imports — mirrors the `reaction-machine.ts`
// and physics `state-machine.ts` precedent so it is unit-testable without mounting
// a component, running on macOS, or initializing the recognizer.
//
// The Svelte layer (Main/MascotView) feeds the FINAL transcript (`is_final`) in,
// maps `emotion` onto the existing reaction-overlay slot, resolves `replyKey`
// through svelte-i18n, and applies `affectionDelta` with a cooldown.

/** The kinds of things the pet can recognize being said to it. */
export type VoiceIntent =
  | 'greet'
  | 'praise'
  | 'headpat'
  | 'feed'
  | 'sleep'
  | 'play'
  | 'scold'
  | 'callName'
  | 'unknown';

export interface IntentResult {
  intent: VoiceIntent;
  /**
   * A CodexPetState animation key (`happy` / `sleep` / `eat` / `angry` / ...) to
   * play on the reaction-overlay slot, or null to leave the base state alone.
   * Pets missing the state fall back to `idle` in MiniPetMascot — safe to over-specify.
   */
  emotion: string | null;
  /** svelte-i18n key for the pet's reply bubble. Resolved at the UI layer. */
  replyKey: string;
  /** Affection to add on a positive intent. Rate-limiting/gating lives at the UI layer. */
  affectionDelta: number;
}

export interface IntentContext {
  /** Display name of the active pet, used for the `callName` intent. May be empty. */
  petName: string;
}

interface Rule {
  intent: Exclude<VoiceIntent, 'callName' | 'unknown'>;
  /** Lower-cased substrings; a hit on any one matches the rule. */
  keywords: string[];
  emotion: string | null;
  replyKey: string;
  affection: number;
}

// Ordered HIGH → LOW priority; the first rule with a keyword hit wins.
// Ordering invariants that matter (and are covered by tests):
//   - `scold` must outrank `greet`        → "你好笨啊" is a scold, not a greet.
//   - `praise` must outrank `greet`       → "你好可爱" is praise, not a greet.
//   - no bare "good" in praise            → "good night" stays a sleep intent.
const RULES: readonly Rule[] = [
  {
    intent: 'headpat',
    keywords: ['摸摸头', '摸摸', '摸你', 'rua', 'pat', 'pet you'],
    emotion: 'happy',
    replyKey: 'voice.reply.headpat',
    affection: 1,
  },
  {
    intent: 'praise',
    keywords: [
      '可爱',
      '真棒',
      '好棒',
      '乖',
      '厉害',
      '么么',
      'cute',
      'nice',
      'well done',
      'good job',
      'good boy',
      'good girl',
    ],
    emotion: 'happy',
    replyKey: 'voice.reply.praise',
    affection: 1,
  },
  {
    intent: 'scold',
    keywords: ['笨', '坏', '讨厌', '烦', 'bad', 'stupid', 'silly'],
    emotion: 'angry',
    replyKey: 'voice.reply.scold',
    affection: 0,
  },
  {
    intent: 'feed',
    keywords: ['吃饭', '喂你', '喂食', '吃东西', '零食', '饿', 'eat', 'hungry', 'food', 'snack'],
    emotion: 'eat',
    replyKey: 'voice.reply.feed',
    affection: 0,
  },
  {
    intent: 'sleep',
    keywords: ['睡觉', '睡吧', '晚安', '困了', 'sleep', 'good night', 'bed'],
    emotion: 'sleep',
    replyKey: 'voice.reply.sleep',
    affection: 0,
  },
  {
    intent: 'play',
    keywords: ['出来玩', '一起玩', '陪我', '玩游戏', '玩', 'play', 'have fun'],
    emotion: 'happy',
    replyKey: 'voice.reply.play',
    affection: 1,
  },
  {
    intent: 'greet',
    keywords: [
      '你好',
      '哈喽',
      '早安',
      '早上好',
      '下午好',
      '晚上好',
      '嗨',
      'hello',
      'hey',
      'morning',
    ],
    emotion: 'happy',
    replyKey: 'voice.reply.greet',
    affection: 0,
  },
];

function unknownResult(): IntentResult {
  return { intent: 'unknown', emotion: null, replyKey: 'voice.reply.unknown', affectionDelta: 0 };
}

/** True when any concrete rule keyword appears — used to let a strong intent
 *  outrank a bare name mention ("homie 你好" → greet, not callName). */
function hasStrongIntent(text: string): boolean {
  return RULES.some((r) => r.keywords.some((k) => text.includes(k)));
}

/**
 * Classify a final transcript into the pet's reaction.
 *
 * Pure and side-effect free: the same (text, ctx) always yields a deep-equal
 * result. Whitespace- and case-insensitive; matches by substring (no tokenizer),
 * which keeps CJK — where word boundaries are fuzzy — working without a segmenter.
 */
export function classifyIntent(raw: string, ctx: IntentContext): IntentResult {
  const text = raw.trim().toLowerCase();
  if (!text) return unknownResult();

  // Name call wins only when nothing more specific was said.
  const petName = ctx.petName.trim().toLowerCase();
  if (petName && text.includes(petName) && !hasStrongIntent(text)) {
    return {
      intent: 'callName',
      emotion: 'happy',
      replyKey: 'voice.reply.callName',
      affectionDelta: 0,
    };
  }

  for (const r of RULES) {
    if (r.keywords.some((k) => text.includes(k))) {
      return {
        intent: r.intent,
        emotion: r.emotion,
        replyKey: r.replyKey,
        affectionDelta: r.affection,
      };
    }
  }
  return unknownResult();
}
