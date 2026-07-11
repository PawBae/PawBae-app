import { describe, expect, it } from 'vitest';
import { effectiveName, NICKNAME_MAX, sanitizeNickname, sanitizeNicknames } from './pet-name';

describe('sanitizeNickname', () => {
  it('trims and collapses internal whitespace', () => {
    expect(sanitizeNickname('  团 团  ')).toBe('团 团');
    expect(sanitizeNickname('a\t\n b')).toBe('a b');
  });

  it('caps at NICKNAME_MAX', () => {
    expect(sanitizeNickname('x'.repeat(50))).toHaveLength(NICKNAME_MAX);
  });

  it('whitespace-only becomes empty', () => {
    expect(sanitizeNickname('   \n\t')).toBe('');
  });
});

describe('effectiveName', () => {
  it('prefers a non-empty nickname', () => {
    expect(effectiveName('团团', 'Yoonie')).toBe('团团');
  });

  it('falls back to the official name on empty/undefined/whitespace', () => {
    expect(effectiveName(undefined, 'Yoonie')).toBe('Yoonie');
    expect(effectiveName('', 'Yoonie')).toBe('Yoonie');
    expect(effectiveName('   ', 'Yoonie')).toBe('Yoonie');
  });

  it('sanitizes the nickname it returns', () => {
    expect(effectiveName('  皮 皮  ', 'Yoonie')).toBe('皮 皮');
  });
});

describe('sanitizeNicknames', () => {
  it('keeps only non-empty string entries, sanitized', () => {
    const out = sanitizeNicknames({
      yoonie: '  团团 ',
      solu: '',
      muru: 42,
      luma: '   ',
    });
    expect(out).toEqual({ yoonie: '团团' });
  });

  it('rejects non-object shapes', () => {
    expect(sanitizeNicknames(null)).toEqual({});
    expect(sanitizeNicknames('yoonie')).toEqual({});
    expect(sanitizeNicknames([['yoonie', 'x']])).toEqual({});
  });

  it('keeps __proto__ as an ordinary own key (null prototype)', () => {
    const out = sanitizeNicknames(JSON.parse('{"__proto__": "evil"}'));
    expect(Object.getPrototypeOf(out)).toBeNull();
    expect(out.__proto__).toBe('evil');
    expect(({} as Record<string, unknown>).__proto__).not.toBe('evil');
  });
});
