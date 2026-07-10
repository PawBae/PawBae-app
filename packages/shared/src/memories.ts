import { assertBoundedInteger, assertEnum, assertExactRecord, deepFreeze } from './validation';

export const MEMORY_TEMPLATE_KEYS = Object.freeze([
  'played_together',
  'worked_together',
  'celebrated_completion',
  'shared_snack',
] as const);
export type MemoryTemplateKey = (typeof MEMORY_TEMPLATE_KEYS)[number];

export const MEMORY_DURATION_BUCKETS = Object.freeze(['short', 'full'] as const);
export type MemoryDurationBucket = (typeof MEMORY_DURATION_BUCKETS)[number];

export const MEMORY_TIMES_OF_DAY = Object.freeze([
  'morning',
  'afternoon',
  'evening',
  'night',
] as const);
export type MemoryTimeOfDay = (typeof MEMORY_TIMES_OF_DAY)[number];

export interface MemoryTemplateParams {
  readonly durationBucket: MemoryDurationBucket;
  readonly timeOfDay: MemoryTimeOfDay;
  readonly interactionCount: number;
}

export interface MemoryTemplatePayload {
  readonly templateKey: MemoryTemplateKey;
  readonly params: MemoryTemplateParams;
}

export type MemoryLocale = 'en' | 'zh';

export interface MemoryTemplateCopy {
  readonly title: string;
  readonly body: string;
}

export type MemoryTemplateLocalizationTable = Readonly<
  Record<MemoryLocale, Readonly<Record<MemoryTemplateKey, Readonly<MemoryTemplateCopy>>>>
>;

export interface MemoryParameterLocalization {
  readonly durationBucket: Readonly<Record<MemoryDurationBucket, string>>;
  readonly timeOfDay: Readonly<Record<MemoryTimeOfDay, string>>;
}

const MEMORY_PARAM_KEYS = Object.freeze([
  'durationBucket',
  'timeOfDay',
  'interactionCount',
] as const);

export function createMemoryTemplatePayload(
  templateKey: unknown,
  input: unknown,
): MemoryTemplatePayload {
  const key = assertEnum(templateKey, MEMORY_TEMPLATE_KEYS, 'memory template key');
  const raw = assertExactRecord(input, MEMORY_PARAM_KEYS, 'memory params');
  const params = Object.freeze({
    durationBucket: assertEnum(
      raw.durationBucket,
      MEMORY_DURATION_BUCKETS,
      'durationBucket',
    ),
    timeOfDay: assertEnum(raw.timeOfDay, MEMORY_TIMES_OF_DAY, 'timeOfDay'),
    interactionCount: assertBoundedInteger(raw.interactionCount, 0, 100, 'interactionCount'),
  });
  return Object.freeze({ templateKey: key, params });
}

export const MEMORY_TEMPLATE_FIXTURES = Object.freeze([
  createMemoryTemplatePayload('played_together', {
    durationBucket: 'short',
    timeOfDay: 'morning',
    interactionCount: 4,
  }),
  createMemoryTemplatePayload('worked_together', {
    durationBucket: 'full',
    timeOfDay: 'afternoon',
    interactionCount: 12,
  }),
  createMemoryTemplatePayload('celebrated_completion', {
    durationBucket: 'short',
    timeOfDay: 'evening',
    interactionCount: 2,
  }),
  createMemoryTemplatePayload('shared_snack', {
    durationBucket: 'short',
    timeOfDay: 'night',
    interactionCount: 1,
  }),
]);

export const MEMORY_TEMPLATE_LOCALIZATIONS: MemoryTemplateLocalizationTable = deepFreeze({
  en: {
    played_together: {
      title: 'A visit together',
      body:
        'They played through a {durationBucket} {timeOfDay} and shared {interactionCount} little moments.',
    },
    worked_together: {
      title: 'Side by side',
      body:
        'They kept each other company for a {durationBucket} {timeOfDay} and shared {interactionCount} little moments.',
    },
    celebrated_completion: {
      title: 'A tiny celebration',
      body:
        'A {durationBucket} {timeOfDay} brought {interactionCount} reasons to celebrate together.',
    },
    shared_snack: {
      title: 'A snack for two',
      body:
        'They shared a snack during a {durationBucket} {timeOfDay} and enjoyed {interactionCount} little moments.',
    },
  },
  zh: {
    played_together: {
      title: '一起串门',
      body: '她们在一个{durationBucket}的{timeOfDay}一起玩，留下了{interactionCount}个小瞬间。',
    },
    worked_together: {
      title: '并肩作伴',
      body: '她们在一个{durationBucket}的{timeOfDay}互相陪伴，留下了{interactionCount}个小瞬间。',
    },
    celebrated_completion: {
      title: '小小庆祝',
      body: '这个{durationBucket}的{timeOfDay}里，她们一起庆祝了{interactionCount}个小瞬间。',
    },
    shared_snack: {
      title: '两只的点心',
      body: '她们在一个{durationBucket}的{timeOfDay}分享点心，留下了{interactionCount}个小瞬间。',
    },
  },
});

export const MEMORY_PARAMETER_LOCALIZATIONS: Readonly<
  Record<MemoryLocale, Readonly<MemoryParameterLocalization>>
> = deepFreeze({
  en: {
    durationBucket: { short: 'little', full: 'full' },
    timeOfDay: {
      morning: 'morning',
      afternoon: 'afternoon',
      evening: 'evening',
      night: 'night',
    },
  },
  zh: {
    durationBucket: { short: '短短', full: '完整' },
    timeOfDay: { morning: '早晨', afternoon: '午后', evening: '傍晚', night: '夜晚' },
  },
});
