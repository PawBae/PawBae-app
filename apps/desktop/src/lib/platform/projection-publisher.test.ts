import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createProjectionPublisher, type ProjectionInput } from './projection-publisher';

function fakeClient() {
  const sent: Array<Record<string, unknown>> = [];
  return {
    sent,
    client: {
      rpc(name: string, params?: Record<string, unknown>) {
        sent.push({ name, ...params });
        return Promise.resolve({ error: null });
      },
    },
  };
}

const WORKING: ProjectionInput = { petId: 'yoonie', skinId: 'yoonie', status: 'working' };
const WAITING: ProjectionInput = { ...WORKING, status: 'waiting' };
const IDLE: ProjectionInput = { ...WORKING, status: 'idle' };

describe('projection publisher', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
  });
  afterEach(() => vi.useRealTimers());

  it('publishes immediately when the gate is open, and dedupes identical states', async () => {
    const { client, sent } = fakeClient();
    const p = createProjectionPublisher(() => client, 3000);
    p.setEnabled(true);
    p.publish(WORKING);
    p.publish(WORKING);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sent).toHaveLength(1);
    expect(sent[0]).toEqual({
      name: 'update_projection',
      p_pet_id: 'yoonie',
      p_skin_id: 'yoonie',
      p_status: 'working',
    });
  });

  it('coalesces bursts inside the min interval, last write wins', async () => {
    const { client, sent } = fakeClient();
    const p = createProjectionPublisher(() => client, 3000);
    p.setEnabled(true);
    p.publish(WORKING); // t=0 立即发
    await vi.advanceTimersByTimeAsync(1000);
    p.publish(WAITING); // 间隔内排队
    p.publish(IDLE); // 覆盖排队值
    await vi.advanceTimersByTimeAsync(5000);
    expect(sent.map((s) => s.p_status)).toEqual(['working', 'idle']);
  });

  it('cancels the pending beat when the state bounces back to the last sent value', async () => {
    const { client, sent } = fakeClient();
    const p = createProjectionPublisher(() => client, 3000);
    p.setEnabled(true);
    p.publish(WORKING);
    await vi.advanceTimersByTimeAsync(1000);
    p.publish(WAITING); // 排队
    p.publish(WORKING); // 弹回已发送值 → 撤销排队
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sent.map((s) => s.p_status)).toEqual(['working']);
  });

  it('closing the gate drops the stream and resets dedupe for the next lease window', async () => {
    const { client, sent } = fakeClient();
    const p = createProjectionPublisher(() => client, 3000);
    p.setEnabled(true);
    p.publish(WORKING);
    await vi.advanceTimersByTimeAsync(500);
    p.publish(WAITING); // 排队中
    p.setEnabled(false); // 租约结束：断流 + 清待发
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sent).toHaveLength(1);

    p.publish(IDLE); // 门关着：丢弃
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sent).toHaveLength(1);

    p.setEnabled(true);
    p.publish(WORKING); // 与上个窗口的最后发送值相同，但去重已清 → 必发
    await vi.advanceTimersByTimeAsync(0);
    expect(sent).toHaveLength(2);
  });

  it('a null client (unconfigured build) never throws', async () => {
    const p = createProjectionPublisher(() => null, 3000);
    p.setEnabled(true);
    expect(() => p.publish(WORKING)).not.toThrow();
    await vi.advanceTimersByTimeAsync(5000);
  });
});
