// MockPlatformClient —— C 线 W5 的解耦支点（line-c-stage.md §3）。
// 实现 PlatformClient 接口的脚本化 mock：虚拟时钟可加速、剧本可注入
// （婉拒/过期/召回/断网），让串门 UI 在 W7 换真实实现前完全脱离服务端开发。
// 换线方式：一行 DI —— new MockPlatformClient() ↔ new SupabasePlatformClient()。
//
// 时间模型：不用真实 setTimeout。所有延时事件进虚拟队列，advance(ms) 按序
// 触发——测试确定性执行，开发面板可用 setInterval(() => mock.advance(dt)) 驱动。

import { isActive, isEnded, newIdempotencyKey } from './lease-machine';
import type {
  FriendEntry,
  PlatformClient,
  PlatformSession,
  ProjectionStatus,
  PublicPetProjection,
  Unsubscribe,
  VisitLease,
  VisitStatus,
} from './types';

export interface MockGuestPet {
  petId: string;
  displayName: string;
  skinId: string;
}

export interface MockPlatformOptions {
  session?: PlatformSession | null;
  friends?: FriendEntry[];
  /** 收到邀请后对方的反应；manual 时用 acceptPending()/declinePending() 手动推进 */
  autoRespond?: 'accept' | 'decline' | 'manual';
  respondDelayMs?: number;
  /** 离家/归家过场时长（拍板 A 档 3.5s） */
  travelMs?: number;
  visitDurationMs?: number;
  inviteExpiryMs?: number;
  /** 访客宠物的投影剧本，循环播放 */
  projectionScript?: Array<{ status: ProjectionStatus; ms: number }>;
  guestPet?: MockGuestPet;
  /** 虚拟时钟起点（epoch ms），默认 0——测试断言时间戳时更直观 */
  startAtMs?: number;
}

interface ScheduledEvent {
  at: number;
  seq: number;
  fn: () => void;
}

const DEFAULT_SESSION: PlatformSession = {
  userId: 'user-mock-me',
  handle: 'yining',
  displayName: 'Yining',
  avatarUrl: null,
};

const DEFAULT_FRIENDS: FriendEntry[] = [
  { userId: 'user-keyu', handle: 'keyu', displayName: 'Keyu', relation: 'accepted', muted: false },
  {
    userId: 'user-sarahk',
    handle: 'sarahk',
    displayName: 'Sarah',
    relation: 'accepted',
    muted: false,
  },
  {
    userId: 'user-devon',
    handle: 'devon',
    displayName: null,
    relation: 'pending_in',
    muted: false,
  },
];

const DEFAULT_PROJECTION_SCRIPT: Array<{ status: ProjectionStatus; ms: number }> = [
  { status: 'working', ms: 8_000 },
  { status: 'waiting', ms: 5_000 },
  { status: 'working', ms: 6_000 },
  { status: 'idle', ms: 6_000 },
  { status: 'compacting', ms: 4_000 },
  { status: 'idle', ms: 8_000 },
];

export class MockPlatformClient implements PlatformClient {
  private currentSession: PlatformSession | null;
  private readonly friendList: FriendEntry[];
  private readonly autoRespond: 'accept' | 'decline' | 'manual';
  private readonly respondDelayMs: number;
  private readonly travelMs: number;
  private readonly visitDurationMs: number;
  private readonly inviteExpiryMs: number;
  private readonly script: Array<{ status: ProjectionStatus; ms: number }>;
  private readonly guestPet: MockGuestPet;

  private virtualNow: number;
  private seq = 0;
  private queue: ScheduledEvent[] = [];

  // 内部持有可变副本（契约 VisitLease 是 readonly；对外发射时按 VisitLease 读）
  private leases = new Map<string, { -readonly [K in keyof VisitLease]: VisitLease[K] }>();
  private idempotency = new Map<string, string>();
  private redeemedInvites = new Set<string>();
  private leaseListeners = new Set<(lease: VisitLease) => void>();
  private sessionListeners = new Set<(s: PlatformSession | null) => void>();
  private projectionSubs = new Map<string, Set<(p: PublicPetProjection) => void>>();
  private projectionStatus = new Map<string, ProjectionStatus>();
  private projectionPaused = new Set<string>();
  private leaseCounter = 0;

  constructor(options: MockPlatformOptions = {}) {
    this.currentSession = options.session === undefined ? DEFAULT_SESSION : options.session;
    this.friendList = options.friends ?? DEFAULT_FRIENDS;
    this.autoRespond = options.autoRespond ?? 'accept';
    this.respondDelayMs = options.respondDelayMs ?? 1_000;
    this.travelMs = options.travelMs ?? 3_500;
    this.visitDurationMs = options.visitDurationMs ?? 30 * 60_000;
    this.inviteExpiryMs = options.inviteExpiryMs ?? 24 * 60 * 60_000;
    this.script = options.projectionScript ?? DEFAULT_PROJECTION_SCRIPT;
    this.guestPet = options.guestPet ?? {
      petId: 'pet-bobo',
      displayName: 'Bobo',
      skinId: 'default',
    };
    this.virtualNow = options.startAtMs ?? 0;
  }

  // ---------- 虚拟时钟 ----------

  now(): number {
    return this.virtualNow;
  }

  /** 推进虚拟时钟，按时间顺序触发到期事件（事件里新排的事件同样会被处理）。 */
  advance(ms: number): void {
    const target = this.virtualNow + ms;
    for (;;) {
      const next = this.queue
        .filter((e) => e.at <= target)
        .sort((a, b) => a.at - b.at || a.seq - b.seq)[0];
      if (!next) break;
      this.queue = this.queue.filter((e) => e !== next);
      this.virtualNow = Math.max(this.virtualNow, next.at);
      next.fn();
    }
    this.virtualNow = target;
  }

  private at(delayMs: number, fn: () => void): void {
    this.queue.push({ at: this.virtualNow + delayMs, seq: this.seq++, fn });
  }

  private iso(ms: number): string {
    return new Date(ms).toISOString();
  }

  // ---------- 会话 ----------

  session(): PlatformSession | null {
    return this.currentSession;
  }

  onSessionChange(cb: (s: PlatformSession | null) => void): Unsubscribe {
    this.sessionListeners.add(cb);
    return () => this.sessionListeners.delete(cb);
  }

  /** 剧本注入：模拟登录/登出。 */
  setSession(session: PlatformSession | null): void {
    this.currentSession = session;
    for (const cb of this.sessionListeners) cb(session);
  }

  // ---------- 串门 ----------

  async requestVisit(hostUserId: string, idempotencyKey: string): Promise<VisitLease> {
    const session = this.currentSession;
    if (!session) throw new Error('NOT_AUTHENTICATED');
    const replay = this.replayByKey(idempotencyKey);
    if (replay) return replay;
    for (const lease of this.leases.values()) {
      if (lease.visitorUserId === session.userId && isActive(lease.status)) {
        throw new Error('VISIT_ALREADY_ACTIVE'); // 服务端由部分唯一索引兜底
      }
    }
    const lease: VisitLease = {
      id: `lease-${++this.leaseCounter}`,
      visitorUserId: session.userId,
      hostUserId,
      status: 'requested',
      startedAt: null,
      endsAt: null,
    };
    this.leases.set(lease.id, lease);
    this.idempotency.set(idempotencyKey, lease.id);
    this.emitLease(lease);

    this.at(this.inviteExpiryMs, () => this.transition(lease.id, ['requested'], 'expired'));
    if (this.autoRespond !== 'manual') {
      const action = this.autoRespond;
      this.at(this.respondDelayMs, () => {
        if (this.leases.get(lease.id)?.status === 'requested') {
          void this.respondVisit(lease.id, action, newIdempotencyKey());
        }
      });
    }
    return this.snapshot(lease.id);
  }

  async respondVisit(
    leaseId: string,
    action: 'accept' | 'decline',
    key: string,
  ): Promise<VisitLease> {
    const replay = this.replayByKey(key);
    if (replay) return replay;
    const lease = this.mustGet(leaseId);
    if (lease.status !== 'requested') throw new Error('INVALID_TRANSITION');
    this.idempotency.set(key, leaseId);

    if (action === 'decline') {
      this.apply(leaseId, { status: 'declined' });
      return this.snapshot(leaseId);
    }

    // SV §3.2：接受时创建 30 分钟租约，记录 startedAt/endsAt，然后才离家
    const startedAt = this.virtualNow;
    const endsAt = startedAt + this.visitDurationMs;
    this.apply(leaseId, {
      status: 'accepted',
      startedAt: this.iso(startedAt),
      endsAt: this.iso(endsAt),
    });
    this.at(0, () => this.transition(leaseId, ['accepted'], 'traveling'));
    this.at(this.travelMs, () => {
      if (this.transition(leaseId, ['traveling'], 'visiting')) {
        this.startProjectionScript(leaseId);
      }
    });
    this.at(this.visitDurationMs, () => {
      if (this.transition(leaseId, ['visiting', 'traveling', 'accepted'], 'returning')) {
        this.at(this.travelMs, () => this.transition(leaseId, ['returning'], 'completed'));
      }
    });
    return this.snapshot(leaseId);
  }

  async cancelVisit(leaseId: string, key: string): Promise<VisitLease> {
    return this.finishEarly(leaseId, key, ['requested'], 'cancelled', false);
  }

  async recallVisit(leaseId: string, key: string): Promise<VisitLease> {
    return this.finishEarly(leaseId, key, ['accepted', 'traveling', 'visiting'], 'recalled', true);
  }

  async endVisit(leaseId: string, key: string): Promise<VisitLease> {
    return this.finishEarly(leaseId, key, ['accepted', 'traveling', 'visiting'], 'completed', true);
  }

  onLeaseChange(cb: (lease: VisitLease) => void): Unsubscribe {
    this.leaseListeners.add(cb);
    return () => this.leaseListeners.delete(cb);
  }

  subscribeGuestProjection(lease: VisitLease, cb: (p: PublicPetProjection) => void): Unsubscribe {
    let subs = this.projectionSubs.get(lease.id);
    if (!subs) {
      subs = new Set();
      this.projectionSubs.set(lease.id, subs);
    }
    subs.add(cb);
    // 订阅立即回放当前投影（仅在租约仍活跃时——撤销语义的守门在发送端）
    const current = this.leases.get(lease.id);
    if (current && isActive(current.status)) {
      cb(this.projectionOf(lease.id));
    }
    return () => {
      subs.delete(cb);
    };
  }

  // ---------- 邀请码 / 好友 ----------

  async redeemInvite(code: string, key: string): Promise<void> {
    if (this.idempotency.has(key)) return;
    if (code === 'expired-code') throw new Error('INVITE_EXPIRED');
    if (code.trim() === '') throw new Error('INVITE_INVALID');
    this.idempotency.set(key, `invite-${code}`);
    this.redeemedInvites.add(code);
  }

  async friends(): Promise<FriendEntry[]> {
    return this.friendList.map((f) => ({ ...f }));
  }

  // ---------- 剧本注入（测试与开发面板用，真实实现没有这些方法） ----------

  /** autoRespond: 'manual' 时手动让对方接受最新的待处理邀请。 */
  acceptPending(): void {
    const pending = this.latestPending();
    if (pending) void this.respondVisit(pending.id, 'accept', newIdempotencyKey());
  }

  declinePending(): void {
    const pending = this.latestPending();
    if (pending) void this.respondVisit(pending.id, 'decline', newIdempotencyKey());
  }

  /** 剧本注入：模拟好友发来的串门邀请（我是接待方）。接受/婉拒走正常 respondVisit。 */
  simulateIncomingVisit(fromUserId: string): VisitLease {
    const me = this.currentSession;
    if (!me) throw new Error('NOT_AUTHENTICATED');
    const lease: VisitLease = {
      id: `lease-${++this.leaseCounter}`,
      visitorUserId: fromUserId,
      hostUserId: me.userId,
      status: 'requested',
      startedAt: null,
      endsAt: null,
    };
    this.leases.set(lease.id, lease);
    this.emitLease(lease);
    this.at(this.inviteExpiryMs, () => this.transition(lease.id, ['requested'], 'expired'));
    return { ...lease };
  }

  /** 模拟主人 agent 断线/恢复等：手动覆盖访客投影状态并暂停剧本。 */
  setGuestProjectionStatus(leaseId: string, status: ProjectionStatus): void {
    this.projectionPaused.add(leaseId);
    this.projectionStatus.set(leaseId, status);
    this.emitProjection(leaseId);
  }

  resumeGuestProjectionScript(leaseId: string): void {
    if (!this.projectionPaused.delete(leaseId)) return;
    this.scheduleProjectionStep(leaseId, 0);
  }

  // ---------- 内部 ----------

  private latestPending(): VisitLease | undefined {
    return [...this.leases.values()].reverse().find((l) => l.status === 'requested');
  }

  private async finishEarly(
    leaseId: string,
    key: string,
    from: VisitStatus[],
    terminal: VisitStatus,
    withReturnTrip: boolean,
  ): Promise<VisitLease> {
    const replay = this.replayByKey(key);
    if (replay) return replay;
    const lease = this.mustGet(leaseId);
    if (!from.includes(lease.status)) throw new Error('INVALID_TRANSITION');
    this.idempotency.set(key, leaseId);
    if (withReturnTrip) {
      this.apply(leaseId, { status: 'returning' });
      this.at(this.travelMs, () => this.transition(leaseId, ['returning'], terminal));
    } else {
      this.apply(leaseId, { status: terminal });
    }
    return this.snapshot(leaseId);
  }

  private replayByKey(key: string): VisitLease | null {
    const leaseId = this.idempotency.get(key);
    return leaseId && this.leases.has(leaseId) ? this.snapshot(leaseId) : null;
  }

  private mustGet(leaseId: string): VisitLease {
    const lease = this.leases.get(leaseId);
    if (!lease) throw new Error('LEASE_NOT_FOUND');
    return lease;
  }

  private apply(leaseId: string, patch: Partial<VisitLease>): void {
    const lease = this.mustGet(leaseId);
    Object.assign(lease, patch);
    this.emitLease(lease);
  }

  /** 守卫式转移：仅当当前状态在 from 里才生效（被召回后迟到的 visiting 事件自然作废）。 */
  private transition(leaseId: string, from: VisitStatus[], to: VisitStatus): boolean {
    const lease = this.leases.get(leaseId);
    if (!lease || !from.includes(lease.status)) return false;
    lease.status = to;
    this.emitLease(lease);
    return true;
  }

  private emitLease(lease: VisitLease): void {
    const copy = { ...lease };
    for (const cb of this.leaseListeners) cb({ ...copy });
  }

  private snapshot(leaseId: string): VisitLease {
    return { ...this.mustGet(leaseId) };
  }

  private projectionOf(leaseId: string): PublicPetProjection {
    return {
      v: 1,
      petId: this.guestPet.petId,
      displayName: this.guestPet.displayName,
      skinId: this.guestPet.skinId,
      status: this.projectionStatus.get(leaseId) ?? 'working',
      updatedAt: this.iso(this.virtualNow),
    };
  }

  private emitProjection(leaseId: string): void {
    const lease = this.leases.get(leaseId);
    if (!lease || isEnded(lease.status)) return; // 发送端守门：租约结束即停播
    const subs = this.projectionSubs.get(leaseId);
    if (!subs) return;
    const payload = this.projectionOf(leaseId);
    for (const cb of subs) cb({ ...payload });
  }

  private startProjectionScript(leaseId: string): void {
    this.projectionStatus.set(leaseId, this.script[0]?.status ?? 'working');
    this.emitProjection(leaseId);
    this.scheduleProjectionStep(leaseId, 0);
  }

  private scheduleProjectionStep(leaseId: string, index: number): void {
    const step = this.script[index % this.script.length];
    if (!step) return;
    this.at(step.ms, () => {
      const lease = this.leases.get(leaseId);
      if (!lease || lease.status !== 'visiting' || this.projectionPaused.has(leaseId)) return;
      const next = this.script[(index + 1) % this.script.length];
      this.projectionStatus.set(leaseId, next.status);
      this.emitProjection(leaseId);
      this.scheduleProjectionStep(leaseId, index + 1);
    });
  }
}
