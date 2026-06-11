import { invoke } from '@tauri-apps/api/core';

/**
 * Best-effort `invoke` for fire-and-forget backend calls (tracking toggles, tray
 * updates, sprite-pad pushes): the caller must never break on failure, but the
 * failure has to leave a trace — a silent `.catch(() => {})` turns a denied macOS
 * permission or a renamed command into "the pet just doesn't react".
 */
export function tryInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | undefined> {
  return invoke<T>(cmd, args).catch((e) => {
    console.warn(`[tauri] ${cmd} failed:`, e);
    return undefined;
  });
}
