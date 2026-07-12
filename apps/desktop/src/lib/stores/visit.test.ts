// VisitStore 测试：用 MockPlatformClient 驱动完整访客/接待双侧流程，
// 时钟经 nowFn 注入——最关键的用例是「服务端没发结束消息，本地照样归家」。
import { describe, expect, it, vi } from 'vitest';

import { MockPlatformClient } from '../platform/mock';
import { VisitStore } from './visit.svelte';

const MIN = 60_000;

/** mock 虚拟时钟 + 可越前的本地时钟（模拟「服务端没跟上」的场景）。 */
function setup(options: ConstructorParameters<typeof MockPlatformClient>[0] = {}) {
  const mock = new MockPlatformClient(options);
  const clock = { skewMs: 0 };
  const store = new VisitStore();
  store.init(mock, () => mock.now() + clock.skewMs);
  const advance = (ms: number) => {
    mock.advance(ms);
    store.tick();
  };
  return { mock, store, advance, clock };
}

describe('outbound（我的宠物出门）', () => {
  it('walks the full lifecycle and frees the slot after homecoming', async () => {
    const { store, advance } = setup();

    await store.requestVisit('user-keyu');
    expect(store.outboundPhase).toBe('pending'); // 邀请挂起，宠物还在家

    advance(1_000); // 对方接受
    expect(store.outboundPhase).toBe('traveling');

    advance(3_500); // 过场结束；租约从接受那刻起算，过场时间含在 30 分钟内
    expect(store.outboundPhase).toBe('visiting');
    expect(store.outboundRemainingMs).toBe(30 * MIN - 3_500);

    advance(10 * MIN);
    expect(store.outboundRemainingMs).toBe(20 * MIN - 3_500);

    advance(20 * MIN + 3_500); // 到期 + 归家过场
    expect(store.outboundPhase).toBe('ended');

    store.clearEndedOutbound();
    expect(store.outbound).toBeNull();
  });

  it('derives homecoming locally when the server never sends an end message（SV §6）', async () => {
    const { store, advance, clock } = setup();
    await store.requestVisit('user-keyu');
    advance(4_500); // → visiting
    expect(store.outboundPhase).toBe('visiting');

    // 本地时钟越过 endsAt，但 mock（=服务端）一个事件都没发
    clock.skewMs = 31 * MIN;
    store.tick();
    expect(store.outboundPhase).toBe('returning');
  });

  it('recall goes home immediately with no confirmation step', async () => {
    const { store, advance } = setup();
    await store.requestVisit('user-keyu');
    advance(4_500);
    await store.recallOutbound();
    expect(store.outboundPhase).toBe('returning');
    advance(3_500);
    expect(store.outbound?.status).toBe('recalled');
  });
});

describe('reset（登出清场）', () => {
  it('clears both slots and the projection but keeps the store usable', async () => {
    const { mock, store, advance } = setup({
      autoRespond: 'manual',
      projectionScript: [{ status: 'working', ms: 5_000 }],
    });
    await store.requestVisit('user-sarahk');
    mock.simulateIncomingVisit('user-keyu');
    await store.respondInbound('accept');
    advance(3_600);
    expect(store.outbound).not.toBeNull();
    expect(store.guestProjection).not.toBeNull();

    // 登出：租约属于会话，双槽/投影/幂等键全清——但 client 订阅与时钟保留
    store.reset();
    expect(store.outbound).toBeNull();
    expect(store.inbound).toBeNull();
    expect(store.guestProjection).toBeNull();
    expect(store.outboundPhase).toBe('none');

    // client 订阅未拆：服务端的新租约事件照常入店，重登直接续用
    mock.simulateIncomingVisit('user-momo');
    expect(store.inboundPhase).toBe('pending');
  });
});

describe('共同记忆结算（W9 P4-C）', () => {
  it('settles exactly once when the visit completes naturally', async () => {
    const { mock, store, advance } = setup();
    const settle = vi.spyOn(mock, 'settleMemory');

    await store.requestVisit('user-keyu');
    advance(1_000 + 30 * MIN + 3_500); // 接受 → 到期 → 归家过场 → completed
    expect(store.outbound?.status).toBe('completed');

    expect(settle).toHaveBeenCalledTimes(1);
    const memories = await mock.sharedMemories();
    expect(memories).toHaveLength(1);
    expect(memories[0].visitId).toBe(store.outbound?.id);
  });

  it('settles recalled visits too（never-punish：召回不惩罚记忆）', async () => {
    const { mock, store, advance } = setup();
    const settle = vi.spyOn(mock, 'settleMemory');

    await store.requestVisit('user-keyu');
    advance(4_500); // → visiting
    await store.recallOutbound();
    advance(3_500); // 归家过场 → recalled
    expect(store.outbound?.status).toBe('recalled');

    expect(settle).toHaveBeenCalledTimes(1);
    expect((await mock.sharedMemories())[0]?.visitId).toBe(store.outbound?.id);
  });

  it('never settles requested-side endings（declined/cancelled 不产生记忆）', async () => {
    const { mock, store, advance } = setup({ autoRespond: 'decline' });
    const settle = vi.spyOn(mock, 'settleMemory');

    await store.requestVisit('user-keyu');
    advance(1_000); // 对方婉拒 → declined
    expect(store.outbound?.status).toBe('declined');

    expect(settle).not.toHaveBeenCalled();
    expect(await mock.sharedMemories()).toEqual([]);
  });
});

describe('inbound（好友宠物来我家）', () => {
  it('accepts an incoming visit and receives the guest projection during the lease window', async () => {
    const { mock, store, advance } = setup({
      autoRespond: 'manual',
      projectionScript: [
        { status: 'working', ms: 5_000 },
        { status: 'idle', ms: 5_000 },
      ],
    });

    mock.simulateIncomingVisit('user-keyu');
    expect(store.inboundPhase).toBe('pending');
    expect(store.guestProjection).toBeNull(); // requested 阶段不订阅

    await store.respondInbound('accept');
    advance(3_600); // 过场结束进 visiting，投影剧本开播
    expect(store.inboundPhase).toBe('visiting');
    expect(store.guestProjection?.status).toBe('working');
    expect(store.guestProjection?.displayName).toBe('Bobo');

    advance(5_000);
    expect(store.guestProjection?.status).toBe('idle');

    await store.endInbound();
    advance(3_500);
    expect(store.inboundPhase).toBe('ended');
    store.clearEndedInbound();
    expect(store.inbound).toBeNull();
    expect(store.guestProjection).toBeNull(); // 退订即清空
  });

  it('declining an incoming visit never subscribes to the projection', async () => {
    const { mock, store, advance } = setup({ autoRespond: 'manual' });
    mock.simulateIncomingVisit('user-keyu');
    await store.respondInbound('decline');
    advance(60 * MIN);
    expect(store.inboundPhase).toBe('ended');
    expect(store.guestProjection).toBeNull();
  });

  it('keeps outbound and inbound slots independent（我出门做客的同时可以接待客人）', async () => {
    const { mock, store, advance } = setup({ autoRespond: 'manual' });
    await store.requestVisit('user-sarahk');
    mock.acceptPending();
    advance(4_000); // 我的宠物到 Sarah 家
    expect(store.outboundPhase).toBe('visiting');

    mock.simulateIncomingVisit('user-keyu'); // Keyu 的宠物申请来我家
    expect(store.inboundPhase).toBe('pending');
    expect(store.outboundPhase).toBe('visiting'); // 互不干扰
  });
});
