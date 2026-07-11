import {
  authConfigured,
  logout as authLogout,
  loginWithGitHub,
  supabaseClient,
  toPlatformSession,
} from '../platform/auth';
import { platformMock } from '../platform/client';
import type { PlatformSession } from '../platform/types';

/**
 * 账号状态（B 线 W3）。AccountSection 的唯一数据源；会话变化同步进
 * PlatformClient（W7 前是 mock 的 setSession——真认证 + mock 串门共存，
 * 换真实现时删掉那一行即可）。
 *
 * 'unconfigured' = 构建时没有 Supabase 环境变量（见 platform/auth.ts）。
 * App 的一切功能不依赖登录 —— null 会话必须照常工作（契约红线）。
 */
export type AccountPhase = 'unconfigured' | 'signedOut' | 'signingIn' | 'signedIn';

class AccountStore {
  phase = $state<AccountPhase>(authConfigured ? 'signedOut' : 'unconfigured');
  session = $state<PlatformSession | null>(null);
  /** 最近一次登录失败的人类可读原因；进入新一轮登录时清空。 */
  error = $state('');

  private initialized = false;

  /** App 启动调用一次：恢复持久化会话 + 订阅后续变化。 */
  async init() {
    if (this.initialized || !authConfigured) return;
    this.initialized = true;
    const sb = supabaseClient();
    if (!sb) return;

    sb.auth.onAuthStateChange((_event, session) => {
      this.applySession(toPlatformSession(session));
    });
    try {
      const { data } = await sb.auth.getSession();
      this.applySession(toPlatformSession(data.session));
    } catch {
      // 离线等读取失败：保持登出态，登录入口仍可用
    }
  }

  private applySession(s: PlatformSession | null) {
    this.session = s;
    if (this.phase !== 'signingIn' || s) {
      this.phase = s ? 'signedIn' : 'signedOut';
    }
    // 让串门层（W7 前是 mock）看到同一份会话
    platformMock.setSession(s);
  }

  async login() {
    if (this.phase === 'signingIn' || this.phase === 'unconfigured') return;
    this.error = '';
    this.phase = 'signingIn';
    try {
      await loginWithGitHub();
      // applySession 由 onAuthStateChange 触发；这里兜底防事件竞态
      if (this.phase === 'signingIn' && this.session) this.phase = 'signedIn';
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.phase = 'signedOut';
    }
  }

  async logout() {
    try {
      await authLogout();
    } finally {
      this.applySession(null);
    }
  }
}

export const accountStore = new AccountStore();
