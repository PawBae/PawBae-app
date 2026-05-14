import { invoke } from '@tauri-apps/api/core';

class WindowStore {
  expanded = $state(false);
  mascotHover = $state(false);
  moveMode = $state(false);
  strollActive = $state(false);
  pinned = $state(false);
  settingsOpen = $state(false);

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
      return (await invoke('get_mini_origin')) as { x: number; y: number };
    } catch {
      return null;
    }
  }

  async getMonitorRect(): Promise<{ x: number; y: number; w: number; h: number } | null> {
    try {
      return (await invoke('get_mini_monitor_rect')) as {
        x: number;
        y: number;
        w: number;
        h: number;
      };
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

  setStrollActive(v: boolean) {
    this.strollActive = v;
  }

  setPinned(v: boolean) {
    this.pinned = v;
  }

  setSettingsOpen(v: boolean) {
    this.settingsOpen = v;
  }

  toggle() {
    this.setExpanded(!this.expanded);
  }
}

export const windowStore = new WindowStore();
