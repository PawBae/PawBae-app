type RuntimeWindow = object | null | undefined;

export type DevPreview = 'onboarding' | 'home' | null;

export function isStageRuntime(
  runtimeWindow: RuntimeWindow,
  readWindowLabel: () => string,
): boolean {
  if (!runtimeWindow || !('__TAURI_INTERNALS__' in runtimeWindow)) return false;
  return readWindowLabel() === 'stage';
}

export function resolveDevPreview(isDev: boolean, search: string): DevPreview {
  if (!isDev) return null;
  const params = new URLSearchParams(search);
  if (params.has('home-preview')) return 'home';
  if (params.has('onboarding-preview')) return 'onboarding';
  return null;
}
