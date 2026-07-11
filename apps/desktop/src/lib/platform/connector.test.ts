import { createTaskCompletedEvent } from '@pawbae/shared';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createConnector, crossedStreakMilestone } from './connector';

function fakeClient() {
  const calls: { rpc: string[]; inserts: Array<Record<string, unknown>> } = {
    rpc: [],
    inserts: [],
  };
  const client = {
    rpc(name: string) {
      calls.rpc.push(name);
      return Promise.resolve({ error: null });
    },
    from(_table: string) {
      return {
        insert(row: Record<string, unknown>) {
          calls.inserts.push(row);
          return Promise.resolve({ error: null });
        },
      };
    },
  };
  return { client, calls };
}

const ALL_ON = {
  signedIn: true,
  connectEnabled: true,
  uploads: {
    task_completed: true,
    egg_hatched: true,
    souvenir_found: true,
    streak_milestone: true,
  },
} as const;

describe('connector heartbeat', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('beats immediately on gates opening, then every interval', async () => {
    const { client, calls } = fakeClient();
    const c = createConnector(() => client, 1000);
    c.configure({ ...ALL_ON });
    await vi.advanceTimersByTimeAsync(0);
    expect(calls.rpc).toEqual(['connector_heartbeat']);
    await vi.advanceTimersByTimeAsync(3000);
    expect(calls.rpc).toHaveLength(4);
    c.stop();
  });

  it('never beats while any gate is closed, and stops when a gate closes', async () => {
    const { client, calls } = fakeClient();
    const c = createConnector(() => client, 1000);
    c.configure({ ...ALL_ON, connectEnabled: false });
    await vi.advanceTimersByTimeAsync(5000);
    expect(calls.rpc).toHaveLength(0);

    c.configure({ ...ALL_ON });
    await vi.advanceTimersByTimeAsync(1000);
    expect(calls.rpc.length).toBeGreaterThan(0);
    const beforeClose = calls.rpc.length;
    c.configure({ ...ALL_ON, signedIn: false });
    await vi.advanceTimersByTimeAsync(5000);
    expect(calls.rpc).toHaveLength(beforeClose);
    c.stop();
  });

  it('survives a rejecting client (silent retry on next tick)', async () => {
    const client = {
      rpc: () => Promise.reject(new Error('offline')),
      from: () => ({ insert: () => Promise.resolve({ error: null }) }),
    };
    const c = createConnector(() => client, 1000);
    c.configure({ ...ALL_ON });
    // 三拍全部被拒也不能抛出未处理拒绝（走到这里不炸即通过）
    await vi.advanceTimersByTimeAsync(2500);
    c.stop();
  });
});

describe('connector event upload', () => {
  it('uploads only when master gate AND the per-kind switch are open', async () => {
    const { client, calls } = fakeClient();
    const c = createConnector(() => client, 60_000);
    const event = createTaskCompletedEvent({ source: 'cc' });

    c.uploadEvent(event); // gates closed
    c.configure({ ...ALL_ON, uploads: { ...ALL_ON.uploads, task_completed: false } });
    c.uploadEvent(event); // kind switch closed
    await Promise.resolve();
    expect(calls.inserts).toHaveLength(0);

    c.configure({ ...ALL_ON });
    c.uploadEvent(event);
    await vi.waitFor(() => expect(calls.inserts).toHaveLength(1));
    expect(calls.inserts[0]).toEqual({ kind: 'task_completed', params: { source: 'cc' } });
    c.stop();
  });

  it('no client (unconfigured build) is a silent no-op', () => {
    const c = createConnector(() => null, 60_000);
    c.configure({ ...ALL_ON });
    expect(() => c.uploadEvent(createTaskCompletedEvent({ source: 'codex' }))).not.toThrow();
    c.stop();
  });
});

describe('crossedStreakMilestone', () => {
  it('fires exactly when a milestone is crossed', () => {
    expect(crossedStreakMilestone(2, 3)).toBe(3);
    expect(crossedStreakMilestone(6, 7)).toBe(7);
    expect(crossedStreakMilestone(3, 4)).toBeNull();
    expect(crossedStreakMilestone(7, 8)).toBeNull();
  });

  it('handles jumps and resets (forgiving streak can skip)', () => {
    expect(crossedStreakMilestone(5, 20)).toBe(7); // 跳变取最先跨过的里程碑
    expect(crossedStreakMilestone(30, 30)).toBeNull();
    expect(crossedStreakMilestone(10, 2)).toBeNull(); // 回落不发
    expect(crossedStreakMilestone(0, 1)).toBeNull();
  });
});
