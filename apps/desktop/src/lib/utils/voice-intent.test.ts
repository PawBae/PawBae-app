import { describe, expect, it } from 'vitest';
import { classifyIntent, type IntentContext } from './voice-intent';

const ctx = (...petNames: string[]): IntentContext => ({ petNames });

describe('classifyIntent — intents', () => {
  it('U1 zh greeting → greet/happy, no affection', () => {
    const r = classifyIntent('你好呀', ctx());
    expect(r.intent).toBe('greet');
    expect(r.emotion).toBe('happy');
    expect(r.affectionDelta).toBe(0);
  });

  it('U2 en greeting → greet', () => {
    expect(classifyIntent('hello', ctx()).intent).toBe('greet');
  });

  it('U3 headpat → happy + affection', () => {
    const r = classifyIntent('摸摸头', ctx());
    expect(r.intent).toBe('headpat');
    expect(r.emotion).toBe('happy');
    expect(r.affectionDelta).toBe(1);
  });

  it('U4 "你好可爱" → praise (praise outranks greet)', () => {
    const r = classifyIntent('你好可爱', ctx());
    expect(r.intent).toBe('praise');
    expect(r.affectionDelta).toBe(1);
  });

  it('U5 feed → eat', () => {
    const r = classifyIntent('我饿了想吃饭', ctx());
    expect(r.intent).toBe('feed');
    expect(r.emotion).toBe('eat');
    expect(r.affectionDelta).toBe(0);
  });

  it('U6 sleep → sleep', () => {
    const r = classifyIntent('晚安睡觉啦', ctx());
    expect(r.intent).toBe('sleep');
    expect(r.emotion).toBe('sleep');
  });

  it('U7 play → happy + affection', () => {
    const r = classifyIntent('出来陪我玩', ctx());
    expect(r.intent).toBe('play');
    expect(r.affectionDelta).toBe(1);
  });

  it('U8 "你好笨啊" → scold (scold outranks greet)', () => {
    const r = classifyIntent('你好笨啊', ctx());
    expect(r.intent).toBe('scold');
    expect(r.emotion).toBe('angry');
    expect(r.affectionDelta).toBe(0);
  });
});

describe('classifyIntent — name calls', () => {
  it('U9 bare name → callName', () => {
    const r = classifyIntent('homie', ctx('Homie'));
    expect(r.intent).toBe('callName');
    expect(r.emotion).toBe('happy');
  });

  it('U10 name + strong intent → the strong intent wins', () => {
    expect(classifyIntent('homie 你好', ctx('Homie')).intent).toBe('greet');
  });

  it('empty pet name never matches callName', () => {
    expect(classifyIntent('天气不错', ctx('')).intent).toBe('unknown');
  });

  it('nickname AND official name both answer (naming+lore)', () => {
    expect(classifyIntent('团团', ctx('团团', 'Yoonie')).intent).toBe('callName');
    expect(classifyIntent('yoonie 过来', ctx('团团', 'Yoonie')).intent).toBe('callName');
  });

  it('blank entries in the name list are ignored', () => {
    expect(classifyIntent('天气不错', ctx('  ', '')).intent).toBe('unknown');
  });
});

describe('classifyIntent — fallbacks & normalization', () => {
  it('U11 no keyword → unknown, no emotion, no affection', () => {
    const r = classifyIntent('今天天气不错', ctx());
    expect(r.intent).toBe('unknown');
    expect(r.emotion).toBeNull();
    expect(r.affectionDelta).toBe(0);
  });

  it('U12 empty / whitespace → unknown', () => {
    expect(classifyIntent('', ctx()).intent).toBe('unknown');
    expect(classifyIntent('   ', ctx()).intent).toBe('unknown');
  });

  it('U13 case + surrounding whitespace normalized', () => {
    expect(classifyIntent('HELLO', ctx()).intent).toBe('greet');
    expect(classifyIntent('  Morning  ', ctx()).intent).toBe('greet');
  });

  it('U14 repeated keyword is idempotent (single result)', () => {
    const r = classifyIntent('摸摸摸摸摸', ctx());
    expect(r.intent).toBe('headpat');
    expect(r.affectionDelta).toBe(1);
  });

  it('U15 long text still matches a contained keyword', () => {
    const long = `${'闲聊'.repeat(200)}该睡觉了`;
    expect(classifyIntent(long, ctx()).intent).toBe('sleep');
  });

  it('U16 punctuation / emoji do not break matching', () => {
    expect(classifyIntent('你好！😀', ctx()).intent).toBe('greet');
  });

  it('"good night" stays sleep (not captured by praise)', () => {
    expect(classifyIntent('good night', ctx()).intent).toBe('sleep');
  });
});

describe('classifyIntent — purity', () => {
  it('same input yields a deep-equal result every call (no side effects)', () => {
    const a = classifyIntent('摸摸头', ctx('Homie'));
    const b = classifyIntent('摸摸头', ctx('Homie'));
    expect(a).toEqual(b);
    expect(a).not.toBe(b);
  });
});
