import { describe, expect, it } from 'vitest';
import {
  DEFAULT_MUSIC_CONFIG,
  initialMusicState,
  type NowPlaying,
  stepMusic,
} from './music-machine';

/** Feed a sequence of samples and return the final step result. */
function run(samples: NowPlaying[]) {
  const s = initialMusicState();
  let last = { listening: false, justEntered: false, justExited: false };
  for (const sample of samples) last = stepMusic(s, sample);
  return { state: s, last };
}

describe('initialMusicState', () => {
  it('starts not listening with zero streaks', () => {
    const s = initialMusicState();
    expect(s.listening).toBe(false);
    expect(s.musicStreak).toBe(0);
    expect(s.silenceStreak).toBe(0);
  });
});

describe('stepMusic — entering', () => {
  it('enters immediately on the first music sample (default enterThreshold = 1)', () => {
    const { state, last } = run(['music']);
    expect(state.listening).toBe(true);
    expect(last.justEntered).toBe(true);
  });

  it('justEntered is true only on the transition tick, not while sustained', () => {
    const s = initialMusicState();
    const enter = stepMusic(s, 'music');
    const sustain = stepMusic(s, 'music');
    expect(enter.justEntered).toBe(true);
    expect(sustain.justEntered).toBe(false);
    expect(sustain.listening).toBe(true);
  });
});

describe('stepMusic — exiting', () => {
  it('does NOT exit on a single silence sample mid-listening (track gap)', () => {
    const s = initialMusicState();
    stepMusic(s, 'music');
    stepMusic(s, 'music'); // listening
    const gap = stepMusic(s, 'none');
    expect(s.listening).toBe(true);
    expect(gap.justExited).toBe(false);
  });

  it('exits after exitThreshold consecutive non-music samples', () => {
    const s = initialMusicState();
    stepMusic(s, 'music');
    stepMusic(s, 'music');
    stepMusic(s, 'none');
    const exit = stepMusic(s, 'none');
    expect(s.listening).toBe(false);
    expect(exit.justExited).toBe(true);
  });

  it('a music sample during the exit window resets the silence streak', () => {
    const s = initialMusicState();
    stepMusic(s, 'music');
    stepMusic(s, 'music'); // listening
    stepMusic(s, 'none'); // silence 1
    stepMusic(s, 'music'); // resets
    const stillGap = stepMusic(s, 'none'); // silence 1 again, not 2
    expect(s.listening).toBe(true);
    expect(stillGap.justExited).toBe(false);
  });
});

describe('stepMusic — video is not music', () => {
  it('treats "video" like "none" — never enters listening', () => {
    const { state } = run(['video', 'video', 'video']);
    expect(state.listening).toBe(false);
  });

  it('exits the listening state when the user switches to a video', () => {
    const s = initialMusicState();
    stepMusic(s, 'music');
    stepMusic(s, 'music');
    stepMusic(s, 'video');
    stepMusic(s, 'video');
    expect(s.listening).toBe(false);
  });
});

describe('stepMusic — flicker resistance (exit hysteresis)', () => {
  it('once listening, alternating none/music stays listening (rides out track gaps)', () => {
    const s = initialMusicState();
    stepMusic(s, 'music'); // listening
    for (const x of ['none', 'music', 'none', 'music', 'none'] as NowPlaying[]) stepMusic(s, x);
    expect(s.listening).toBe(true);
  });
});

describe('stepMusic — custom config', () => {
  it('respects a higher enterThreshold', () => {
    const cfg = { ...DEFAULT_MUSIC_CONFIG, enterThreshold: 3 };
    const s = initialMusicState();
    stepMusic(s, 'music', cfg);
    stepMusic(s, 'music', cfg);
    expect(s.listening).toBe(false);
    stepMusic(s, 'music', cfg);
    expect(s.listening).toBe(true);
  });
});
