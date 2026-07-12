import { describe, expect, expectTypeOf, it } from 'vitest';
import type { FriendEntry, PublicPetProjection, VisitLease } from '../platform/types';
import type { CodexPet } from './codex-pet';
import {
  allowedHomeActions,
  announceableMemory,
  deriveHomePetIdentity,
  deriveLocalAgentState,
  derivePresence,
  deriveVisitRequest,
  type FriendSummary,
  friendDisplayName,
  friendSummaries,
  isOfficialPetId,
  MEMORY_ANNOUNCE_WINDOW_MS,
  memoryCardCopy,
  memorySummaries,
  type SocialHomeModel,
  selectFriendAction,
  selectHomeEvent,
} from './social-home';

const momoFriend: FriendSummary = {
  id: 'friend-momo',
  displayName: 'Momo',
  handle: '@momo',
  pet: { id: 'solu', name: 'Solu', officialPetId: 'solu' },
  availability: 'available',
  publicAgentState: 'idle',
  visitDirection: 'visit-them',
};

const base: SocialHomeModel = {
  localPet: { id: 'muru', name: 'Muru', officialPetId: 'muru' },
  presence: { kind: 'home', visitor: null },
  agentState: 'idle',
  realtimeState: 'connected',
  affection: 86,
  coins: 140,
  togetherDays: 23,
  growthCurrent: 320,
  growthTarget: 500,
  friends: [],
  pendingVisit: null,
  latestMemory: null,
  memories: [],
};

const customMuru: CodexPet = {
  id: 'muru',
  displayName: 'Cloud Muru',
  description: 'A custom pet whose id collides with an official poster id.',
  spritesheetUrl: 'codexpet://custom/muru/spritesheet.png',
  atlas: { cellW: 48, cellH: 48, cols: 4, rows: 4 },
  animations: { idle: { row: 0, frames: 4 } },
  stateMap: {
    idle: 'idle',
    working: 'idle',
    compacting: 'idle',
    waiting: 'idle',
  },
  oneShot: new Set(),
  imageRendering: 'pixelated',
};

describe('social Home model', () => {
  it('uses official poster identity when no resolved pet overrides the selected id', () => {
    expect(deriveHomePetIdentity('solu', null, null, 'Solu')).toEqual({
      id: 'solu',
      name: 'Solu',
      officialPetId: 'solu',
    });
  });

  it('keeps custom sprite provenance when its id collides with an official id', () => {
    expect(deriveHomePetIdentity('muru', customMuru, 'Storm', 'Muru')).toEqual({
      id: 'muru',
      name: 'Storm',
    });
  });

  it('keeps the current desktop identity for legacy pet ids', () => {
    expect(
      deriveHomePetIdentity('yoonie', { ...customMuru, id: 'yoonie' }, 'Bean', 'Muru'),
    ).toEqual({
      id: 'yoonie',
      name: 'Bean',
    });
  });

  it('prioritizes an incoming visit over a memory card', () => {
    const model = {
      ...base,
      friends: [momoFriend],
      pendingVisit: {
        id: 'visit-1',
        friendId: 'friend-momo',
        ownerName: 'Momo',
        pet: { id: 'solu', name: 'Solu', officialPetId: 'solu' },
      },
      latestMemory: {
        id: 'memory-1',
        templateKey: 'played_together',
        params: { durationBucket: 'short', timeOfDay: 'morning', interactionCount: 4 },
        occurredAt: Date.UTC(2026, 6, 10),
        petIds: ['muru', 'solu'],
      },
    } satisfies SocialHomeModel;
    expect(selectHomeEvent(model)?.kind).toBe('visit-request');
  });

  it('does not expose a visit request whose stable friend id is not mutual', () => {
    const model = {
      ...base,
      friends: [momoFriend],
      pendingVisit: {
        id: 'visit-stranger',
        friendId: 'friend-stranger',
        ownerName: 'Momo',
        pet: { id: 'luma', name: 'Luma', officialPetId: 'luma' },
      },
    } satisfies SocialHomeModel;

    expect(selectHomeEvent(model)?.kind).not.toBe('visit-request');
  });

  it('accepts a mutual visit request only while the local pet is home without a visitor', () => {
    const pendingVisit = {
      id: 'visit-1',
      friendId: momoFriend.id,
      ownerName: momoFriend.displayName,
      pet: momoFriend.pet,
    };
    const clearHome = { ...base, friends: [momoFriend], pendingVisit } satisfies SocialHomeModel;
    const hosting = {
      ...clearHome,
      presence: {
        kind: 'home',
        visitor: momoFriend.pet,
        visitorOwnerName: momoFriend.displayName,
        visitorAgentState: 'idle',
        endsAt: '16:30',
        leaseMinutes: 30,
      },
    } satisfies SocialHomeModel;
    const away = {
      ...clearHome,
      presence: {
        kind: 'away',
        friendId: momoFriend.id,
        friendName: 'Momo',
        endsAt: '16:30',
        leaseMinutes: 30,
      },
    } satisfies SocialHomeModel;

    expect(selectHomeEvent(clearHome)?.kind).toBe('visit-request');
    expect(selectHomeEvent(hosting)?.kind).not.toBe('visit-request');
    expect(selectHomeEvent(away)?.kind).not.toBe('visit-request');
  });

  it('centralizes Visit, Invite, Recall, and disabled friend-row semantics', () => {
    const request = {
      id: 'visit-1',
      friendId: momoFriend.id,
      ownerName: momoFriend.displayName,
      pet: momoFriend.pet,
    };

    expect(selectFriendAction(base.presence, momoFriend, null)).toEqual({
      kind: 'visit',
      disabledReason: null,
    });
    expect(selectFriendAction(base.presence, momoFriend, request)).toEqual({
      kind: 'visit',
      disabledReason: 'already-requested',
    });
    expect(
      selectFriendAction(
        base.presence,
        { ...momoFriend, id: 'friend-invite', visitDirection: 'invite-over' },
        null,
      ),
    ).toEqual({ kind: 'invite', disabledReason: null });
    expect(
      selectFriendAction(
        {
          kind: 'away',
          friendId: momoFriend.id,
          friendName: 'Momo Renamed',
          endsAt: '16:30',
          leaseMinutes: 30,
        },
        { ...momoFriend, displayName: 'New Momo Name' },
        null,
      ),
    ).toEqual({ kind: 'recall', disabledReason: null });
    expect(
      selectFriendAction(
        {
          kind: 'away',
          friendId: 'friend-other',
          friendName: 'Momo',
          endsAt: '16:30',
          leaseMinutes: 30,
        },
        { ...momoFriend, id: 'friend-duplicate-name', displayName: 'Momo' },
        null,
      ),
    ).toEqual({ kind: 'recall', disabledReason: 'away-elsewhere' });
    expect(
      selectFriendAction(
        {
          kind: 'home',
          visitor: momoFriend.pet,
          visitorOwnerName: 'Momo',
          visitorAgentState: 'idle',
          endsAt: '16:30',
          leaseMinutes: 30,
        },
        momoFriend,
        null,
      ),
    ).toEqual({ kind: 'visit', disabledReason: 'hosting' });
    expect(
      selectFriendAction(base.presence, { ...momoFriend, availability: 'offline' }, null),
    ).toEqual({ kind: 'visit', disabledReason: 'friend-offline' });
    expect(
      selectFriendAction(base.presence, { ...momoFriend, availability: 'visiting' }, null),
    ).toEqual({ kind: 'visit', disabledReason: 'friend-busy' });
  });

  // 网络边界的模板允许清单校验在平台层（toSharedMemoryEntry，见 supabase-client.test）；
  // 这里测契约行 → 相册摘要的纯映射与展示派生。

  it('maps memory entries to album summaries with resolved participant names', () => {
    const entries = [
      {
        id: 'memory-1',
        visitId: 'visit-1',
        visitorUserId: 'user-me',
        hostUserId: 'user-momo',
        templateKey: 'played_together' as const,
        params: {
          durationBucket: 'short' as const,
          timeOfDay: 'morning' as const,
          interactionCount: 4,
        },
        createdAt: '2026-07-10T08:00:00.000Z',
      },
      {
        // created_at 不可解析的行丢弃（fail-closed）
        id: 'memory-bad',
        visitId: 'visit-2',
        visitorUserId: 'user-momo',
        hostUserId: 'user-me',
        templateKey: 'shared_snack' as const,
        params: {
          durationBucket: 'short' as const,
          timeOfDay: 'night' as const,
          interactionCount: 1,
        },
        createdAt: 'not-a-date',
      },
    ];
    const summaries = memorySummaries(entries, 'user-me', 'Muru', (id) =>
      id === 'user-momo' ? 'Momo' : '?',
    );
    expect(summaries).toEqual([
      {
        id: 'memory-1',
        occurredAt: Date.parse('2026-07-10T08:00:00.000Z'),
        petIds: ['Muru', 'Momo'],
        templateKey: 'played_together',
        params: { durationBucket: 'short', timeOfDay: 'morning', interactionCount: 4 },
      },
    ]);
  });

  it('announces only a fresh, undismissed memory on the event card', () => {
    const nowMs = Date.UTC(2026, 6, 12);
    const memory = {
      id: 'memory-1',
      occurredAt: nowMs - 60_000,
      petIds: ['Muru', 'Momo'],
      templateKey: 'played_together' as const,
      params: {
        durationBucket: 'short' as const,
        timeOfDay: 'morning' as const,
        interactionCount: 4,
      },
    };
    expect(announceableMemory([memory], null, nowMs)?.id).toBe('memory-1');
    // 打开过即本地收起
    expect(announceableMemory([memory], 'memory-1', nowMs)).toBeNull();
    // 超出播报窗口只进相册不上卡
    const stale = { ...memory, occurredAt: nowMs - MEMORY_ANNOUNCE_WINDOW_MS - 1 };
    expect(announceableMemory([stale], null, nowMs)).toBeNull();
    // 多条时报最新
    const older = { ...memory, id: 'memory-0', occurredAt: memory.occurredAt - 1 };
    expect(announceableMemory([older, memory], null, nowMs)?.id).toBe('memory-1');
  });

  it('renders memory card copy from the shared contract tables in both locales', () => {
    const memory = {
      templateKey: 'played_together' as const,
      params: {
        durationBucket: 'short' as const,
        timeOfDay: 'morning' as const,
        interactionCount: 4,
      },
    };
    expect(memoryCardCopy(memory, 'zh-CN')).toEqual({
      title: '一起串门',
      body: '她们在一个短短的早晨一起玩，留下了4个小瞬间。',
    });
    const en = memoryCardCopy(memory, 'en');
    expect(en.title).toBe('A visit together');
    expect(en.body).toBe('They played through a little morning and shared 4 little moments.');
    // 未知/缺省 locale 回落英文
    expect(memoryCardCopy(memory, null).title).toBe('A visit together');
  });

  it('never renders the local pet body while away', () => {
    const model = {
      ...base,
      presence: {
        kind: 'away',
        friendId: momoFriend.id,
        friendName: 'Momo',
        endsAt: '16:30',
        leaseMinutes: 30,
      },
    } satisfies SocialHomeModel;
    expect(allowedHomeActions(model)).toEqual(['view-visit', 'recall']);
  });

  it('requires a fixed lease for hosted and away visits', () => {
    const hosted = {
      ...base,
      presence: {
        kind: 'home',
        visitor: { id: 'solu', name: 'Solu', officialPetId: 'solu' },
        visitorOwnerName: 'Momo',
        visitorAgentState: 'idle',
        endsAt: '16:30',
        leaseMinutes: 30,
      },
    } satisfies SocialHomeModel;
    const away = {
      ...base,
      presence: {
        kind: 'away',
        friendId: momoFriend.id,
        friendName: 'Momo',
        endsAt: '16:30',
        leaseMinutes: 30,
      },
    } satisfies SocialHomeModel;
    const leaseLessHosted = {
      ...base,
      presence: {
        kind: 'home',
        visitor: { id: 'solu', name: 'Solu', officialPetId: 'solu' },
        visitorOwnerName: 'Momo',
        endsAt: '16:30',
      } as const,
    };
    const wrongLeaseAway = {
      ...base,
      presence: {
        kind: 'away',
        friendId: momoFriend.id,
        friendName: 'Momo',
        endsAt: '16:30',
        leaseMinutes: 45,
      } as const,
    };

    expectTypeOf(hosted.presence.leaseMinutes).toEqualTypeOf<30>();
    expectTypeOf(away.presence.leaseMinutes).toEqualTypeOf<30>();
    expectTypeOf(leaseLessHosted).not.toMatchTypeOf<SocialHomeModel>();
    expectTypeOf(wrongLeaseAway).not.toMatchTypeOf<SocialHomeModel>();
    expect([hosted.presence.leaseMinutes, away.presence.leaseMinutes]).toEqual([30, 30]);
  });

  it('accepts only official ids and derives only safe agent enums', () => {
    expect(isOfficialPetId('muru')).toBe(true);
    expect(isOfficialPetId('yoonie')).toBe(false);
    const quiet = { waiting: 0, compacting: 0, working: 0 };
    expect(deriveLocalAgentState(false, quiet, false)).toBe('offline');
    expect(deriveLocalAgentState(true, { ...quiet, waiting: 1 }, true)).toBe('waiting');
    expect(deriveLocalAgentState(true, { ...quiet, compacting: 1 }, false)).toBe('compacting');
    expect(deriveLocalAgentState(true, quiet, true)).toBe('working');
    expect(deriveLocalAgentState(true, quiet, false)).toBe('idle');
  });
});

// ---------- 平台契约 → Home 模型（W7 换线）----------

const acceptedEntry: FriendEntry = {
  userId: 'user-momo',
  handle: 'user-abc123',
  displayName: 'Momo',
  relation: 'accepted',
  muted: false,
};

function lease(overrides: Partial<VisitLease>): VisitLease {
  return {
    id: 'visit-1',
    visitorUserId: 'user-momo',
    hostUserId: 'me',
    status: 'requested',
    startedAt: null,
    endsAt: null,
    ...overrides,
  };
}

const guestFrame: PublicPetProjection = {
  v: 1,
  petId: 'solu',
  displayName: 'Solu',
  skinId: 'solu',
  status: 'working',
  updatedAt: '2026-07-11T10:00:00Z',
};

const nameOf = (userId: string) => (userId === 'user-momo' ? 'Momo' : null);

describe('friendSummaries', () => {
  it('prefers displayName and falls back to the anonymous handle', () => {
    expect(friendDisplayName(acceptedEntry)).toBe('Momo');
    expect(friendDisplayName({ ...acceptedEntry, displayName: null })).toBe('user-abc123');
  });

  it('maps accepted entries only, with owner-name pet placeholder', () => {
    const summaries = friendSummaries([
      acceptedEntry,
      { ...acceptedEntry, userId: 'user-out', relation: 'pending_out' },
      { ...acceptedEntry, userId: 'user-in', relation: 'pending_in' },
    ]);
    expect(summaries).toHaveLength(1);
    expect(summaries[0].id).toBe('user-momo');
    expect(summaries[0].displayName).toBe('Momo');
    expect(summaries[0].pet).toEqual({ id: 'user-momo', name: 'Momo' });
    expect(summaries[0].visitDirection).toBe('visit-them');
  });

  it('keeps the visit entry actionable despite unknown presence', () => {
    // v1 契约没有好友在线状态：占位必须让 selectFriendAction 给出可用的 visit
    const [summary] = friendSummaries([acceptedEntry]);
    const action = selectFriendAction({ kind: 'home', visitor: null }, summary, null);
    expect(action).toEqual({ kind: 'visit', disabledReason: null });
  });
});

describe('derivePresence', () => {
  it('defaults to an empty home', () => {
    expect(derivePresence(null, 'none', null, 'none', null, nameOf)).toEqual({
      kind: 'home',
      visitor: null,
    });
  });

  it('shows the guest from the projection while an inbound visit is live', () => {
    const inbound = lease({ status: 'visiting', endsAt: '2026-07-11T10:30:00Z' });
    const presence = derivePresence(null, 'none', inbound, 'visiting', guestFrame, nameOf);
    expect(presence).toEqual({
      kind: 'home',
      visitor: { id: 'solu', name: 'Solu' },
      visitorOwnerName: 'Momo',
      visitorAgentState: 'working',
      endsAt: '2026-07-11T10:30:00Z',
      leaseMinutes: 30,
    });
  });

  it('stays home until the first projection frame lands', () => {
    const inbound = lease({ status: 'visiting' });
    expect(derivePresence(null, 'none', inbound, 'visiting', null, nameOf)).toEqual({
      kind: 'home',
      visitor: null,
    });
  });

  it('marks away while the own pet is out, but keeps pending at home', () => {
    const outbound = lease({
      visitorUserId: 'me',
      hostUserId: 'user-momo',
      status: 'traveling',
      endsAt: '2026-07-11T10:30:00Z',
    });
    expect(derivePresence(outbound, 'traveling', null, 'none', null, nameOf)).toEqual({
      kind: 'away',
      friendId: 'user-momo',
      friendName: 'Momo',
      endsAt: '2026-07-11T10:30:00Z',
      leaseMinutes: 30,
    });
    expect(
      derivePresence(
        lease({ visitorUserId: 'me', hostUserId: 'user-momo' }),
        'pending',
        null,
        'none',
        null,
        nameOf,
      ),
    ).toEqual({ kind: 'home', visitor: null });
  });

  it('prefers the guest at home over the own pet being away', () => {
    const outbound = lease({
      id: 'v-out',
      visitorUserId: 'me',
      hostUserId: 'user-momo',
      status: 'visiting',
    });
    const inbound = lease({ id: 'v-in', status: 'visiting', endsAt: '2026-07-11T10:30:00Z' });
    const presence = derivePresence(outbound, 'visiting', inbound, 'visiting', guestFrame, nameOf);
    expect(presence).toMatchObject({ kind: 'home', visitor: { id: 'solu', name: 'Solu' } });
  });
});

describe('deriveVisitRequest', () => {
  it('surfaces a pending inbound lease with empty pet identity', () => {
    expect(deriveVisitRequest(lease({}), 'pending', nameOf)).toEqual({
      id: 'visit-1',
      friendId: 'user-momo',
      ownerName: 'Momo',
      pet: { id: 'user-momo', name: '' },
    });
  });

  it('withholds the card until the requester resolves to a known friend', () => {
    expect(
      deriveVisitRequest(lease({ visitorUserId: 'user-stranger' }), 'pending', nameOf),
    ).toBeNull();
  });

  it('returns null outside the pending phase', () => {
    expect(deriveVisitRequest(null, 'none', nameOf)).toBeNull();
    expect(deriveVisitRequest(lease({ status: 'visiting' }), 'visiting', nameOf)).toBeNull();
  });
});
