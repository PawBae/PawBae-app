import type { Session } from '@supabase/supabase-js';
import { describe, expect, it } from 'vitest';
import { extractAuthCode, toPlatformSession } from './auth';

function fakeSession(meta: Record<string, unknown>, email?: string): Session {
  return {
    user: { id: 'uuid-1234-abcd', email, user_metadata: meta },
  } as unknown as Session;
}

describe('toPlatformSession', () => {
  it('maps github metadata to the contract shape', () => {
    const s = toPlatformSession(
      fakeSession({
        user_name: 'azealoo',
        full_name: 'Yining Zhong',
        avatar_url: 'https://avatars.githubusercontent.com/u/1',
      }),
    );
    expect(s).toEqual({
      userId: 'uuid-1234-abcd',
      handle: 'azealoo',
      displayName: 'Yining Zhong',
      avatarUrl: 'https://avatars.githubusercontent.com/u/1',
    });
  });

  it('falls back handle to email prefix then user id', () => {
    expect(toPlatformSession(fakeSession({}, 'rex@example.com'))?.handle).toBe('rex');
    expect(toPlatformSession(fakeSession({}))?.handle).toBe('uuid-123');
  });

  it('null session maps to null', () => {
    expect(toPlatformSession(null)).toBeNull();
  });

  it('empty-string metadata never leaks into displayName/avatar', () => {
    const s = toPlatformSession(fakeSession({ user_name: 'x', full_name: '', avatar_url: '' }));
    expect(s?.displayName).toBeNull();
    expect(s?.avatarUrl).toBeNull();
  });
});

describe('extractAuthCode', () => {
  it('extracts the code param', () => {
    expect(extractAuthCode('code=abc123&state=xyz')).toBe('abc123');
  });

  it('throws the provider error message when present', () => {
    expect(() => extractAuthCode('error=access_denied&error_description=User+cancelled')).toThrow(
      'User cancelled',
    );
  });

  it('throws when no code came back', () => {
    expect(() => extractAuthCode('state=only')).toThrow(/no code/);
  });
});
