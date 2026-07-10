// 租约状态机纯函数测试：本地相位推导（含 §6 时钟过期）、倒计时展示粒度、
// 乱序/重连收敛（无分身）、单活动访问约束。
import { describe, expect, it } from 'vitest';

import {
  canStartVisit,
  deriveLocalPhase,
  formatRemaining,
  isActive,
  isEnded,
  newIdempotencyKey,
  reconcileLease,
  remainingMs,
} from './lease-machine';
import type { VisitLease, VisitStatus } from './types';

function lease(status: VisitStatus, overrides: Partial<VisitLease> = {}): VisitLease {
  return {
    id: 'lease-1',
    visitorUserId: 'user-a',
    hostUserId: 'user-b',
    status,
    startedAt: '1970-01-01T00:00:00.000Z',
    endsAt: '1970-01-01T00:30:00.000Z', // epoch + 30min
    ...overrides,
  };
}

const MIN = 60_000;

describe('deriveLocalPhase', () => {
  it('maps null to none and requested to pending (pet stays home)', () => {
    expect(deriveLocalPhase(null, 0)).toBe('none');
    expect(deriveLocalPhase(lease('requested', { startedAt: null, endsAt: null }), 0)).toBe(
      'pending',
    );
  });

  it('maps in-flight statuses before endsAt', () => {
    expect(deriveLocalPhase(lease('accepted'), 5 * MIN)).toBe('traveling');
    expect(deriveLocalPhase(lease('traveling'), 5 * MIN)).toBe('traveling');
    expect(deriveLocalPhase(lease('visiting'), 5 * MIN)).toBe('visiting');
    expect(deriveLocalPhase(lease('returning'), 5 * MIN)).toBe('returning');
  });

  it('derives returning locally once the clock passes endsAt (SV §6: no must-arrive end message)', () => {
    expect(deriveLocalPhase(lease('visiting'), 30 * MIN)).toBe('returning');
    expect(deriveLocalPhase(lease('visiting'), 31 * MIN)).toBe('returning');
    expect(deriveLocalPhase(lease('traveling'), 31 * MIN)).toBe('returning');
  });

  it('maps every terminal status to ended', () => {
    for (const s of [
      'completed',
      'declined',
      'cancelled',
      'expired',
      'recalled',
      'blocked',
    ] as const) {
      expect(deriveLocalPhase(lease(s), 0)).toBe('ended');
    }
  });
});

describe('remainingMs / formatRemaining', () => {
  it('returns null before the lease has an endsAt, clamps to 0 after expiry', () => {
    expect(remainingMs(lease('requested', { endsAt: null }), 0)).toBeNull();
    expect(remainingMs(lease('visiting'), 10 * MIN)).toBe(20 * MIN);
    expect(remainingMs(lease('visiting'), 40 * MIN)).toBe(0);
  });

  it('shows minutes above one minute, seconds only inside the last minute（拍板：不催促）', () => {
    expect(formatRemaining(30 * MIN)).toEqual({ unit: 'minutes', value: 30 });
    expect(formatRemaining(61_000)).toEqual({ unit: 'minutes', value: 2 });
    expect(formatRemaining(60_000)).toEqual({ unit: 'seconds', value: 60 });
    expect(formatRemaining(45_000)).toEqual({ unit: 'seconds', value: 45 });
    expect(formatRemaining(500)).toEqual({ unit: 'seconds', value: 1 });
  });
});

describe('status predicates and single-active-visit rule', () => {
  it('splits the 11 statuses into 5 active + 6 ended', () => {
    expect(isActive('requested')).toBe(true);
    expect(isActive('returning')).toBe(true);
    expect(isEnded('recalled')).toBe(true);
    expect(isEnded('blocked')).toBe(true);
  });

  it('canStartVisit only when no lease is active', () => {
    expect(canStartVisit([])).toBe(true);
    expect(canStartVisit([lease('completed'), lease('declined', { id: 'lease-2' })])).toBe(true);
    expect(canStartVisit([lease('completed'), lease('visiting', { id: 'lease-2' })])).toBe(false);
  });
});

describe('newIdempotencyKey', () => {
  it('produces distinct non-empty keys', () => {
    const a = newIdempotencyKey();
    const b = newIdempotencyKey();
    expect(a).toBeTruthy();
    expect(a).not.toBe(b);
  });
});

describe('reconcileLease（无分身收敛）', () => {
  it('takes the incoming lease when nothing is tracked', () => {
    const incoming = lease('visiting');
    expect(reconcileLease(null, incoming)).toBe(incoming);
  });

  it('never resurrects a terminal lease from a stale active echo', () => {
    const current = lease('completed');
    const stale = lease('visiting');
    expect(reconcileLease(current, stale)).toBe(current);
  });

  it('never regresses within the same lease (late accepted after visiting is an echo)', () => {
    const current = lease('visiting');
    expect(reconcileLease(current, lease('accepted'))).toBe(current);
    const forward = lease('returning');
    expect(reconcileLease(current, forward)).toBe(forward);
  });

  it('blocked wins over everything（拉黑最高优先级，不能被旧事件绕过）', () => {
    const blocked = lease('blocked');
    expect(reconcileLease(lease('completed'), blocked)).toBe(blocked);
    expect(reconcileLease(lease('visiting'), blocked)).toBe(blocked);
  });

  it('across different lease ids: live server lease replaces an ended one, but a stale ended lease never clobbers a live one', () => {
    const newLive = lease('traveling', { id: 'lease-2' });
    expect(reconcileLease(lease('completed'), newLive)).toBe(newLive);

    const currentLive = lease('visiting');
    const staleEnded = lease('expired', { id: 'lease-0' });
    expect(reconcileLease(currentLive, staleEnded)).toBe(currentLive);
  });
});
