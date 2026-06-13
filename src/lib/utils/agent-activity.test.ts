import { describe, expect, it } from 'vitest';
import {
  aggregateSessions,
  bubbleKindFor,
  busyCount,
  isOverloaded,
  mascotStateFor,
  OVERLOAD_SESSIONS,
  type SessionStatusLike,
} from './agent-activity';

const s = (...statuses: (string | undefined)[]): SessionStatusLike[] =>
  statuses.map((status) => ({ status }));

describe('aggregateSessions', () => {
  it('buckets known statuses and ignores idle/stopped/unknown', () => {
    const a = aggregateSessions(
      s('processing', 'tool_running', 'compacting', 'waiting', 'idle', 'stopped', 'wat', undefined),
    );
    expect(a).toEqual({ waiting: 1, compacting: 1, working: 2 });
  });

  it('returns all-zero for an empty list', () => {
    expect(aggregateSessions([])).toEqual({ waiting: 0, compacting: 0, working: 0 });
  });
});

describe('busyCount / isOverloaded', () => {
  it('counts every busy kind toward the total', () => {
    expect(busyCount({ waiting: 1, compacting: 1, working: 1 })).toBe(3);
  });

  it('flags overload at the threshold of parallel busy sessions', () => {
    expect(isOverloaded(aggregateSessions(s('processing', 'tool_running')))).toBe(false);
    const atThreshold = aggregateSessions(
      s(...Array.from({ length: OVERLOAD_SESSIONS }, () => 'processing')),
    );
    expect(isOverloaded(atThreshold)).toBe(true);
    // Mixed kinds also add up to the threshold.
    expect(isOverloaded({ waiting: 1, compacting: 1, working: 1 })).toBe(OVERLOAD_SESSIONS <= 3);
  });
});

describe('bubbleKindFor precedence', () => {
  it('waiting outranks compacting outranks working', () => {
    expect(bubbleKindFor({ waiting: 1, compacting: 1, working: 1 })).toBe('waiting');
    expect(bubbleKindFor({ waiting: 0, compacting: 1, working: 2 })).toBe('compacting');
    expect(bubbleKindFor({ waiting: 0, compacting: 0, working: 1 })).toBe('working');
    expect(bubbleKindFor({ waiting: 0, compacting: 0, working: 0 })).toBeNull();
  });
});

describe('mascotStateFor', () => {
  it('mirrors the same precedence and never loses a waiting signal', () => {
    expect(mascotStateFor({ waiting: 1, compacting: 2, working: 3 }, true)).toBe('waiting');
    expect(mascotStateFor({ waiting: 0, compacting: 1, working: 0 }, false)).toBe('compacting');
    expect(mascotStateFor({ waiting: 0, compacting: 0, working: 1 }, false)).toBe('working');
  });

  it('reads health-only activity (OpenClaw agents with no hook status) as working', () => {
    expect(mascotStateFor({ waiting: 0, compacting: 0, working: 0 }, true)).toBe('working');
    expect(mascotStateFor({ waiting: 0, compacting: 0, working: 0 }, false)).toBe('idle');
  });
});
