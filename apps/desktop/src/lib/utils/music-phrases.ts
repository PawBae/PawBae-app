// Music reaction phrases the pet says while listening. The strings themselves live in
// i18n (petMusic.*) so they translate; this module owns the KEYS and the pure picker so
// the selection logic is unit-testable without svelte-i18n. MascotView resolves the
// chosen key with `$_()`.

/** i18n keys for the rotating "I'm vibing" lines. Order is irrelevant to the picker. */
export const MUSIC_PHRASE_KEYS: readonly string[] = [
  'petMusic.p1',
  'petMusic.p2',
  'petMusic.p3',
  'petMusic.p4',
  'petMusic.p5',
  'petMusic.p6',
  'petMusic.p7',
  'petMusic.p8',
] as const;

/**
 * Pick the next phrase index in [0, count). `rand` ∈ [0, 1) is injected for determinism in
 * tests (the caller passes Math.random()). Avoids repeating `lastIndex` so two consecutive
 * lines never read the same. Returns -1 when there are no phrases.
 */
export function pickPhraseIndex(count: number, lastIndex: number, rand: number): number {
  if (count <= 0) return -1;
  if (count === 1) return 0;

  const r = Number.isFinite(rand) ? Math.min(Math.max(rand, 0), 0.999999) : 0;
  let idx = Math.floor(r * count);
  if (idx >= count) idx = count - 1; // float guard

  if (idx === lastIndex) idx = (idx + 1) % count; // never repeat back-to-back
  return idx;
}
