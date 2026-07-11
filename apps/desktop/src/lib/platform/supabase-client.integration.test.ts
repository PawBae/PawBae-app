// @vitest-environment node
// W6 联调：SupabasePlatformClient 对 `supabase start` 本地真实栈的全链路验证。
// 平时 `pnpm test` 自动跳过（环境变量缺失）；用 scripts/w6-supabase-integration.sh
// 一键运行——它负责发现本地栈地址、播种一次性邀请码并注入下面三个变量。
//
// 覆盖面（对应 P4 spec §2-§4，与单测的假件互补——这里全是真 GoTrue/PostgREST/Realtime）：
//   会话恢复 → 好友三表拼装（pending_out/in → accepted）→ 邀请码兑换与幂等重放
//   → requestVisit 回包即喂 / 对端轮询感知 → accept 30 分钟租约 → 投影订阅即回放
//   + 广播帧剥 transport id → recall → visit_ended/轮询把 returning→recalled 带给双方。

import { createClient, type SupabaseClient } from '@supabase/supabase-js';
import { afterAll, describe, expect, it } from 'vitest';
import { type SupabaseLike, SupabasePlatformClient } from './supabase-client';
import type { PublicPetProjection, VisitLease } from './types';

// 桌面 tsconfig 面向 DOM，没有（也不该为一份联调文件引入）@types/node——
// 本文件只在 vitest 的 node 环境跑，就地声明 process.env 的最小形状；
// 随机数统一走 Web Crypto 全局（DOM lib 有类型，Node ≥19 运行时也有）。
declare const process: { env: Record<string, string | undefined> };

const SUPABASE_URL = process.env.PAWBAE_SUPABASE_URL ?? '';
const PUBLISHABLE_KEY = process.env.PAWBAE_SUPABASE_PUBLISHABLE_KEY ?? '';
const INVITE_CODE = process.env.PAWBAE_INVITE_CODE ?? '';

/** 联调把 15s 生产轮询压到 2s：跨端感知断言不用等一刻钟。 */
const POLL_MS = 2_000;

async function until<T>(
  probe: () => T | undefined | null | false,
  what: string,
  timeoutMs = 15_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = probe();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
}

interface Actor {
  raw: SupabaseClient;
  platform: SupabasePlatformClient;
  userId: string;
  /** onLeaseChange 全量流水：三源合流的观测窗口。 */
  leases: VisitLease[];
}

async function signUpActor(label: string): Promise<Actor> {
  const raw = createClient(SUPABASE_URL, PUBLISHABLE_KEY, {
    auth: { persistSession: false, autoRefreshToken: false },
  });
  const suffix = crypto.randomUUID().replaceAll('-', '').slice(0, 10);
  const { data, error } = await raw.auth.signUp({
    email: `w6-${label}-${suffix}@example.test`,
    password: `T3st-${crypto.randomUUID()}`,
  });
  if (error || !data.user) throw new Error(`signUp(${label}) failed: ${error?.message}`);
  const platform = new SupabasePlatformClient(() => raw as unknown as SupabaseLike, POLL_MS);
  await platform.start();
  const actor: Actor = { raw, platform, userId: data.user.id, leases: [] };
  platform.onLeaseChange((lease) => actor.leases.push(lease));
  return actor;
}

async function rawRpc(actor: Actor, name: string, params: Record<string, unknown>): Promise<void> {
  const { error } = await actor.raw.rpc(name, params);
  if (error) throw new Error(`${name} failed: ${error.message}`);
}

/**
 * 等 Realtime 频道真正 join 完成再触发广播源：Broadcast 不回放，join 前发出的帧
 * 会永久丢失（生产里由轮询兜底，测试里则要确定性）。走 supabase-js 公开 API
 * 观察频道状态，不需要生产接口暴露 join 时机。
 */
async function untilJoined(actor: Actor, lease: VisitLease): Promise<void> {
  const suffix = `pet:${lease.visitorUserId}:${lease.id}`;
  await until(
    () => actor.raw.getChannels().some((c) => c.topic.endsWith(suffix) && c.state === 'joined'),
    `Realtime 频道 ${suffix} join 完成`,
  );
}

describe.runIf(SUPABASE_URL !== '' && PUBLISHABLE_KEY !== '')(
  'SupabasePlatformClient ↔ supabase start 真实栈',
  () => {
    let visitor: Actor;
    let host: Actor;
    let acceptedLease: VisitLease;

    afterAll(async () => {
      visitor?.platform.dispose();
      host?.platform.dispose();
      await visitor?.raw.removeAllChannels();
      await host?.raw.removeAllChannels();
    });

    it('start() 恢复注册即得的会话', async () => {
      [visitor, host] = await Promise.all([signUpActor('visitor'), signUpActor('host')]);
      expect(visitor.platform.session()?.userId).toBe(visitor.userId);
      expect(host.platform.session()?.userId).toBe(host.userId);
    }, 20_000);

    it('friends()：pending_out/pending_in → accepted，且 handle 是匿名代号', async () => {
      await rawRpc(visitor, 'send_friend_request', { p_target_user_id: host.userId });

      const outbound = await visitor.platform.friends();
      expect(outbound).toHaveLength(1);
      expect(outbound[0].userId).toBe(host.userId);
      expect(outbound[0].relation).toBe('pending_out');
      expect(outbound[0].muted).toBe(false);
      // D 决议：profiles.handle 是服务端生成的匿名代号，绝不是注册邮箱/用户名
      expect(outbound[0].handle).toMatch(/^user-[0-9a-f]{32}$/);

      const inbound = await host.platform.friends();
      expect(inbound[0].relation).toBe('pending_in');

      await rawRpc(host, 'accept_friend_request', { p_requester_user_id: visitor.userId });
      const settled = await visitor.platform.friends();
      expect(settled[0].relation).toBe('accepted');
    }, 20_000);

    it.runIf(INVITE_CODE !== '')(
      'redeemInvite：兑换成功 + 同键重放幂等 + 坏码报错',
      async () => {
        const key = crypto.randomUUID();
        await expect(visitor.platform.redeemInvite(INVITE_CODE, key)).resolves.toBeUndefined();
        // 同一幂等键重放：命中 48h 结果缓存，同样成功返回
        await expect(visitor.platform.redeemInvite(INVITE_CODE, key)).resolves.toBeUndefined();
        await expect(
          host.platform.redeemInvite('bogus-code-123', crypto.randomUUID()),
        ).rejects.toThrow(/invite/i);
      },
      20_000,
    );

    it('requestVisit：回包立即喂本端，对端在一个轮询周期内看到', async () => {
      const lease = await visitor.platform.requestVisit(host.userId, crypto.randomUUID());
      expect(lease.status).toBe('requested');
      expect(lease.visitorUserId).toBe(visitor.userId);
      expect(lease.hostUserId).toBe(host.userId);
      // RPC 回包路径：await 返回时监听器已被喂过，不等轮询
      expect(visitor.leases.some((l) => l.id === lease.id && l.status === 'requested')).toBe(true);

      // 对端没有调 RPC，纯靠 visits 参与者轮询感知（真实 RLS + PostgREST）
      const seen = await until(
        () => host.leases.find((l) => l.id === lease.id),
        'host 轮询到 requested 租约',
      );
      expect(seen.status).toBe('requested');
    }, 20_000);

    it('respondVisit accept：30 分钟租约窗口', async () => {
      const pending = host.leases[host.leases.length - 1];
      acceptedLease = await host.platform.respondVisit(pending.id, 'accept', crypto.randomUUID());
      expect(acceptedLease.status).toBe('accepted');
      expect(acceptedLease.startedAt).not.toBeNull();
      expect(acceptedLease.endsAt).not.toBeNull();
      expect(
        Date.parse(acceptedLease.endsAt ?? '') - Date.parse(acceptedLease.startedAt ?? ''),
      ).toBe(30 * 60 * 1000);
    }, 20_000);

    it('投影：订阅即回放已有投影，广播帧剥掉 transport id 后恰好六键', async () => {
      // 访客宠物先发布投影（#64 发布管道走的同一个 RPC），host 订阅时应立即回放
      await rawRpc(visitor, 'update_projection', {
        p_pet_id: 'yoonie',
        p_skin_id: 'yoonie',
        p_status: 'working',
      });

      const frames: PublicPetProjection[] = [];
      const unsubscribe = host.platform.subscribeGuestProjection(acceptedLease, (p) =>
        frames.push(p),
      );
      try {
        const replay = await until(() => frames[0], '订阅时的投影回放');
        expect(replay.v).toBe(1);
        expect(replay.petId).toBe('yoonie');
        expect(replay.status).toBe('working');

        // 回放只证明单次 PostgREST 读完成，不证明频道已 join——join 前广播会丢
        await untilJoined(host, acceptedLease);
        await rawRpc(visitor, 'update_projection', {
          p_pet_id: 'yoonie',
          p_skin_id: 'yoonie',
          p_status: 'idle',
        });
        const live = await until(
          () => frames.find((f) => f.status === 'idle'),
          '广播的 projection_updated 帧',
        );
        // Realtime 注入的 transport id 必须被剥掉：共享清洗器恰好六键
        expect(Object.keys(live).toSorted()).toEqual([
          'displayName',
          'petId',
          'skinId',
          'status',
          'updatedAt',
          'v',
        ]);
      } finally {
        unsubscribe();
      }
    }, 30_000);

    it('recallVisit：本端回包 returning，对端经广播/轮询收敛到 recalled', async () => {
      // recall 期间保持订阅：visit_ended 广播是对端最快的终局信号。
      // 等 join 完成再 recall，确保广播路径被确定性压到（轮询仍是正确性兜底）。
      const unsubscribe = host.platform.subscribeGuestProjection(acceptedLease, () => {});
      try {
        await untilJoined(host, acceptedLease);
        const returning = await visitor.platform.recallVisit(acceptedLease.id, crypto.randomUUID());
        expect(returning.status).toBe('returning');

        await until(
          () =>
            host.leases.find(
              (l) => l.id === acceptedLease.id && ['returning', 'recalled'].includes(l.status),
            ),
          'host 感知归途状态',
        );
      } finally {
        unsubscribe();
      }

      // returning → recalled 由服务端 cron 收尾（与 A 线 api-e2e 同一预算：≤100s）
      await until(
        () => host.leases.find((l) => l.id === acceptedLease.id && l.status === 'recalled'),
        'host 收敛到 recalled',
        100_000,
      );
      await until(
        () => visitor.leases.find((l) => l.id === acceptedLease.id && l.status === 'recalled'),
        'visitor 轮询收敛到 recalled',
        100_000,
      );
    }, 240_000);
  },
);
