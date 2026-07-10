// Pet naming (角色 IP brick #1). Yoonie is the OFFICIAL character — lore, marketing,
// and the share-card watermark always use the official name. The nickname is the
// user's per-pet override for in-app address (profile card, share card body, voice
// call-name). Pure logic, zero Svelte/Tauri imports — the reducer precedent.
// See docs/superpowers/specs/2026-07-08-naming-lore-design.md.

export const NICKNAME_MAX = 20;

/**
 * Normalize a raw nickname: trim, collapse internal whitespace runs to one space,
 * hard-cap at NICKNAME_MAX code units. Empty output means "no nickname" — callers
 * fall back to the official name.
 */
export function sanitizeNickname(raw: string): string {
  return raw.trim().replace(/\s+/g, ' ').slice(0, NICKNAME_MAX);
}

/** The name the pet answers to: sanitized nickname when present, else official. */
export function effectiveName(nickname: string | undefined, officialName: string): string {
  const nick = sanitizeNickname(nickname ?? '');
  return nick || officialName;
}

/**
 * Hydrate-time sanitizer for the persisted nickname map (a hand-edited
 * settings.json must not crash or poison state — the daily-board precedent).
 * Keeps only string→string entries whose sanitized value is non-empty; returns a
 * null-prototype record so a hostile key like "__proto__" stays an ordinary key.
 */
export function sanitizeNicknames(raw: unknown): Record<string, string> {
  const out: Record<string, string> = Object.create(null);
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) return out;
  for (const [k, v] of Object.entries(raw)) {
    if (typeof v !== 'string') continue;
    const nick = sanitizeNickname(v);
    if (nick) out[k] = nick;
  }
  return out;
}
