// @vitest-environment node
// W9 联调：SV §12「集成恢复」异常矩阵，对 `supabase start` 本地真实栈执行。
// 平时 `pnpm test` 自动跳过（环境变量缺失）；用 scripts/w9-recovery-matrix.sh
// 一键运行——比 W6 多注入 db 容器名，用于时间快进（拨时钟列 + 手动跑
// private.maintain_visits()，与生产 pg_cron 同一条代码路径）。
//
// 矩阵与 §12 验收线的对应：
//   requested/visiting 阶段断网（关闭应用）→ 重启后从服务端真相重建，一个轮询周期收敛
//   应用关闭后跨租约结束时间再启动 → 直接看到终局，不重新掉进访问态
//   双端同时召回/送客、召回vs到期、拉黑vs召回 → 单一终局、双端一致、干净报错不悬挂
//   不重复记忆 → 双端各自结算 settle_shared_memory，服务端只落一行
//   不出分身 → 重启后活跃访问唯一；投影退订/重订阅帧不重复、死订阅不收帧
//
// “断网”的诚实分解：本客户端的两条网络腿是 PostgREST 轮询与 Realtime 广播。
// 轮询腿的断连+恢复 ≡ 重启语义（缓存清零后靠轮询重建，下面全面覆盖）；
// 广播腿的传输层自动重连是 supabase-js 自己的重连机（不属于本单元），
// 租约正确性从不依赖广播——轮询兜底正是本矩阵反复证明的通道。

import { afterEach, describe, expect, it } from 'vitest';
import {
  type Actor,
  ageVisitClocks,
  DB_CONTAINER,
  disposeActor,
  maintainVisits,
  PUBLISHABLE_KEY,
  rawRpc,
  restartActor,
  SUPABASE_URL,
  signUpActor,
  sql,
  until,
  untilJoined,
  visitRowStatus,
} from './integration-harness';
import type { PublicPetProjection, VisitLease } from './types';

const ACTIVE_STATUSES: readonly string[] = ['accepted', 'traveling', 'visiting'];

const live: Actor[] = [];

/** 每个场景一对全新好友：账号唯一活动访问的部分唯一索引不跨场景串扰。 */
async function pair(label: string): Promise<{ visitor: Actor; host: Actor }> {
  const [visitor, host] = await Promise.all([signUpActor(`${label}-v`), signUpActor(`${label}-h`)]);
  live.push(visitor, host);
  await rawRpc(visitor, 'send_friend_request', { p_target_user_id: host.userId });
  await rawRpc(host, 'accept_friend_request', { p_requester_user_id: visitor.userId });
  return { visitor, host };
}

/** 请求→接受→快进过场：把一对好友确定性推进到 visiting。 */
async function establishVisiting(visitor: Actor, host: Actor): Promise<VisitLease> {
  const requested = await visitor.platform.requestVisit(host.userId, crypto.randomUUID());
  const pending = await until(
    () => host.leases.find((l) => l.id === requested.id),
    'host 轮询到访问请求',
  );
  await host.platform.respondVisit(pending.id, 'accept', crypto.randomUUID());
  // 租约整体前移 20s（30 分钟跨度不变）：maintain 的 accepted→traveling→visiting
  // 两跳在同一次调用内完成
  ageVisitClocks(requested.id, 20, ['started_at', 'ends_at']);
  maintainVisits();
  return until(
    () => visitor.leases.find((l) => l.id === requested.id && l.status === 'visiting'),
    'visitor 收敛到 visiting',
  );
}

/** 竞态可能把行留在 returning——按生产节奏快进 15s 过场，让 cron 逻辑收出终局。 */
function settleReturning(visitId: string): void {
  if (visitRowStatus(visitId) === 'returning') {
    ageVisitClocks(visitId, 16, ['returning_started_at']);
    maintainVisits();
  }
}

afterEach(async () => {
  while (live.length > 0) await disposeActor(live.pop());
});

describe.runIf(SUPABASE_URL !== '' && PUBLISHABLE_KEY !== '' && DB_CONTAINER !== '')(
  'SV §12 集成恢复矩阵 ↔ supabase start 真实栈',
  () => {
    it('requested 阶段断网：离线期间被接受，重启后一个轮询周期收敛到活跃租约', async () => {
      const { visitor, host } = await pair('req-offline');
      const requested = await visitor.platform.requestVisit(host.userId, crypto.randomUUID());

      // 访客“断网/关闭应用”：轮询与频道全部停摆，错过 accept 的一切在线信号
      visitor.platform.dispose();
      const pending = await until(
        () => host.leases.find((l) => l.id === requested.id),
        'host 轮询到访问请求',
      );
      await host.platform.respondVisit(pending.id, 'accept', crypto.randomUUID());

      // 重启：全新客户端、全新租约流水，只有服务端真相可依赖
      await restartActor(visitor);
      const recovered = await until(
        () =>
          visitor.leases.find((l) => l.id === requested.id && ACTIVE_STATUSES.includes(l.status)),
        'visitor 重启后收敛到活跃租约',
      );
      expect(recovered.startedAt).not.toBeNull();
      expect(recovered.endsAt).not.toBeNull();

      // 不出分身：重启后的流水里活跃访问只有这一单
      const activeIds = new Set(
        visitor.leases.filter((l) => ACTIVE_STATUSES.includes(l.status)).map((l) => l.id),
      );
      expect(activeIds).toEqual(new Set([requested.id]));
    }, 60_000);

    it('visiting 阶段 host 断网 + 跨租约结束重启：直接看到 completed，不重新掉进访问态', async () => {
      const { visitor, host } = await pair('lease-restart');
      const lease = await establishVisiting(visitor, host);

      // 主人端应用关闭；服务端不管双方在不在线，到点照样收尾租约
      host.platform.dispose();
      ageVisitClocks(lease.id, 30 * 60 + 1, ['started_at', 'ends_at']);
      maintainVisits();
      expect(visitRowStatus(lease.id)).toBe('completed');

      // 跨租约结束时间后重启：重建的认知里该访问只有终局，没有任何活跃状态
      await restartActor(host);
      await until(
        () => host.leases.find((l) => l.id === lease.id && l.status === 'completed'),
        'host 重启后看到 completed',
      );
      expect(
        host.leases.filter((l) => l.id === lease.id).every((l) => l.status === 'completed'),
      ).toBe(true);

      // 在线的一端靠轮询收敛到同一终局（不依赖广播）——不永久留在访问状态
      await until(
        () => visitor.leases.find((l) => l.id === lease.id && l.status === 'completed'),
        'visitor 轮询收敛到 completed',
      );
    }, 60_000);

    it('双端同时召回/送客：单一终局、双端一致，双端重复结算不重复记忆', async () => {
      const { visitor, host } = await pair('dual-end');
      const lease = await establishVisiting(visitor, host);

      const outcome = await Promise.allSettled([
        visitor.platform.recallVisit(lease.id, crypto.randomUUID()),
        host.platform.endVisit(lease.id, crypto.randomUUID()),
      ]);
      // 至少一端成功；落败的一端必须是干净的错误，不是悬挂
      expect(outcome.some((r) => r.status === 'fulfilled')).toBe(true);
      for (const r of outcome) {
        if (r.status === 'rejected') expect(r.reason).toBeInstanceOf(Error);
      }

      settleReturning(lease.id);
      const finalStatus = visitRowStatus(lease.id);
      expect(['completed', 'recalled']).toContain(finalStatus);

      // visits 行是单一真相：双端各自经轮询收敛到同一个终局
      await until(
        () => visitor.leases.find((l) => l.id === lease.id && l.status === finalStatus),
        `visitor 收敛到 ${finalStatus}`,
      );
      await until(
        () => host.leases.find((l) => l.id === lease.id && l.status === finalStatus),
        `host 收敛到 ${finalStatus}`,
      );

      // 不重复记忆：双端各用自己的幂等键结算，服务端只能落一行
      const settles = await Promise.allSettled([
        rawRpc(visitor, 'settle_shared_memory', {
          p_visit_id: lease.id,
          p_idempotency_key: crypto.randomUUID(),
        }),
        rawRpc(host, 'settle_shared_memory', {
          p_visit_id: lease.id,
          p_idempotency_key: crypto.randomUUID(),
        }),
      ]);
      expect(settles.some((r) => r.status === 'fulfilled')).toBe(true);
      expect(
        sql(`SELECT count(*) FROM public.shared_memories WHERE visit_id = '${lease.id}';`),
      ).toBe('1');
    }, 60_000);

    it('召回与服务端到期竞态：谁赢都行，双端收敛到同一个终局', async () => {
      const { visitor, host } = await pair('recall-vs-expiry');
      const lease = await establishVisiting(visitor, host);

      const [recall] = await Promise.allSettled([
        visitor.platform.recallVisit(lease.id, crypto.randomUUID()),
        Promise.resolve().then(() => {
          ageVisitClocks(lease.id, 30 * 60 + 1, ['started_at', 'ends_at']);
          maintainVisits();
        }),
      ]);
      // 到期先到时召回落败——必须是干净的错误而不是悬挂
      if (recall.status === 'rejected') expect(recall.reason).toBeInstanceOf(Error);

      settleReturning(lease.id);
      const finalStatus = visitRowStatus(lease.id);
      expect(['completed', 'recalled']).toContain(finalStatus);
      await until(
        () => visitor.leases.find((l) => l.id === lease.id && l.status === finalStatus),
        `visitor 收敛到 ${finalStatus}`,
      );
      await until(
        () => host.leases.find((l) => l.id === lease.id && l.status === finalStatus),
        `host 收敛到 ${finalStatus}`,
      );
    }, 60_000);

    it('拉黑与召回竞态：进行中的访问被服务端终结，双端收敛且此后无法再发起', async () => {
      const { visitor, host } = await pair('block-race');
      const lease = await establishVisiting(visitor, host);

      const [block] = await Promise.allSettled([
        rawRpc(host, 'block_user', { p_target_user_id: visitor.userId }),
        visitor.platform.recallVisit(lease.id, crypto.randomUUID()),
      ]);
      // 拉黑不依赖访问状态，必须成功；blocks 上的触发器负责终结进行中的访问
      expect(block.status).toBe('fulfilled');

      settleReturning(lease.id);
      const finalStatus = visitRowStatus(lease.id);
      expect(['blocked', 'recalled']).toContain(finalStatus);
      await until(
        () => visitor.leases.find((l) => l.id === lease.id && l.status === finalStatus),
        `visitor 收敛到 ${finalStatus}`,
      );
      await until(
        () => host.leases.find((l) => l.id === lease.id && l.status === finalStatus),
        `host 收敛到 ${finalStatus}`,
      );

      // 拉黑删除了好友关系：新的访问请求被干净拒绝（never-punish 的呈现留给 UI）
      await expect(
        visitor.platform.requestVisit(host.userId, crypto.randomUUID()),
      ).rejects.toThrow();
    }, 60_000);

    it('「稍后」的 24h 兜底：requested 到期后双端收敛 expired，不卡待处理', async () => {
      const { visitor, host } = await pair('request-expiry');
      const requested = await visitor.platform.requestVisit(host.userId, crypto.randomUUID());
      await until(
        () => host.leases.find((l) => l.id === requested.id && l.status === 'requested'),
        'host 看到 requested',
      );

      ageVisitClocks(requested.id, 24 * 60 * 60 + 1, ['requested_at', 'request_expires_at']);
      maintainVisits();

      await until(
        () => host.leases.find((l) => l.id === requested.id && l.status === 'expired'),
        'host 收敛到 expired',
      );
      await until(
        () => visitor.leases.find((l) => l.id === requested.id && l.status === 'expired'),
        'visitor 收敛到 expired',
      );
    }, 60_000);

    it('投影退订/重订阅：帧不重复、死订阅不再收帧——分身防线', async () => {
      const { visitor, host } = await pair('projection-resub');
      const lease = await establishVisiting(visitor, host);

      const firstFrames: PublicPetProjection[] = [];
      const unsubFirst = host.platform.subscribeGuestProjection(lease, (p) => firstFrames.push(p));
      await untilJoined(host, lease);
      await rawRpc(visitor, 'update_projection', {
        p_pet_id: 'yoonie',
        p_skin_id: 'yoonie',
        p_status: 'working',
      });
      await until(() => firstFrames.find((f) => f.status === 'working'), '第一订阅收到 working 帧');
      unsubFirst();

      const secondFrames: PublicPetProjection[] = [];
      const unsubSecond = host.platform.subscribeGuestProjection(lease, (p) =>
        secondFrames.push(p),
      );
      try {
        await untilJoined(host, lease);
        // 让订阅时的一次性回放（working）先落地，之后收到的 idle 只可能来自广播
        await new Promise((resolve) => setTimeout(resolve, 1_000));
        await rawRpc(visitor, 'update_projection', {
          p_pet_id: 'yoonie',
          p_skin_id: 'yoonie',
          p_status: 'idle',
        });
        await until(() => secondFrames.find((f) => f.status === 'idle'), '重订阅收到 idle 帧');
        // 给重复帧一个出现窗口再断言恰好一帧：旧频道未被移除时这里会翻车
        await new Promise((resolve) => setTimeout(resolve, 1_500));
        expect(secondFrames.filter((f) => f.status === 'idle')).toHaveLength(1);
        // 退订过的第一订阅是死的，不该再收到任何新帧
        expect(firstFrames.some((f) => f.status === 'idle')).toBe(false);
      } finally {
        unsubSecond();
      }
    }, 60_000);
  },
);
