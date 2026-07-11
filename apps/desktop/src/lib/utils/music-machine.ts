// Hysteresis state machine for the "pet is listening to music" reaction.
//
// Pure logic, zero Svelte/Tauri imports — mirrors `reaction-machine.ts` so it is
// unit-testable without mounting a component or running on macOS. The Svelte layer
// (MascotView) polls the Rust `get_now_playing` command and feeds the samples in here.
//
// Why hysteresis: `get_now_playing` flips to "none" for a beat between tracks, during
// ads, or while a player buffers. A naive `listening = sample === 'music'` would make the
// pet snap in and out of the music state every few seconds. We require N consecutive
// "music" reads to ENTER and N consecutive non-"music" reads to EXIT, so brief gaps are
// absorbed.

/** Raw result of the Rust `get_now_playing` command. */
export type NowPlaying = 'music' | 'video' | 'none';

export interface MusicMachineConfig {
  /** Consecutive "music" samples required to enter the listening state. */
  enterThreshold: number;
  /** Consecutive non-"music" samples required to leave the listening state. */
  exitThreshold: number;
}

// Enter on the FIRST "music" sample so the pet reacts promptly (~one poll). The menu-bar
// play/pause label the detector reads is stable — there's no transient blip to debounce on
// the way in — so a 1-sample entry doesn't cause false starts. Exit still needs 2
// consecutive non-"music" samples so the brief "none" between tracks doesn't drop the pet
// out of the listening state.
export const DEFAULT_MUSIC_CONFIG: MusicMachineConfig = {
  enterThreshold: 1,
  exitThreshold: 2,
};

export interface MusicState {
  /** True while the pet should show the listening reaction. */
  listening: boolean;
  /** Consecutive "music" samples seen so far. */
  musicStreak: number;
  /** Consecutive non-"music" samples seen so far. */
  silenceStreak: number;
}

export interface MusicStep {
  listening: boolean;
  /** True only on the single tick the pet enters the listening state. */
  justEntered: boolean;
  /** True only on the single tick the pet leaves the listening state. */
  justExited: boolean;
}

export function initialMusicState(): MusicState {
  return { listening: false, musicStreak: 0, silenceStreak: 0 };
}

/**
 * Feed one `get_now_playing` sample. Mutates `s` in place (mirrors the physics `step()`
 * and reaction-machine conventions) and returns the edge-flags the caller acts on.
 * `video` and `none` are both treated as "not music" — watching a video must not put the
 * pet into the listening state.
 */
export function stepMusic(
  s: MusicState,
  sample: NowPlaying,
  cfg: MusicMachineConfig = DEFAULT_MUSIC_CONFIG,
): MusicStep {
  const isMusic = sample === 'music';

  if (isMusic) {
    s.musicStreak += 1;
    s.silenceStreak = 0;
  } else {
    s.silenceStreak += 1;
    s.musicStreak = 0;
  }

  let justEntered = false;
  let justExited = false;

  if (!s.listening && isMusic && s.musicStreak >= cfg.enterThreshold) {
    s.listening = true;
    justEntered = true;
  } else if (s.listening && !isMusic && s.silenceStreak >= cfg.exitThreshold) {
    s.listening = false;
    justExited = true;
  }

  return { listening: s.listening, justEntered, justExited };
}
