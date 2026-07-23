// MockPlatformClient 测试：虚拟时钟驱动完整租约生命周期、幂等回放、
// 单活动访问、婉拒/过期/召回剧本、投影脚本与「发送端守门」停播语义。
import { describe, expect, it } from 'vitest';

import { MockPlatformClient } from './mock';
import type { ProjectionStatus, VisitStatus } from './types';

const MIN = 60_000;

function collectStatuses(mock: MockPlatformClient): VisitStatus[] {
  const seen: VisitStatus[] = [];
  mock.onLeaseChange((l) => seen.push(l.status));
  return seen;
}

describe('happy path lifecycle', () => {
  it('walks requested → accepted → traveling → visiting → returning → completed on the virtual clock', async () => {
    const mock = new MockPlatformClient();
    const seen = collectStatuses(mock);

    const lease = await mock.requestVisit('user-keyu', 'key-req-1');
    expect(lease.status).toBe('requested');
    expect(lease.startedAt).toBeNull();

    mock.advance(1_000); // 对方接受（默认 respondDelayMs）
    mock.advance(3_500); // 离家过场（默认 travelMs，拍板 A 档）
    mock.advance(30 * MIN + 3_500); // 30 分钟租约 + 归家过场

    expect(seen).toEqual([
      'requested',
      'accepted',
      'traveling',
      'visiting',
      'returning',
      'completed',
    ]);
  });

  it('stamps startedAt/endsAt exactly 30 minutes apart at accept time', async () => {
    const mock = new MockPlatformClient();
    let latest: { startedAt: string | null; endsAt: string | null } | null = null;
    mock.onLeaseChange((l) => {
      if (l.status === 'accepted') latest = { startedAt: l.startedAt, endsAt: l.endsAt };
    });
    await mock.requestVisit('user-keyu', 'key-req-2');
    mock.advance(1_000);
    expect(latest).not.toBeNull();
    const { startedAt, endsAt } = latest as unknown as { startedAt: string; endsAt: string };
    expect(Date.parse(endsAt) - Date.parse(startedAt)).toBe(30 * MIN);
  });
});

describe('idempotency and single active visit', () => {
  it('replays the same lease for a repeated idempotency key instead of creating a duplicate', async () => {
    const mock = new MockPlatformClient();
    const first = await mock.requestVisit('user-keyu', 'key-same');
    mock.advance(1_000);
    const replay = await mock.requestVisit('user-keyu', 'key-same');
    expect(replay.id).toBe(first.id);
    expect(replay.status).toBe('traveling'); // 回放返回当前状态（accept 后已进过场），而不是新租约
  });

  it('rejects a second visit with a fresh key while one is active（部分唯一索引语义）', async () => {
    const mock = new MockPlatformClient();
    await mock.requestVisit('user-keyu', 'key-a');
    await expect(mock.requestVisit('user-sarahk', 'key-b')).rejects.toThrow('VISIT_ALREADY_ACTIVE');
  });

  it('allows a new visit after the previous lease ends', async () => {
    const mock = new MockPlatformClient({ autoRespond: 'decline' });
    await mock.requestVisit('user-keyu', 'key-a');
    mock.advance(1_000); // → declined
    const second = await mock.requestVisit('user-sarahk', 'key-b');
    expect(second.status).toBe('requested');
  });

  it('rejects when logged out（App 未登录也必须可运行，串门只是拒绝）', async () => {
    const mock = new MockPlatformClient({ session: null });
    await expect(mock.requestVisit('user-keyu', 'key-x')).rejects.toThrow('NOT_AUTHENTICATED');
  });
});

describe('invite and friend controls', () => {
  it('keeps invite eligibility after redemption and repeated reads', async () => {
    const mock = new MockPlatformClient();
    await expect(mock.inviteEligibility()).resolves.toEqual({
      redeemed: false,
      redeemedAt: null,
    });
    await mock.redeemInvite('CODE-123', 'invite-key');
    const first = await mock.inviteEligibility();
    const second = await mock.inviteEligibility();
    expect(first.redeemed).toBe(true);
    expect(second).toEqual(first);
  });

  it('supports exact lookup, accept, mute, remove, and block transitions', async () => {
    const mock = new MockPlatformClient();
    const incoming = await mock.findProfileByHandle('@devon');
    expect(incoming?.handle).toBe('devon');
    if (!incoming) throw new Error('mock incoming profile is missing');
    await mock.acceptFriendRequest(incoming.userId);
    await mock.muteUser(incoming.userId, true);
    expect(
      (await mock.friends()).find((friend) => friend.userId === incoming.userId),
    ).toMatchObject({ relation: 'accepted', muted: true });
    await mock.unfriend(incoming.userId);
    expect((await mock.friends()).some((friend) => friend.userId === incoming.userId)).toBe(false);

    const keyu = await mock.findProfileByHandle('keyu');
    if (!keyu) throw new Error('mock accepted profile is missing');
    await mock.blockUser(keyu.userId);
    expect((await mock.friends()).some((friend) => friend.userId === keyu.userId)).toBe(false);
  });
});

describe('decline / expiry / recall scripts（SV §6 异常剧本）', () => {
  it('declines with no further transitions', async () => {
    const mock = new MockPlatformClient({ autoRespond: 'decline' });
    const seen = collectStatuses(mock);
    await mock.requestVisit('user-keyu', 'key-1');
    mock.advance(24 * 60 * MIN); // 远超所有计时器
    expect(seen).toEqual(['requested', 'declined']);
  });

  it('expires an unanswered invite after 24h（自然过期，无负反馈）', async () => {
    const mock = new MockPlatformClient({ autoRespond: 'manual' });
    const seen = collectStatuses(mock);
    await mock.requestVisit('user-keyu', 'key-1');
    mock.advance(24 * 60 * MIN);
    expect(seen).toEqual(['requested', 'expired']);
  });

  it('recall during visiting goes returning → recalled and cancels the scheduled natural ending', async () => {
    const mock = new MockPlatformClient({ autoRespond: 'manual' });
    const seen = collectStatuses(mock);
    const lease = await mock.requestVisit('user-keyu', 'key-1');
    mock.acceptPending();
    mock.advance(4_000); // 过场结束，visiting
    await mock.recallVisit(lease.id, 'key-recall');
    mock.advance(60 * MIN); // 归家过场 + 原定到期时间点都过去
    expect(seen).toEqual([
      'requested',
      'accepted',
      'traveling',
      'visiting',
      'returning',
      'recalled',
    ]);
  });

  it('cancelVisit withdraws a still-requested invite', async () => {
    const mock = new MockPlatformClient({ autoRespond: 'manual' });
    const lease = await mock.requestVisit('user-keyu', 'key-1');
    const cancelled = await mock.cancelVisit(lease.id, 'key-cancel');
    expect(cancelled.status).toBe('cancelled');
  });
});

describe('guest projection（好友端订阅访客投影）', () => {
  it('plays the script while visiting and stops emitting after the lease ends（发送端守门）', async () => {
    const mock = new MockPlatformClient({
      projectionScript: [
        { status: 'working', ms: 5_000 },
        { status: 'idle', ms: 5_000 },
      ],
    });
    const lease = await mock.requestVisit('user-keyu', 'key-1');
    mock.advance(1_000 + 3_500); // → visiting

    const seen: ProjectionStatus[] = [];
    mock.subscribeGuestProjection(lease, (p) => seen.push(p.status));
    expect(seen).toEqual(['working']); // 订阅即回放当前投影

    mock.advance(5_000);
    expect(seen).toEqual(['working', 'idle']);

    await mock.endVisit(lease.id, 'key-end');
    const countAtEnd = seen.length;
    mock.advance(60 * MIN);
    expect(seen.length).toBe(countAtEnd); // 租约结束后不再有任何投影
  });

  it('manual override pauses the script (simulating connector offline) and resume continues it', async () => {
    const mock = new MockPlatformClient({
      projectionScript: [
        { status: 'working', ms: 5_000 },
        { status: 'idle', ms: 5_000 },
      ],
    });
    const lease = await mock.requestVisit('user-keyu', 'key-1');
    mock.advance(4_500); // → visiting

    const seen: ProjectionStatus[] = [];
    mock.subscribeGuestProjection(lease, (p) => seen.push(p.status));

    mock.setGuestProjectionStatus(lease.id, 'offline'); // 主人 agent 断线 → 客房睡觉
    expect(seen.at(-1)).toBe('offline');

    mock.advance(20_000); // 剧本暂停，不产生新状态
    expect(seen.at(-1)).toBe('offline');

    mock.resumeGuestProjectionScript(lease.id);
    mock.advance(5_000);
    expect(seen.length).toBeGreaterThan(2); // 恢复后剧本继续
  });
});
