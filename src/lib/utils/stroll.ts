// Stroll-mode gate for MascotView's physics effect.
//
// `pushStrollMode` is the value the effect must forward to the Rust
// `set_stroll_mode` command; `null` means "don't push" — those paths rely on
// the previous effect run's teardown having already pushed `false`. The
// explicit `false` push when the user disabled stroll exists so the macOS tray
// checkbox matches settings.json right after startup, when no teardown has run
// yet (Rust boots with stroll_enabled = true).
export interface StrollGateInput {
  /** The selected pet declares physics support. */
  physicsCapable: boolean;
  /** Settings panel is open — it resizes the window, so physics is torn down. */
  settingsOpen: boolean;
  /** Persisted user setting (settings.json `stroll_mode_enabled`). */
  strollEnabled: boolean;
  /**
   * The pet is off on an adventure (or mid depart/return transition) — the window
   * must stay put under the ⛺ marker, so native stroll is pushed off too.
   */
  away?: boolean;
}

export interface StrollGateResult {
  runLoop: boolean;
  pushStrollMode: boolean | null;
}

export function strollGate(input: StrollGateInput): StrollGateResult {
  if (!input.physicsCapable || input.settingsOpen) {
    return { runLoop: false, pushStrollMode: null };
  }
  if (input.away) {
    return { runLoop: false, pushStrollMode: false };
  }
  if (!input.strollEnabled) {
    return { runLoop: false, pushStrollMode: false };
  }
  return { runLoop: true, pushStrollMode: true };
}
