import { describe, expect, expectTypeOf, it } from 'vitest';
import type { CodexPet } from './codex-pet';
import {
  allowedHomeActions,
  deriveHomePetIdentity,
  deriveLocalAgentState,
  type FriendSummary,
  isOfficialPetId,
  parseSharedMemory,
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
        templateKey: 'rainy-tea',
        params: {},
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

  it('rejects arbitrary shared-memory titles and accepts only allowlisted templates', () => {
    expect(
      parseSharedMemory({
        id: 'unsafe',
        title: '/Users/alice/secret-project prompt: deploy prod',
        occurredAt: Date.UTC(2026, 6, 10),
        petIds: ['muru', 'solu'],
      }),
    ).toBeNull();
    expect(
      parseSharedMemory({
        id: 'memory-1',
        templateKey: 'rainy-tea',
        params: {},
        occurredAt: Date.UTC(2026, 6, 10),
        petIds: ['muru', 'solu'],
      }),
    ).toEqual({
      id: 'memory-1',
      templateKey: 'rainy-tea',
      params: {},
      occurredAt: Date.UTC(2026, 6, 10),
      petIds: ['muru', 'solu'],
    });
    expect(
      parseSharedMemory({
        id: 'memory-2',
        templateKey: 'shared-photo',
        params: { photoCount: '/tmp/private' },
        occurredAt: Date.UTC(2026, 6, 10),
        petIds: ['muru', 'solu'],
      }),
    ).toBeNull();
    expect(
      parseSharedMemory({
        id: 'memory-3',
        templateKey: 'rainy-tea',
        params: {},
        occurredAt: Date.UTC(2026, 6, 10),
        petIds: ['muru', '/Users/alice/private-task.md'],
      }),
    ).toBeNull();
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
