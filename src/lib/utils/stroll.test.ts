// Stroll-mode gate: decides whether MascotView's physics loop may run and what
// value (if any) to push to Rust `set_stroll_mode`. The push keeps the macOS
// tray checkbox in sync with settings.json, which the frontend owns.
import { describe, expect, it } from 'vitest';

import { strollGate } from './stroll';

describe('strollGate', () => {
  it('runs the loop and pushes enabled when physics pet + panel closed + stroll on', () => {
    expect(strollGate({ physicsCapable: true, settingsOpen: false, strollEnabled: true })).toEqual({
      runLoop: true,
      pushStrollMode: true,
    });
  });

  it('stays silent for a non-physics pet — teardown of a prior run already pushed false', () => {
    expect(strollGate({ physicsCapable: false, settingsOpen: false, strollEnabled: true })).toEqual(
      { runLoop: false, pushStrollMode: null },
    );
  });

  it('stays silent while the settings panel is open — the panel resize owns the window', () => {
    expect(strollGate({ physicsCapable: true, settingsOpen: true, strollEnabled: true })).toEqual({
      runLoop: false,
      pushStrollMode: null,
    });
  });

  it('pushes disabled when the user turned stroll off, so the tray checkbox matches at startup', () => {
    expect(strollGate({ physicsCapable: true, settingsOpen: false, strollEnabled: false })).toEqual(
      { runLoop: false, pushStrollMode: false },
    );
  });

  it('does not push disabled when stroll is off but the pet has no physics anyway', () => {
    expect(
      strollGate({ physicsCapable: false, settingsOpen: false, strollEnabled: false }),
    ).toEqual({ runLoop: false, pushStrollMode: null });
  });
});
