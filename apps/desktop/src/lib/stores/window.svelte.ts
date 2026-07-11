import { invoke } from '@tauri-apps/api/core';

class WindowStore {
  expanded = $state(false);
  mascotHover = $state(false);
  moveMode = $state(false);
  pinned = $state(false);
  settingsOpen = $state(false);
  homeOpen = $state(false);

  async setExpanded(v: boolean, mascotScale: number = 1) {
    this.expanded = v;
    try {
      await invoke('set_mini_expanded', { expanded: v, mascotScale });
    } catch (e) {
      console.warn('[window] set_mini_expanded failed:', e);
    }
  }

  async moveBy(dx: number, dy: number) {
    try {
      await invoke('move_mini_by', { dx, dy });
    } catch {
      // ignore
    }
  }

  async setOrigin(x: number, y: number) {
    try {
      await invoke('set_mini_origin', { x, y });
    } catch {
      // ignore
    }
  }

  async getOrigin(): Promise<{ x: number; y: number } | null> {
    try {
      // get_mini_origin returns a Rust tuple → JSON array [x, y].
      const [x, y] = (await invoke('get_mini_origin')) as [number, number];
      return { x, y };
    } catch {
      return null;
    }
  }

  async getMonitorRect(): Promise<{ x: number; y: number; w: number; h: number } | null> {
    try {
      // get_mini_monitor_rect returns a Rust tuple → JSON array [x, y, w, h].
      const [x, y, w, h] = (await invoke('get_mini_monitor_rect')) as [
        number,
        number,
        number,
        number,
      ];
      return { x, y, w, h };
    } catch {
      return null;
    }
  }

  async openMini() {
    try {
      await invoke('open_mini');
    } catch {
      // ignore
    }
  }

  async closeMini() {
    try {
      await invoke('close_mini');
    } catch {
      // ignore
    }
  }

  async reassertFloating() {
    try {
      await invoke('reassert_floating');
    } catch {
      // ignore
    }
  }

  setMascotHover(v: boolean) {
    this.mascotHover = v;
  }

  setMoveMode(v: boolean) {
    this.moveMode = v;
  }

  setPinned(v: boolean) {
    this.pinned = v;
  }

  setSettingsOpen(v: boolean) {
    this.settingsOpen = v;
  }

  setHomeOpen(v: boolean) {
    this.homeOpen = v;
  }

  toggle() {
    this.setExpanded(!this.expanded);
  }
}

export const windowStore = new WindowStore();
