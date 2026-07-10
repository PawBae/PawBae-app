// 租约状态机的客户端纯函数层（SV spec §3.2 / §6）。核心原则：
//   1. 「时间到没到」一律以 endsAt 本地推导——结束消息不是必达的（§6）；
//   2. 收敛永不复活已终止的租约，绝不出现两个宠物实例（无分身）；
//   3. blocked 是最高优先级终态，任何旧事件都不能绕过它（§6/§7）。
// 全部纯函数，UI 层（MascotView / 空窝牌）与 mock/真实 PlatformClient 共用。

import type { VisitLease, VisitStatus } from './types';

const ENDED_STATUSES: ReadonlySet<VisitStatus> = new Set([
  'completed',
  'declined',
  'cancelled',
  'expired',
  'recalled',
  'blocked',
]);

// 同一租约内乱序事件的裁决序：只进不退（visiting 之后迟到的 accepted 是回声，不是真相）
const STATUS_RANK: Record<VisitStatus, number> = {
  requested: 0,
  accepted: 1,
  traveling: 2,
  visiting: 3,
  returning: 4,
  completed: 5,
  declined: 5,
  cancelled: 5,
  expired: 5,
  recalled: 5,
  blocked: 6, // 拉黑最高优先级
};

export function isEnded(status: VisitStatus): boolean {
  return ENDED_STATUSES.has(status);
}

export function isActive(status: VisitStatus): boolean {
  return !ENDED_STATUSES.has(status);
}

/** v1 约束：同一只宠物同时只能有一个活动访问（服务端由部分唯一索引兜底）。 */
export function canStartVisit(leases: readonly VisitLease[]): boolean {
  return leases.every((lease) => isEnded(lease.status));
}

/**
 * UI 视角的本地相位。与 VisitStatus 的区别：叠加了 endsAt 时钟推导——
 * 服务端还认为 visiting 但本地时钟已过期时，这里直接给 returning，
 * 主人端恢复 home、好友端移走访客，不等一条可能永远不来的结束消息。
 */
export type LocalVisitPhase = 'none' | 'pending' | 'traveling' | 'visiting' | 'returning' | 'ended';

export function deriveLocalPhase(lease: VisitLease | null, nowMs: number): LocalVisitPhase {
  if (lease === null) return 'none';
  if (isEnded(lease.status)) return 'ended';
  switch (lease.status) {
    case 'requested':
      return 'pending'; // 邀请挂起，宠物留在家（§6：对方接受后才离家）
    case 'accepted':
    case 'traveling':
      return isClockExpired(lease, nowMs) ? 'returning' : 'traveling';
    case 'visiting':
      return isClockExpired(lease, nowMs) ? 'returning' : 'visiting';
    case 'returning':
      return 'returning';
    default:
      return 'ended';
  }
}

function isClockExpired(lease: VisitLease, nowMs: number): boolean {
  if (lease.endsAt === null) return false;
  return Date.parse(lease.endsAt) <= nowMs;
}

/** 剩余毫秒；未开始（无 endsAt）返回 null，已过期钳到 0。 */
export function remainingMs(lease: VisitLease, nowMs: number): number | null {
  if (lease.endsAt === null) return null;
  return Math.max(0, Date.parse(lease.endsAt) - nowMs);
}

/**
 * 空窝牌倒计时的展示粒度（过场原型拍板）：分钟级显示，最后一分钟才走秒——
 * 秒级跳动盯着看会变成催促。返回结构化值，文案由 UI 层按 locale 生成。
 */
export interface RemainingDisplay {
  unit: 'minutes' | 'seconds';
  value: number;
}

export function formatRemaining(ms: number): RemainingDisplay {
  if (ms > 60_000) return { unit: 'minutes', value: Math.ceil(ms / 60_000) };
  return { unit: 'seconds', value: Math.ceil(ms / 1_000) };
}

/**
 * 幂等键：每个「用户意图」生成一次，重试复用同一个键（服务端
 * unique(actor_id, idempotency_key) 去重）。不要在重试循环里重新生成。
 */
export function newIdempotencyKey(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `idem-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

/**
 * 重连/乱序收敛（§6：重连后以服务端租约收敛，不合并出两个宠物实例）：
 *   - 不同租约：活的压过死的；两个都活着以 incoming（服务端最新）为准；
 *   - 同一租约：状态只进不退，已终止不被迟到的活跃事件复活；
 *   - blocked 永远赢——它不能被自动欢迎或旧邀请绕过。
 */
export function reconcileLease(current: VisitLease | null, incoming: VisitLease): VisitLease {
  if (current === null) return incoming;
  if (incoming.status === 'blocked') return incoming;
  if (current.id !== incoming.id) {
    if (isActive(incoming.status)) return incoming; // 服务端开了新租约，旧的让位
    return isEnded(current.status) ? incoming : current; // 迟到的旧终局别压住活租约
  }
  return STATUS_RANK[incoming.status] >= STATUS_RANK[current.status] ? incoming : current;
}
