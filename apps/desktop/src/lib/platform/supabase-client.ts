// SupabasePlatformClient —— PlatformClient 的真实实现（B 线 W5-6，P4 spec §2-§4）。
// 与 MockPlatformClient 同接口：W7 在 client.ts 一行 DI 换入，上层零改动。
//
// 数据通路（P4 spec 定稿）：
//   - 六个访问动作 = SECURITY DEFINER RPC（request/accept/decline/cancel/recall/end_visit），
//     回包即 visits 规范行——立即喂给 onLeaseChange 订阅者，不等轮询；
//   - 租约变化没有服务端推送通道（visits 不在 Realtime publication 里），
//     onLeaseChange 靠「RPC 回包 + visits 参与者轮询 + visit_ended 广播」三源合流，
//     乱序收敛由上层 reconcileLease 裁决（时间戳才是正确性来源，spec §2）；
//   - 访客投影 = 租约限定私有 Broadcast topic `pet:{visitor_user_id}:{visit_id}`，
//     事件 projection_updated / visit_ended；撤销守门在发送端（服务端触发器
//     锁内复查），客户端只管订阅窗口与租约窗口一致；
//   - 好友 = friendships/profiles/friend_mutes 三表 RLS 直读拼装（无专用 RPC）。
//
// 轮询纪律（B 线祖训）：busy lock 防重入；失败静默等下一个 tick，绝不 stale-discard。

import {
  createMemoryTemplatePayload,
  sanitizePublicPetProjection,
  VISIT_STATUSES,
} from '@pawbae/shared';
import type { Session } from '@supabase/supabase-js';
import { authConfigured, supabaseClient, toPlatformSession } from './auth';
import type {
  FriendEntry,
  InviteEligibility,
  PlatformClient,
  PlatformConnectionState,
  PlatformSession,
  PublicPetProjection,
  PublicProfile,
  SharedMemoryEntry,
  Unsubscribe,
  VisitLease,
  VisitStatus,
} from './types';

export const LEASE_POLL_INTERVAL_MS = 15_000;
/** 单次轮询取回的行数上限：双槽位（出访+接待）活动租约最多 2 张，20 行足够覆盖近期终态。 */
const LEASE_POLL_LIMIT = 20;
/** 相册单次取回上限：v1 相册只展示近况，分页留给后续版本。 */
const MEMORY_FETCH_LIMIT = 30;

// ---------- 注入的 supabase 客户端的结构化窄类型（测试注入假 client 用） ----------

type RpcResult = { data: unknown; error: { message?: string } | null };

type SelectBuilder = PromiseLike<{ data: unknown; error: unknown }> & {
  order(column: string, opts: { ascending: boolean }): SelectBuilder;
  limit(n: number): SelectBuilder;
  eq(column: string, value: string): SelectBuilder;
  in(column: string, values: readonly string[]): SelectBuilder;
  maybeSingle(): PromiseLike<{ data: unknown; error: unknown }>;
};

type ChannelLike = {
  on(
    type: 'broadcast',
    filter: { event: string },
    cb: (message: { payload: unknown }) => void,
  ): ChannelLike;
  subscribe(cb?: (status: string) => void): unknown;
};

export type SupabaseLike = {
  rpc(name: string, params?: Record<string, unknown>): PromiseLike<RpcResult>;
  from(table: string): { select(columns: string): SelectBuilder };
  channel(topic: string, opts: { config: { private: boolean } }): ChannelLike;
  removeChannel(channel: ChannelLike): unknown;
  auth: {
    getSession(): Promise<{ data: { session: unknown } }>;
    onAuthStateChange(cb: (event: string, session: unknown) => void): {
      data: { subscription: { unsubscribe(): void } };
    };
  };
};

// ---------- 行映射 ----------

const VISIT_STATUS_SET: ReadonlySet<string> = new Set(VISIT_STATUSES);

function isVisitStatus(value: unknown): value is VisitStatus {
  return typeof value === 'string' && VISIT_STATUS_SET.has(value);
}

function asNullableString(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

/** visits 规范行（snake_case）→ 契约 VisitLease。形状不对抛 TypeError（fail-closed）。 */
export function toVisitLease(row: unknown): VisitLease {
  if (typeof row !== 'object' || row === null) throw new TypeError('visit row must be an object');
  const r = row as Record<string, unknown>;
  if (typeof r.id !== 'string' || r.id === '') throw new TypeError('visit row.id must be a string');
  if (typeof r.visitor_user_id !== 'string' || typeof r.host_user_id !== 'string') {
    throw new TypeError('visit row participants must be strings');
  }
  if (!isVisitStatus(r.status)) throw new TypeError('visit row.status is not a VisitStatus');
  return Object.freeze({
    id: r.id,
    visitorUserId: r.visitor_user_id,
    hostUserId: r.host_user_id,
    status: r.status,
    startedAt: asNullableString(r.started_at),
    endsAt: asNullableString(r.ends_at),
  });
}

function sameLease(a: VisitLease, b: VisitLease): boolean {
  return a.status === b.status && a.startedAt === b.startedAt && a.endsAt === b.endsAt;
}

/**
 * shared_memories 规范行（snake_case）→ 契约 SharedMemoryEntry。模板键与安全参数
 * 走共享契约校验器（createMemoryTemplatePayload）——形状不对抛错（fail-closed）。
 */
export function toSharedMemoryEntry(row: unknown): SharedMemoryEntry {
  if (typeof row !== 'object' || row === null) {
    throw new TypeError('memory row must be an object');
  }
  const r = row as Record<string, unknown>;
  for (const key of ['id', 'visit_id', 'visitor_user_id', 'host_user_id', 'created_at'] as const) {
    if (typeof r[key] !== 'string' || r[key] === '') {
      throw new TypeError(`memory row.${key} must be a non-empty string`);
    }
  }
  const payload = createMemoryTemplatePayload(r.template_key, r.params);
  return Object.freeze({
    id: r.id as string,
    visitId: r.visit_id as string,
    visitorUserId: r.visitor_user_id as string,
    hostUserId: r.host_user_id as string,
    templateKey: payload.templateKey,
    params: payload.params,
    createdAt: r.created_at as string,
  });
}

function errorMessage(error: { message?: string } | null): string {
  return error?.message || 'PLATFORM_RPC_FAILED';
}

/**
 * Broadcast 投影帧 → PublicPetProjection。Realtime 会往 payload 注入一个
 * transport 元数据 `id`（UUID），共享清洗器要求恰好六键——先剥再洗（spec §4）。
 * 形状不合法返回 null（丢帧，不让一条坏消息打断订阅）。
 */
export function parseProjectionFrame(payload: unknown): PublicPetProjection | null {
  if (typeof payload !== 'object' || payload === null) return null;
  const { id: _transportId, ...frame } = payload as Record<string, unknown>;
  try {
    return sanitizePublicPetProjection(frame);
  } catch (e) {
    console.warn('[platform] dropped malformed projection frame:', e);
    return null;
  }
}

// ---------- 实现 ----------

export class SupabasePlatformClient implements PlatformClient {
  private currentSession: PlatformSession | null = null;
  private sessionListeners = new Set<(s: PlatformSession | null) => void>();
  private connectionListeners = new Set<(state: PlatformConnectionState) => void>();
  private leaseListeners = new Set<(lease: VisitLease) => void>();
  private currentConnectionState: PlatformConnectionState = 'degraded';
  private sessionGeneration = 0;
  /** 已发射租约的最后形态：轮询去抖（内容没变不重发）+ visit_ended 合成的底稿。 */
  private lastLeases = new Map<string, VisitLease>();
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private pollInFlight = false;
  private authUnsub: (() => void) | null = null;

  constructor(
    private readonly clientFn: () => SupabaseLike | null,
    private readonly pollIntervalMs = LEASE_POLL_INTERVAL_MS,
  ) {}

  /** App 挂载时调用一次：恢复持久化会话 + 订阅认证变化。未配置时是 no-op。 */
  async start(): Promise<void> {
    const client = this.clientFn();
    if (!client) return;
    const { data } = await client.auth.getSession();
    const initial = toPlatformSession(data.session as Session | null);
    this.applySession(await this.canonicalSession(client, initial));
    const { data: sub } = client.auth.onAuthStateChange((_event, session) => {
      const generation = ++this.sessionGeneration;
      const fallback = toPlatformSession(session as Session | null);
      this.applySession(fallback);
      if (fallback) {
        void this.canonicalSession(client, fallback).then((canonical) => {
          if (generation === this.sessionGeneration) this.applySession(canonical);
        });
      }
    });
    this.authUnsub = () => sub.subscription.unsubscribe();
  }

  dispose(): void {
    this.stopPolling();
    this.authUnsub?.();
    this.authUnsub = null;
  }

  // ---------- 会话 ----------

  session(): PlatformSession | null {
    return this.currentSession;
  }

  onSessionChange(cb: (s: PlatformSession | null) => void): Unsubscribe {
    this.sessionListeners.add(cb);
    return () => this.sessionListeners.delete(cb);
  }

  connectionState(): PlatformConnectionState {
    return this.currentConnectionState;
  }

  onConnectionStateChange(cb: (state: PlatformConnectionState) => void): Unsubscribe {
    this.connectionListeners.add(cb);
    return () => this.connectionListeners.delete(cb);
  }

  private setConnectionState(next: PlatformConnectionState): void {
    if (next === this.currentConnectionState) return;
    this.currentConnectionState = next;
    for (const cb of this.connectionListeners) cb(next);
  }

  private async canonicalSession(
    client: SupabaseLike,
    fallback: PlatformSession | null,
  ): Promise<PlatformSession | null> {
    if (!fallback) return null;
    const { data, error } = await client
      .from('profiles')
      .select('handle,display_name,avatar_url')
      .eq('id', fallback.userId)
      .maybeSingle();
    if (error || typeof data !== 'object' || data === null) return fallback;
    const row = data as Record<string, unknown>;
    if (typeof row.handle !== 'string' || row.handle === '') return fallback;
    return {
      userId: fallback.userId,
      handle: row.handle,
      displayName: asNullableString(row.display_name) ?? fallback.displayName,
      avatarUrl: asNullableString(row.avatar_url) ?? fallback.avatarUrl,
    };
  }

  private applySession(next: PlatformSession | null): void {
    if (next?.userId === this.currentSession?.userId) {
      this.currentSession = next; // 同人 token 刷新：更新引用但不广播不重启轮询
      return;
    }
    this.currentSession = next;
    this.setConnectionState(next ? 'connected' : 'degraded');
    if (next === null) this.lastLeases.clear(); // 换号不吃前任的去抖记忆
    for (const cb of this.sessionListeners) cb(next);
    this.syncPolling();
  }

  // ---------- 串门 ----------

  async requestVisit(hostUserId: string, idempotencyKey: string): Promise<VisitLease> {
    return this.visitRpc('request_visit', {
      p_host_user_id: hostUserId,
      p_idempotency_key: idempotencyKey,
    });
  }

  async respondVisit(
    leaseId: string,
    action: 'accept' | 'decline',
    key: string,
  ): Promise<VisitLease> {
    return this.visitRpc(action === 'accept' ? 'accept_visit' : 'decline_visit', {
      p_visit_id: leaseId,
      p_idempotency_key: key,
    });
  }

  async cancelVisit(leaseId: string, key: string): Promise<VisitLease> {
    return this.visitRpc('cancel_visit', { p_visit_id: leaseId, p_idempotency_key: key });
  }

  async recallVisit(leaseId: string, key: string): Promise<VisitLease> {
    return this.visitRpc('recall_visit', { p_visit_id: leaseId, p_idempotency_key: key });
  }

  async endVisit(leaseId: string, key: string): Promise<VisitLease> {
    return this.visitRpc('end_visit', { p_visit_id: leaseId, p_idempotency_key: key });
  }

  onLeaseChange(cb: (lease: VisitLease) => void): Unsubscribe {
    this.leaseListeners.add(cb);
    this.syncPolling();
    return () => {
      this.leaseListeners.delete(cb);
      this.syncPolling();
    };
  }

  subscribeGuestProjection(lease: VisitLease, cb: (p: PublicPetProjection) => void): Unsubscribe {
    const client = this.clientFn();
    if (!client) return () => {};
    const topic = `pet:${lease.visitorUserId}:${lease.id}`;
    const channel = client
      .channel(topic, { config: { private: true } })
      .on('broadcast', { event: 'projection_updated' }, ({ payload }) => {
        const projection = parseProjectionFrame(payload);
        if (projection) cb(projection);
      })
      .on('broadcast', { event: 'visit_ended' }, ({ payload }) => {
        this.ingestVisitEnded(lease, payload);
      });
    this.setConnectionState('reconnecting');
    channel.subscribe((status) => {
      if (status === 'SUBSCRIBED') this.setConnectionState('connected');
      else if (status === 'CHANNEL_ERROR' || status === 'TIMED_OUT' || status === 'CLOSED') {
        this.setConnectionState('degraded');
      } else {
        this.setConnectionState('reconnecting');
      }
    });
    // 订阅立即回放当前投影（与 mock 语义一致）：下一次状态变化前访客不该是空白。
    // RLS 只在活跃租约窗口内放行读取，读不到就静默等广播。
    void this.fetchProjectionOnce(client, lease.visitorUserId, cb);
    return () => {
      client.removeChannel(channel);
      this.setConnectionState(this.currentSession ? 'connected' : 'degraded');
    };
  }

  // ---------- 邀请码 / 好友 ----------

  async redeemInvite(code: string, key: string): Promise<void> {
    const client = this.mustClient();
    const { error } = await client.rpc('redeem_invite', {
      p_code: code,
      p_idempotency_key: key,
    });
    if (error) throw new Error(errorMessage(error));
  }

  async inviteEligibility(): Promise<InviteEligibility> {
    const me = this.currentSession?.userId;
    const client = this.clientFn();
    if (!me || !client) return { redeemed: false, redeemedAt: null };
    const { data, error } = await client
      .from('invite_redemptions')
      .select('created_at')
      .eq('user_id', me)
      .limit(1);
    if (error) throw new Error(errorMessage(error as { message?: string }));
    const first = Array.isArray(data)
      ? (data[0] as Record<string, unknown> | undefined)
      : undefined;
    return {
      redeemed: Boolean(first),
      redeemedAt: first ? asNullableString(first.created_at) : null,
    };
  }

  async friends(): Promise<FriendEntry[]> {
    const me = this.currentSession?.userId;
    const client = this.clientFn();
    if (!me || !client) return [];

    const { data: rows, error } = await client
      .from('friendships')
      .select('user_a,user_b,requester_id,status');
    if (error) throw new Error(errorMessage(error as { message?: string }));
    const friendships = Array.isArray(rows) ? (rows as Record<string, unknown>[]) : [];
    const others = friendships
      .map((r) => (r.user_a === me ? r.user_b : r.user_a))
      .filter((id): id is string => typeof id === 'string' && id !== me);
    if (others.length === 0) return [];

    const [profilesRes, mutesRes] = await Promise.all([
      client.from('profiles').select('id,handle,display_name').in('id', others),
      client.from('friend_mutes').select('muted_user_id,muted'),
    ]);
    const profiles = new Map<string, { handle: string; displayName: string | null }>();
    if (Array.isArray(profilesRes.data)) {
      for (const p of profilesRes.data as Record<string, unknown>[]) {
        if (typeof p.id === 'string' && typeof p.handle === 'string') {
          profiles.set(p.id, { handle: p.handle, displayName: asNullableString(p.display_name) });
        }
      }
    }
    const muted = new Set<string>();
    if (Array.isArray(mutesRes.data)) {
      for (const m of mutesRes.data as Record<string, unknown>[]) {
        if (m.muted === true && typeof m.muted_user_id === 'string') muted.add(m.muted_user_id);
      }
    }

    const entries: FriendEntry[] = [];
    for (const row of friendships) {
      const other = row.user_a === me ? row.user_b : row.user_a;
      if (typeof other !== 'string') continue;
      const profile = profiles.get(other);
      if (!profile) continue; // 被拉黑的对端 profiles RLS 不放行——列表里直接不出现
      entries.push({
        userId: other,
        handle: profile.handle,
        displayName: profile.displayName,
        relation:
          row.status === 'accepted'
            ? 'accepted'
            : row.requester_id === me
              ? 'pending_out'
              : 'pending_in',
        muted: muted.has(other),
      });
    }
    return entries.sort((a, b) => a.handle.localeCompare(b.handle));
  }

  async findProfileByHandle(handle: string): Promise<PublicProfile | null> {
    const normalized = handle.trim().replace(/^@/, '').toLowerCase();
    if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(normalized) || normalized.length > 39) {
      throw new Error('invalid_github_handle');
    }
    const client = this.mustClient();
    const { data, error } = await client
      .from('profiles')
      .select('id,handle,display_name,avatar_url')
      .eq('handle', normalized)
      .maybeSingle();
    if (error) throw new Error(errorMessage(error as { message?: string }));
    if (typeof data !== 'object' || data === null) return null;
    const row = data as Record<string, unknown>;
    if (typeof row.id !== 'string' || typeof row.handle !== 'string') return null;
    return {
      userId: row.id,
      handle: row.handle,
      displayName: asNullableString(row.display_name),
      avatarUrl: asNullableString(row.avatar_url),
    };
  }

  async sendFriendRequest(userId: string): Promise<void> {
    await this.socialRpc('send_friend_request', { p_target_user_id: userId });
  }

  async acceptFriendRequest(userId: string): Promise<void> {
    await this.socialRpc('accept_friend_request', { p_requester_user_id: userId });
  }

  async unfriend(userId: string): Promise<void> {
    await this.socialRpc('unfriend', { p_other_user_id: userId });
  }

  async muteUser(userId: string, muted: boolean): Promise<void> {
    await this.socialRpc('mute_user', { p_target_user_id: userId, p_muted: muted });
  }

  async blockUser(userId: string): Promise<void> {
    await this.socialRpc('block_user', { p_target_user_id: userId });
  }

  // ---------- 共同记忆（P4-C 数据面） ----------

  async settleMemory(visitId: string, key: string): Promise<SharedMemoryEntry> {
    const client = this.mustClient();
    const { data, error } = await client.rpc('settle_shared_memory', {
      p_visit_id: visitId,
      p_idempotency_key: key,
    });
    if (error) throw new Error(errorMessage(error));
    return toSharedMemoryEntry(data);
  }

  async sharedMemories(): Promise<SharedMemoryEntry[]> {
    const client = this.clientFn();
    if (!client || this.currentSession === null) return [];
    const { data, error } = await client
      .from('shared_memories')
      .select('id,visit_id,visitor_user_id,host_user_id,template_key,params,created_at')
      .order('created_at', { ascending: false })
      .limit(MEMORY_FETCH_LIMIT);
    if (error) throw new Error(errorMessage(error as { message?: string }));
    if (!Array.isArray(data)) return [];
    const entries: SharedMemoryEntry[] = [];
    for (const row of data) {
      try {
        entries.push(toSharedMemoryEntry(row));
      } catch (e) {
        console.warn('[platform] dropped malformed memory row:', e);
      }
    }
    return entries;
  }

  async recordMemoryView(memoryId: string, key: string): Promise<void> {
    const client = this.mustClient();
    const { error } = await client.rpc('record_memory_view', {
      p_memory_id: memoryId,
      p_idempotency_key: key,
    });
    if (error) throw new Error(errorMessage(error));
  }

  // ---------- 内部 ----------

  private mustClient(): SupabaseLike {
    const client = this.clientFn();
    if (!client) throw new Error('PLATFORM_NOT_CONFIGURED');
    return client;
  }

  private async socialRpc(name: string, params: Record<string, unknown>): Promise<void> {
    const { error } = await this.mustClient().rpc(name, params);
    if (error) throw new Error(errorMessage(error));
  }

  private async visitRpc(name: string, params: Record<string, unknown>): Promise<VisitLease> {
    const client = this.mustClient();
    const { data, error } = await client.rpc(name, params);
    if (error) throw new Error(errorMessage(error));
    const lease = toVisitLease(data);
    this.ingestLease(lease); // 回包即事实：先于下一次轮询喂给订阅者
    return lease;
  }

  private ingestLease(lease: VisitLease): void {
    const prev = this.lastLeases.get(lease.id);
    if (prev && sameLease(prev, lease)) return;
    this.lastLeases.set(lease.id, lease);
    for (const cb of this.leaseListeners) cb(lease);
  }

  /**
   * visit_ended 广播只带 {leaseId, status, endedAt}——用缓存底稿合成完整租约发射。
   * 只改 status：时间戳以 visits 行为准（下一次轮询会带来规范行，reconcile 只进不退）。
   */
  private ingestVisitEnded(fallback: VisitLease, payload: unknown): void {
    if (typeof payload !== 'object' || payload === null) return;
    const p = payload as Record<string, unknown>;
    if (typeof p.leaseId !== 'string' || !isVisitStatus(p.status)) return;
    const base = this.lastLeases.get(p.leaseId) ?? (p.leaseId === fallback.id ? fallback : null);
    if (!base) return;
    this.ingestLease(Object.freeze({ ...base, status: p.status }));
  }

  private async fetchProjectionOnce(
    client: SupabaseLike,
    ownerUserId: string,
    cb: (p: PublicPetProjection) => void,
  ): Promise<void> {
    try {
      const { data } = await client
        .from('pet_projections')
        .select('version,pet_id,display_name,skin_id,status,updated_at')
        .eq('owner_user_id', ownerUserId)
        .maybeSingle();
      if (typeof data !== 'object' || data === null) return;
      const row = data as Record<string, unknown>;
      const projection = parseProjectionFrame({
        v: row.version,
        petId: row.pet_id,
        displayName: row.display_name,
        skinId: row.skin_id,
        status: row.status,
        updatedAt: row.updated_at,
      });
      if (projection) cb(projection);
    } catch {
      // 静默：初始回放是锦上添花，广播才是主通道
    }
  }

  private syncPolling(): void {
    const shouldPoll = this.currentSession !== null && this.leaseListeners.size > 0;
    if (shouldPoll && this.pollTimer === null) {
      void this.pollLeases(); // 开闸沿先拉一次，登录后立即看到挂起邀请
      this.pollTimer = setInterval(() => void this.pollLeases(), this.pollIntervalMs);
    } else if (!shouldPoll && this.pollTimer !== null) {
      this.stopPolling();
    }
  }

  private stopPolling(): void {
    if (this.pollTimer !== null) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  private async pollLeases(): Promise<void> {
    if (this.pollInFlight) return; // busy lock：上一次没回来就跳过本 tick
    const client = this.clientFn();
    if (!client) return;
    this.pollInFlight = true;
    try {
      const { data, error } = await client
        .from('visits')
        .select('id,visitor_user_id,host_user_id,status,started_at,ends_at')
        .order('updated_at', { ascending: false })
        .limit(LEASE_POLL_LIMIT);
      if (error || !Array.isArray(data)) return; // 静默：下一个 tick 重试
      for (const row of data) {
        try {
          this.ingestLease(toVisitLease(row));
        } catch (e) {
          console.warn('[platform] dropped malformed visit row:', e);
        }
      }
    } catch {
      // 静默：网络抖动交给下一个 tick
    } finally {
      this.pollInFlight = false;
    }
  }
}

/** App 单例工厂：W7 在 client.ts 里换入 `createSupabasePlatformClient()` 并调用 start()。 */
export function createSupabasePlatformClient(): SupabasePlatformClient {
  return new SupabasePlatformClient(() =>
    authConfigured ? (supabaseClient() as unknown as SupabaseLike) : null,
  );
}
