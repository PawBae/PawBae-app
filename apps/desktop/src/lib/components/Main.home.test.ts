import { describe, expect, it } from 'vitest';
import windowSource from '../stores/window.svelte.ts?raw';
import mainSource from './Main.svelte?raw';
import panelSource from './Panel.svelte?raw';

function between(source: string, start: string, end: string): string {
  const startAt = source.indexOf(start);
  const endAt = source.indexOf(end, startAt + start.length);
  expect(startAt).toBeGreaterThanOrEqual(0);
  expect(endAt).toBeGreaterThan(startAt);
  return source.slice(startAt, endAt);
}

describe('Social Home window flow', () => {
  it('opens Home after onboarding without restoring mini immediately', () => {
    const completion = between(
      mainSource,
      'async function handleOnboardingComplete',
      '$effect(() =>',
    );

    expect(completion).toContain('setHomeOpen(true)');
    expect(completion).toContain('setSettingsOpen(false)');
    expect(completion).toContain('homeTheme = loadHomeTheme()');
    expect(completion).not.toContain('restore: true');
  });

  it('restores mini only from the desktop transition', () => {
    const transition = between(mainSource, 'async function sendPetToDesktop', '</script>');

    expect(transition).toContain('setHomeOpen(false)');
    expect(transition).toMatch(/set_mini_size[\s\S]*restore: true/);
  });

  it('expands Home from the mini panel and keeps Home and Settings exclusive', () => {
    const openHome = between(mainSource, 'async function openHome', 'async function');
    const panelOpenHome = between(panelSource, 'async function openHome', '</script>');

    expect(openHome).toContain('setExpanded(false');
    expect(openHome).toContain('setSettingsOpen(false)');
    expect(openHome).toContain('setHomeOpen(true)');
    expect(openHome).toMatch(/set_mini_size[\s\S]*restore: false[\s\S]*keepOnTop: false/);
    expect(panelOpenHome).toContain('onOpenHome');
    expect(panelSource).toContain('data-action="open-home"');
    expect(windowSource).toContain('homeOpen = $state(false)');
  });

  it('routes tray Settings from Home through the return-to-Home path without resizing', () => {
    const trayListener = between(
      mainSource,
      "addListener('tray-open-settings'",
      "addListener('stage-closed'",
    );
    const openSettings = between(mainSource, 'async function openSettings', 'async function');
    const closeSettings = between(mainSource, 'async function closeSettings', '</script>');

    expect(trayListener).toContain('openSettings()');
    expect(openSettings).toContain('const openedFromHome = windowStore.homeOpen');
    expect(openSettings).toContain('returnToHomeAfterSettings = openedFromHome');
    expect(openSettings).toContain('setHomeOpen(false)');
    expect(openSettings).toContain('setSettingsOpen(true)');
    expect(openSettings).toMatch(/if \(openedFromHome\) return;[\s\S]*set_mini_size/);
    expect(closeSettings).toContain('if (returnToHomeAfterSettings)');
    expect(closeSettings).toContain('setHomeOpen(true)');
  });

  it('keeps tray Settings from mini on the existing resize and restore path', () => {
    const openSettings = between(mainSource, 'async function openSettings', 'async function');
    const closeSettings = between(mainSource, 'async function closeSettings', '</script>');

    expect(openSettings).toMatch(
      /returnToHomeAfterSettings = openedFromHome[\s\S]*if \(openedFromHome\) return;[\s\S]*set_mini_size[\s\S]*restore: false/,
    );
    expect(closeSettings).toMatch(/else[\s\S]*set_mini_size[\s\S]*restore: true/);
  });

  it('feeds the model from real stores and hides the mini UI while Home is open', () => {
    expect(mainSource).toContain('aggregateSessions(sessionStore.claudeSessions)');
    expect(mainSource).toContain('deriveLocalAgentState(');
    // W7 换线：社交字段从真实租约流/好友契约灌入（#65 的 local-only 占位已升级）
    expect(mainSource).toContain('friends: friendSummaries(friendEntries)');
    expect(mainSource).toContain('derivePresence(');
    expect(mainSource).toContain('deriveVisitRequest(');
    expect(mainSource).toMatch(
      /pendingVisit: pendingVisit && pendingVisit\.id !== dismissedVisitId/,
    );
    // P4-C 共同记忆未上线：记忆字段保持诚实为空
    expect(mainSource).toContain('latestMemory: null');
    expect(mainSource).toContain('memories: []');
    expect(mainSource).toContain('{#if !windowStore.homeOpen}');
    expect(mainSource).toContain('<SocialHome');
    expect(mainSource).toContain('onSendToDesktop={sendPetToDesktop}');
    expect(mainSource).toContain('onOpenSettings={openSettings}');
    expect(mainSource).toContain('onVisitFriend={visitFriend}');
    expect(mainSource).toContain('onAcceptVisit={acceptVisit}');
    expect(mainSource).toContain('onDelayVisit={delayVisit}');
  });

  it('wires the real platform client at mount', () => {
    expect(mainSource).toContain('void platformClient.start()');
    expect(mainSource).toContain('visitStore.init(platformClient)');
    expect(mainSource).toContain('visitStore.startClock()');
    // 好友刷新键在平台会话镜像上（不是 accountStore——两边并行恢复会有空窗）；
    // 登出清空 visitStore，租约不跨账号残留
    expect(mainSource).toContain('platformClient.onSessionChange((s) => {');
    expect(mainSource).toContain('platformSession = s;');
    expect(mainSource).toContain('if (s === null) visitStore.reset()');
    expect(mainSource).toMatch(/if \(platformSession === null\) \{\s*friendEntries = \[\];/);
  });

  it('shares and persists the onboarding theme choice', () => {
    expect(mainSource).toContain('ONBOARDING_THEME_STORAGE_KEY');
    expect(mainSource).toContain('normalizeOnboardingTheme');
    expect(mainSource).toContain('localStorage.setItem(ONBOARDING_THEME_STORAGE_KEY, nextTheme)');
  });
});
