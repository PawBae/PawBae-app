// 串门状态 store：PlatformClient 与 UI 之间的胶水层（line-c W5-6）。
// 双槽位：outbound = 我的宠物出门做客（空窝/召回），inbound = 好友宠物来我家
// （GuestPet/待客）。v1 规则「一只宠物只能发起一个访问、一个家同时只接待一位」
// 即每槽最多一张租约。
//
// 三条硬语义都下沉在 lease-machine：到期一律按 endsAt 本地推导（不依赖必达的
// 结束消息）、reconcileLease 乱序收敛（无分身）、blocked 最高优先级。
// 真实后端是唯一事实源，因此本 store 不做本地持久化——App 重启后由
// PlatformClient 的会话恢复流程重新喂租约。

import {
  deriveLocalPhase,
  isEnded,
  newIdempotencyKey,
  reconcileLease,
  remainingMs,
} from '../platform/lease-machine';
import type {
  PlatformClient,
  PublicPetProjection,
  Unsubscribe,
  VisitLease,
} from '../platform/types';

export class VisitStore {
  outbound = $state<VisitLease | null>(null);
  inbound = $state<VisitLease | null>(null);
  /** 来访宠物的最新公开投影（仅 inbound 租约有效期内有值）。 */
  guestProjection = $state<PublicPetProjection | null>(null);
  nowMs = $state(0);

  outboundPhase = $derived(deriveLocalPhase(this.outbound, this.nowMs));
  inboundPhase = $derived(deriveLocalPhase(this.inbound, this.nowMs));
  /** 空窝牌倒计时的原料；展示粒度用 lease-machine 的 formatRemaining。 */
  outboundRemainingMs = $derived(
    this.outbound === null ? null : remainingMs(this.outbound, this.nowMs),
  );

  private client: PlatformClient | null = null;
  private nowFn: () => number = () => Date.now();
  private unsubLease: Unsubscribe | null = null;
  private unsubProjection: Unsubscribe | null = null;
  private clockTimer: ReturnType<typeof setInterval> | null = null;
  /** 幂等键跟随「用户意图」：同一目标的失败重试复用同一个键，成功后作废。 */
  private pendingRequest: { host: string; key: string } | null = null;

  /** @param nowFn 时钟注入——mock 驱动时传 () => mock.now()，真实实现用默认 Date.now。 */
  init(client: PlatformClient, nowFn?: () => number): void {
    this.dispose();
    this.client = client;
    if (nowFn) this.nowFn = nowFn;
    this.nowMs = this.nowFn();
    this.unsubLease = client.onLeaseChange((incoming) => {
      const me = client.session()?.userId;
      if (!me) return;
      if (incoming.visitorUserId === me) {
        this.outbound = reconcileLease(this.outbound, incoming);
      }
      if (incoming.hostUserId === me) {
        this.inbound = reconcileLease(this.inbound, incoming);
        this.syncGuestSubscription();
      }
    });
  }

  dispose(): void {
    this.stopClock();
    this.unsubLease?.();
    this.unsubLease = null;
    this.dropGuestSubscription();
    this.client = null;
  }

  // ---------- 时钟 ----------

  /** UI 挂载时启动秒级时钟；空窝倒计时与到期本地推导都靠它驱动。 */
  startClock(intervalMs = 1_000): void {
    this.stopClock();
    this.clockTimer = setInterval(() => {
      this.nowMs = this.nowFn();
    }, intervalMs);
  }

  stopClock(): void {
    if (this.clockTimer !== null) clearInterval(this.clockTimer);
    this.clockTimer = null;
  }

  /** 手动推一格时钟（测试与 mock 剧本用）。 */
  tick(): void {
    this.nowMs = this.nowFn();
  }

  // ---------- 动作（访客侧） ----------

  async requestVisit(hostUserId: string): Promise<VisitLease> {
    const client = this.mustClient();
    if (this.pendingRequest?.host !== hostUserId) {
      this.pendingRequest = { host: hostUserId, key: newIdempotencyKey() };
    }
    const lease = await client.requestVisit(hostUserId, this.pendingRequest.key);
    this.pendingRequest = null; // 失败会 throw 并保留键，重试复用
    this.outbound = reconcileLease(this.outbound, lease);
    return lease;
  }

  async cancelOutbound(): Promise<void> {
    await this.actOn(this.outbound, (client, id) => client.cancelVisit(id, newIdempotencyKey()));
  }

  async recallOutbound(): Promise<void> {
    // 召回零确认、零损失（never-punish）：调用即归家
    await this.actOn(this.outbound, (client, id) => client.recallVisit(id, newIdempotencyKey()));
  }

  /** 归家动画播完后由 UI 调用，腾出槽位允许下一次出门。 */
  clearEndedOutbound(): void {
    if (this.outbound && isEnded(this.outbound.status)) this.outbound = null;
  }

  // ---------- 动作（接待侧） ----------

  async respondInbound(action: 'accept' | 'decline'): Promise<void> {
    await this.actOn(this.inbound, (client, id) =>
      client.respondVisit(id, action, newIdempotencyKey()),
    );
  }

  async endInbound(): Promise<void> {
    await this.actOn(this.inbound, (client, id) => client.endVisit(id, newIdempotencyKey()));
  }

  clearEndedInbound(): void {
    if (this.inbound && isEnded(this.inbound.status)) {
      this.inbound = null;
      this.dropGuestSubscription();
    }
  }

  // ---------- 内部 ----------

  private mustClient(): PlatformClient {
    if (!this.client) throw new Error('VISIT_STORE_NOT_INITIALIZED');
    return this.client;
  }

  private async actOn(
    lease: VisitLease | null,
    fn: (client: PlatformClient, leaseId: string) => Promise<VisitLease>,
  ): Promise<void> {
    if (!lease) return;
    const updated = await fn(this.mustClient(), lease.id);
    const me = this.client?.session()?.userId;
    if (updated.visitorUserId === me) this.outbound = reconcileLease(this.outbound, updated);
    if (updated.hostUserId === me) {
      this.inbound = reconcileLease(this.inbound, updated);
      this.syncGuestSubscription();
    }
  }

  /** inbound 租约生效即订阅访客投影，结束即退订——订阅窗口与租约窗口一致。 */
  private syncGuestSubscription(): void {
    const lease = this.inbound;
    if (!lease || isEnded(lease.status)) {
      this.dropGuestSubscription();
      return;
    }
    if (lease.status === 'requested' || this.unsubProjection !== null) return;
    const client = this.client;
    if (!client) return;
    this.unsubProjection = client.subscribeGuestProjection(lease, (p) => {
      this.guestProjection = p;
    });
  }

  private dropGuestSubscription(): void {
    this.unsubProjection?.();
    this.unsubProjection = null;
    this.guestProjection = null;
  }
}

export const visitStore = new VisitStore();
