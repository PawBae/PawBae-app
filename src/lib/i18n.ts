import { register, init, getLocaleFromNavigator } from 'svelte-i18n';

register('en', () => import('./i18n/en.json'));
register('en-US', () => import('./i18n/en.json'));
register('en-GB', () => import('./i18n/en.json'));
register('zh', () => import('./i18n/zh.json'));
register('zh-CN', () => import('./i18n/zh.json'));
register('zh-TW', () => import('./i18n/zh.json'));
register('zh-HK', () => import('./i18n/zh.json'));

function resolveLocale(): string {
  const nav = getLocaleFromNavigator() ?? 'en';
  if (nav.startsWith('zh')) return nav;
  return nav;
}

init({
  fallbackLocale: 'en',
  initialLocale: resolveLocale(),
});
