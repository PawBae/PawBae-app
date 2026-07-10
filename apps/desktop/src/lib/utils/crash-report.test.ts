import { describe, expect, it, vi } from 'vitest';
import { createErrorReporter } from './crash-report';

describe('crash-report', () => {
  it('reports an error with truncated message and stack', () => {
    const invokeFn = vi.fn().mockResolvedValue(undefined);
    const { report } = createErrorReporter(invokeFn);
    report('error', 'boom', 'stack-line');
    expect(invokeFn).toHaveBeenCalledWith('report_frontend_error', {
      kind: 'error',
      message: 'boom',
      stack: 'stack-line',
    });
  });

  it('dedupes identical messages', () => {
    const invokeFn = vi.fn().mockResolvedValue(undefined);
    const { report } = createErrorReporter(invokeFn);
    report('error', 'same');
    report('error', 'same');
    report('rejection', 'same'); // 不同 kind 算不同 key
    expect(invokeFn).toHaveBeenCalledTimes(2);
  });

  it('caps total reports per run', () => {
    const invokeFn = vi.fn().mockResolvedValue(undefined);
    const { report } = createErrorReporter(invokeFn);
    for (let i = 0; i < 50; i++) {
      report('error', `msg-${i}`);
    }
    expect(invokeFn).toHaveBeenCalledTimes(30);
  });

  it('truncates oversized payloads and nulls missing stack', () => {
    const invokeFn = vi.fn().mockResolvedValue(undefined);
    const { report } = createErrorReporter(invokeFn);
    report('rejection', 'x'.repeat(5000));
    const args = invokeFn.mock.calls[0][1] as { message: string; stack: string | null };
    expect(args.message.length).toBe(2000);
    expect(args.stack).toBeNull();
  });

  it('swallows invoke failures', () => {
    const invokeFn = vi.fn().mockRejectedValue(new Error('ipc down'));
    const { report } = createErrorReporter(invokeFn);
    expect(() => report('error', 'boom')).not.toThrow();
  });
});
