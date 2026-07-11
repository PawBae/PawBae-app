import { describe, expect, expectTypeOf, it } from 'vitest';
import {
  APPROVED_SKIN_IDS,
  FRIEND_RELATIONS,
  PROJECTION_STATUSES,
  VISIT_STATUSES,
  type FriendEntry,
  type ProjectionStatus,
  type PublicPetProjection,
  type VisitLease,
  type VisitStatus,
} from './contracts';
import type { Database, Json } from './index';

describe('shared social contracts', () => {
  it('freezes the complete projection, visit, friend, and skin dictionaries', () => {
    expect(PROJECTION_STATUSES).toEqual(['idle', 'working', 'waiting', 'compacting', 'offline']);
    expect(VISIT_STATUSES).toEqual([
      'requested',
      'accepted',
      'traveling',
      'visiting',
      'returning',
      'completed',
      'declined',
      'cancelled',
      'expired',
      'recalled',
      'blocked',
    ]);
    expect(FRIEND_RELATIONS).toEqual(['pending_in', 'pending_out', 'accepted']);
    // 只有 #55 版权清理后存活的 3 只（D1）；四宠就绪后随 reviewed migration 同步加行
    expect(APPROVED_SKIN_IDS).toEqual(['shimeji-bola', 'wukong', 'yoonie']);
    expect(Object.isFrozen(PROJECTION_STATUSES)).toBe(true);
    expect(Object.isFrozen(VISIT_STATUSES)).toBe(true);
    expect(Object.isFrozen(FRIEND_RELATIONS)).toBe(true);
    expect(Object.isFrozen(APPROVED_SKIN_IDS)).toBe(true);
  });

  it('exposes the intended readonly public shapes', () => {
    expectTypeOf<ProjectionStatus>().toEqualTypeOf<
      'idle' | 'working' | 'waiting' | 'compacting' | 'offline'
    >();
    expectTypeOf<VisitStatus>().toEqualTypeOf<
      | 'requested'
      | 'accepted'
      | 'traveling'
      | 'visiting'
      | 'returning'
      | 'completed'
      | 'declined'
      | 'cancelled'
      | 'expired'
      | 'recalled'
      | 'blocked'
    >();
    expectTypeOf<PublicPetProjection['v']>().toEqualTypeOf<1>();
    expectTypeOf<VisitLease['endsAt']>().toEqualTypeOf<string | null>();
    expectTypeOf<FriendEntry['relation']>().toEqualTypeOf<
      'pending_in' | 'pending_out' | 'accepted'
    >();
    expectTypeOf<Database['public']['Tables']['visits']['Row']['status']>().toEqualTypeOf<
      VisitStatus
    >();
    expectTypeOf<Json>().toMatchTypeOf<unknown>();
  });
});
