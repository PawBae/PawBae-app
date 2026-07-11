import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  LEASE_POLL_INTERVAL_MS,
  parseProjectionFrame,
  type SupabaseLike,
  SupabasePlatformClient,
  toVisitLease,
} from './supabase-client';
import type { VisitLease } from './types';

// ---------- 假 supabase client ----------

type Row = Record<string, unknown>;

function visitRow(overrides: Row = {}): Row {
  return {
    id: 'visit-1',
    visitor_user_id: 'user-me',
    host_user_id: 'user-keyu',
    status: 'requested',
    started_at: null,
    ends_at: null,
    ...overrides,
  };
}

class FakeChannel {
  handlers = new Map<string, (message: { payload: unknown }) => void>();
  subscribed = false;
  constructor(
    readonly topic: string,
    readonly config: { private: boolean },
  ) {}
  on(_type: 'broadcast', filter: { event: string }, cb: (m: { payload: unknown }) => void) {
    this.handlers.set(filter.event, cb);
    return this;
  }
  subscribe() {
    this.subscribed = true;
    return this;
  }
  emit(event: string, payload: unknown) {
    this.handlers.get(event)?.({ payload });
  }
}

function fakeSupabase() {
  const state = {
    rpcCalls: [] as Array<{ name: string; params: Record<string, unknown> | undefined }>,
    rpcResult: { data: visitRow() as unknown, error: null as { message?: string } | null },
    tables: new Map<string, Row[]>([
      ['visits', []],
      ['friendships', []],
      ['profiles', []],
      ['friend_mutes', []],
      ['pet_projections', []],
    ]),
    channels: [] as FakeChannel[],
    removed: [] as FakeChannel[],
    selectCount: new Map<string, number>(),
    /** visits 查询挂起控制：设为 Promise 可模拟慢请求 */
    visitsGate: null as Promise<void> | null,
    session: null as unknown,
    authCallbacks: [] as Array<(event: string, session: unknown) => void>,
  };

  function makeBuilder(table: string) {
    const filters: Array<(r: Row) => boolean> = [];
    const run = async () => {
      state.selectCount.set(table, (state.selectCount.get(table) ?? 0) + 1);
      if (table === 'visits' && state.visitsGate) await state.visitsGate;
      const rows = (state.tables.get(table) ?? []).filter((r) => filters.every((f) => f(r)));
      return { data: rows, error: null };
    };
    const builder = {
      order: () => builder,
      limit: () => builder,
      eq(column: string, value: string) {
        filters.push((r) => r[column] === value);
        return builder;
      },
      in(column: string, values: readonly string[]) {
        filters.push((r) => values.includes(r[column] as string));
        return builder;
      },
      maybeSingle: async () => {
        const { data } = await run();
        return { data: (data as Row[])[0] ?? null, error: null };
      },
      // biome-ignore lint/suspicious/noThenProperty: supabase-js 的查询构造器就是 thenable，假件需同形
      then(onFulfilled: (v: { data: unknown; error: unknown }) => unknown) {
        return run().then(onFulfilled);
      },
    };
    return builder;
  }

  const client: SupabaseLike = {
    rpc: async (name, params) => {
      state.rpcCalls.push({ name, params });
      return state.rpcResult;
    },
    from: (table: string) => ({ select: () => makeBuilder(table) }) as never,
    channel(topic, opts) {
      const ch = new FakeChannel(topic, opts.config);
      state.channels.push(ch);
      return ch;
    },
    removeChannel(ch) {
      state.removed.push(ch as FakeChannel);
    },
    auth: {
      getSession: async () => ({ data: { session: state.session } }),
      onAuthStateChange(cb) {
        state.authCallbacks.push(cb);
        return { data: { subscription: { unsubscribe: () => {} } } };
      },
    },
  };
  return { client, state };
}

/** 造一个 supabase-js Session 形状的最小对象（auth.ts 的 toPlatformSession 消费）。 */
function fakeAuthSession(userId = 'user-me') {
  return {
    user: {
      id: userId,
      email: 'me@example.com',
      user_metadata: { user_name: 'yining', avatar_url: null },
    },
  };
}

async function startedClient(overrides?: { signedIn?: boolean }) {
  const { client, state } = fakeSupabase();
  if (overrides?.signedIn !== false) state.session = fakeAuthSession();
  const platform = new SupabasePlatformClient(() => client);
  await platform.start();
  return { platform, state };
}

const LEASE: VisitLease = Object.freeze({
  id: 'visit-1',
  visitorUserId: 'user-guest',
  hostUserId: 'user-me',
  status: 'visiting',
  startedAt: '2026-07-11T00:00:00.000Z',
  endsAt: '2026-07-11T00:30:00.000Z',
});

describe('toVisitLease', () => {
  it('maps a canonical snake_case row to the camelCase contract', () => {
    const lease = toVisitLease(
      visitRow({
        status: 'visiting',
        started_at: '2026-07-11T00:00:00Z',
        ends_at: '2026-07-11T00:30:00Z',
      }),
    );
    expect(lease).toEqual({
      id: 'visit-1',
      visitorUserId: 'user-me',
      hostUserId: 'user-keyu',
      status: 'visiting',
      startedAt: '2026-07-11T00:00:00Z',
      endsAt: '2026-07-11T00:30:00Z',
    });
  });

  it('rejects unknown statuses and malformed rows', () => {
    expect(() => toVisitLease(visitRow({ status: 'partying' }))).toThrow(TypeError);
    expect(() => toVisitLease(null)).toThrow(TypeError);
    expect(() => toVisitLease(visitRow({ id: 42 }))).toThrow(TypeError);
  });
});

describe('visit RPCs', () => {
  it('requestVisit calls request_visit with PostgREST arg names and emits the lease', async () => {
    const { platform, state } = await startedClient();
    const seen: VisitLease[] = [];
    platform.onLeaseChange((l) => seen.push(l));
    const lease = await platform.requestVisit('user-keyu', 'idem-1');
    expect(state.rpcCalls.at(-1)).toEqual({
      name: 'request_visit',
      params: { p_host_user_id: 'user-keyu', p_idempotency_key: 'idem-1' },
    });
    expect(lease.status).toBe('requested');
    expect(seen.map((l) => l.id)).toContain('visit-1');
  });

  it('respondVisit maps accept/decline to their two RPCs', async () => {
    const { platform, state } = await startedClient();
    state.rpcResult = { data: visitRow({ status: 'accepted' }), error: null };
    await platform.respondVisit('visit-1', 'accept', 'k1');
    state.rpcResult = { data: visitRow({ status: 'declined' }), error: null };
    await platform.respondVisit('visit-1', 'decline', 'k2');
    expect(state.rpcCalls.map((c) => c.name)).toEqual(['accept_visit', 'decline_visit']);
    expect(state.rpcCalls[0].params).toEqual({ p_visit_id: 'visit-1', p_idempotency_key: 'k1' });
  });

  it('cancel/recall/end map to their RPCs', async () => {
    const { platform, state } = await startedClient();
    state.rpcResult = { data: visitRow({ status: 'cancelled' }), error: null };
    await platform.cancelVisit('visit-1', 'k1');
    state.rpcResult = { data: visitRow({ status: 'recalled' }), error: null };
    await platform.recallVisit('visit-1', 'k2');
    state.rpcResult = { data: visitRow({ status: 'completed' }), error: null };
    await platform.endVisit('visit-1', 'k3');
    expect(state.rpcCalls.map((c) => c.name)).toEqual([
      'cancel_visit',
      'recall_visit',
      'end_visit',
    ]);
  });

  it('throws the server message on RPC error and emits nothing', async () => {
    const { platform, state } = await startedClient();
    state.rpcResult = { data: null, error: { message: 'host already has an active visit' } };
    const seen: VisitLease[] = [];
    platform.onLeaseChange((l) => seen.push(l));
    await expect(platform.requestVisit('user-keyu', 'k')).rejects.toThrow(
      'host already has an active visit',
    );
    expect(seen).toHaveLength(0);
  });

  it('throws PLATFORM_NOT_CONFIGURED without env config', async () => {
    const platform = new SupabasePlatformClient(() => null);
    await platform.start();
    expect(platform.session()).toBeNull();
    await expect(platform.requestVisit('user-keyu', 'k')).rejects.toThrow(
      'PLATFORM_NOT_CONFIGURED',
    );
    await expect(platform.friends()).resolves.toEqual([]);
  });
});

describe('lease polling', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('polls while signed in with a listener, emits changed rows only', async () => {
    const { platform, state } = await startedClient();
    state.tables.set('visits', [visitRow({ status: 'requested' })]);
    const seen: VisitLease[] = [];
    platform.onLeaseChange((l) => seen.push(l)); // 开闸沿先拉一次
    await vi.advanceTimersByTimeAsync(0);
    expect(seen.map((l) => l.status)).toEqual(['requested']);

    await vi.advanceTimersByTimeAsync(LEASE_POLL_INTERVAL_MS); // 内容没变：不重发
    expect(seen).toHaveLength(1);

    state.tables.set('visits', [
      visitRow({
        status: 'accepted',
        started_at: '2026-07-11T00:00:00Z',
        ends_at: '2026-07-11T00:30:00Z',
      }),
    ]);
    await vi.advanceTimersByTimeAsync(LEASE_POLL_INTERVAL_MS);
    expect(seen.map((l) => l.status)).toEqual(['requested', 'accepted']);
  });

  it('does not poll when signed out, stops when the last listener unsubscribes', async () => {
    const { platform, state } = await startedClient({ signedIn: false });
    const unsub = platform.onLeaseChange(() => {});
    await vi.advanceTimersByTimeAsync(LEASE_POLL_INTERVAL_MS * 2);
    expect(state.selectCount.get('visits') ?? 0).toBe(0);

    state.authCallbacks[0]('SIGNED_IN', fakeAuthSession()); // 登录开闸
    await vi.advanceTimersByTimeAsync(0);
    expect(state.selectCount.get('visits')).toBe(1);

    unsub();
    await vi.advanceTimersByTimeAsync(LEASE_POLL_INTERVAL_MS * 3);
    expect(state.selectCount.get('visits')).toBe(1);
  });

  it('busy lock: a slow poll is never overlapped by the next tick', async () => {
    const { platform, state } = await startedClient();
    let release!: () => void;
    state.visitsGate = new Promise((r) => {
      release = r;
    });
    platform.onLeaseChange(() => {});
    await vi.advanceTimersByTimeAsync(LEASE_POLL_INTERVAL_MS * 3); // 首拉挂起，3 个 tick 全跳过
    expect(state.selectCount.get('visits')).toBe(1);
    state.visitsGate = null;
    release();
    await vi.advanceTimersByTimeAsync(LEASE_POLL_INTERVAL_MS);
    expect(state.selectCount.get('visits')).toBe(2);
  });
});

describe('guest projection subscription', () => {
  it('joins the lease-scoped private topic and replays the current projection', async () => {
    const { platform, state } = await startedClient();
    state.tables.set('pet_projections', [
      {
        owner_user_id: 'user-guest',
        version: 1,
        pet_id: 'pet-bobo',
        display_name: 'Bobo',
        skin_id: 'yoonie',
        status: 'working',
        updated_at: '2026-07-11T00:01:00.000Z',
      },
    ]);
    const seen: string[] = [];
    platform.subscribeGuestProjection(LEASE, (p) => seen.push(p.status));
    await Promise.resolve();
    await Promise.resolve();

    const channel = state.channels[0];
    expect(channel.topic).toBe('pet:user-guest:visit-1');
    expect(channel.config).toEqual({ private: true });
    expect(channel.subscribed).toBe(true);
    expect(seen).toEqual(['working']); // 初始回放
  });

  it('delivers broadcast frames after stripping the Realtime transport id, drops malformed ones', async () => {
    const { platform, state } = await startedClient();
    const seen: string[] = [];
    const unsub = platform.subscribeGuestProjection(LEASE, (p) => seen.push(p.status));
    const channel = state.channels[0];

    channel.emit('projection_updated', {
      id: 'b9dc1c94-0000-0000-0000-000000000000', // Realtime 注入的 transport 元数据
      v: 1,
      petId: 'pet-bobo',
      displayName: 'Bobo',
      skinId: 'yoonie',
      status: 'waiting',
      updatedAt: '2026-07-11T00:02:00.000Z',
    });
    channel.emit('projection_updated', { v: 1, status: 'working' }); // 缺键：丢帧
    expect(seen).toEqual(['waiting']);

    unsub();
    expect(state.removed).toContain(channel);
  });

  it('a visit_ended frame surfaces through onLeaseChange with the terminal status', async () => {
    const { platform, state } = await startedClient();
    const seen: VisitLease[] = [];
    platform.onLeaseChange((l) => seen.push(l));
    platform.subscribeGuestProjection(LEASE, () => {});
    state.channels[0].emit('visit_ended', {
      leaseId: 'visit-1',
      status: 'recalled',
      endedAt: '2026-07-11T00:10:00.000Z',
    });
    expect(seen.at(-1)).toMatchObject({ id: 'visit-1', status: 'recalled' });
    // 时间戳保持底稿值：正确性来源是 visits 行，广播只推终态
    expect(seen.at(-1)?.endsAt).toBe(LEASE.endsAt);
  });
});

describe('invites and friends', () => {
  it('redeemInvite calls redeem_invite and throws on server rejection', async () => {
    const { platform, state } = await startedClient();
    state.rpcResult = { data: { id: 'redemption-1' }, error: null };
    await platform.redeemInvite('CODE-123', 'k1');
    expect(state.rpcCalls.at(-1)).toEqual({
      name: 'redeem_invite',
      params: { p_code: 'CODE-123', p_idempotency_key: 'k1' },
    });
    state.rpcResult = { data: null, error: { message: 'invite code is not valid' } };
    await expect(platform.redeemInvite('BAD', 'k2')).rejects.toThrow('invite code is not valid');
  });

  it('composes friendships + profiles + mutes into FriendEntry list', async () => {
    const { platform, state } = await startedClient();
    state.tables.set('friendships', [
      { user_a: 'user-keyu', user_b: 'user-me', requester_id: 'user-me', status: 'accepted' },
      { user_a: 'user-me', user_b: 'user-sarah', requester_id: 'user-me', status: 'pending' },
      { user_a: 'user-ben', user_b: 'user-me', requester_id: 'user-ben', status: 'pending' },
    ]);
    state.tables.set('profiles', [
      { id: 'user-keyu', handle: 'keyu', display_name: 'Keyu' },
      { id: 'user-sarah', handle: 'sarahk', display_name: null },
      { id: 'user-ben', handle: 'ben', display_name: 'Ben' },
    ]);
    state.tables.set('friend_mutes', [{ muted_user_id: 'user-keyu', muted: true }]);

    await expect(platform.friends()).resolves.toEqual([
      {
        userId: 'user-ben',
        handle: 'ben',
        displayName: 'Ben',
        relation: 'pending_in',
        muted: false,
      },
      {
        userId: 'user-keyu',
        handle: 'keyu',
        displayName: 'Keyu',
        relation: 'accepted',
        muted: true,
      },
      {
        userId: 'user-sarah',
        handle: 'sarahk',
        displayName: null,
        relation: 'pending_out',
        muted: false,
      },
    ]);
  });

  it('omits friendships whose profile is not readable (blocked peer)', async () => {
    const { platform, state } = await startedClient();
    state.tables.set('friendships', [
      { user_a: 'user-me', user_b: 'user-ghost', requester_id: 'user-ghost', status: 'accepted' },
    ]);
    await expect(platform.friends()).resolves.toEqual([]);
  });
});

describe('session surface', () => {
  it('restores the persisted session on start and broadcasts auth changes', async () => {
    const { platform, state } = await startedClient();
    expect(platform.session()).toMatchObject({ userId: 'user-me', handle: 'yining' });

    const seen: Array<string | null> = [];
    platform.onSessionChange((s) => seen.push(s?.userId ?? null));
    state.authCallbacks[0]('SIGNED_OUT', null);
    state.authCallbacks[0]('SIGNED_IN', fakeAuthSession('user-two'));
    expect(seen).toEqual([null, 'user-two']);
    expect(platform.session()?.userId).toBe('user-two');
  });

  it('token refresh for the same user does not re-broadcast', async () => {
    const { platform, state } = await startedClient();
    const seen: unknown[] = [];
    platform.onSessionChange((s) => seen.push(s));
    state.authCallbacks[0]('TOKEN_REFRESHED', fakeAuthSession('user-me'));
    expect(seen).toHaveLength(0);
    expect(platform.session()?.userId).toBe('user-me');
  });
});

describe('parseProjectionFrame', () => {
  it('returns null for non-object payloads', () => {
    expect(parseProjectionFrame(null)).toBeNull();
    expect(parseProjectionFrame('nope')).toBeNull();
  });
});
