import {
  authConfigured,
  logout as authLogout,
  loginWithGitHub,
  supabaseClient,
  toCanonicalPlatformSession,
} from '../platform/auth';
import type { PlatformSession } from '../platform/types';

/**
 * 账号状态（B 线 W3）。AccountSection 的唯一数据源。
 * W7 起不再向 PlatformClient 桥接会话——真实客户端在 start() 里自己订阅
 * 同一个 GoTrue（platform/supabase-client.ts），两边天然同源。
 *
 * 'unconfigured' = 构建时没有 Supabase 环境变量（见 platform/auth.ts）。
 * App 的一切功能不依赖登录 —— null 会话必须照常工作（契约红线）。
 */
export type AccountPhase = 'initializing' | 'signedOut' | 'signingIn' | 'signedIn' | 'error';

class AccountStore {
  phase = $state<AccountPhase>(authConfigured ? 'initializing' : 'signedOut');
  readonly configured = authConfigured;
  session = $state<PlatformSession | null>(null);
  /** 最近一次登录失败的人类可读原因；进入新一轮登录时清空。 */
  error = $state('');

  private initialized = false;

  /** App 启动调用一次：恢复持久化会话 + 订阅后续变化。 */
  async init() {
    if (this.initialized) return;
    this.initialized = true;
    if (!authConfigured) {
      this.phase = 'signedOut';
      return;
    }
    const sb = supabaseClient();
    if (!sb) {
      this.phase = 'error';
      this.error = 'auth not configured';
      return;
    }

    sb.auth.onAuthStateChange((_event, session) => {
      void toCanonicalPlatformSession(sb, session).then((profile) => this.applySession(profile));
    });
    try {
      const { data } = await sb.auth.getSession();
      this.applySession(await toCanonicalPlatformSession(sb, data.session));
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.phase = 'error';
    }
  }

  private applySession(s: PlatformSession | null) {
    this.session = s;
    if (this.phase !== 'signingIn' || s) {
      this.phase = s ? 'signedIn' : 'signedOut';
    }
  }

  async login(): Promise<PlatformSession> {
    if (this.phase === 'signingIn') throw new Error('sign-in already in progress');
    if (!authConfigured) throw new Error('auth not configured');
    this.error = '';
    this.phase = 'signingIn';
    try {
      await loginWithGitHub();
      const sb = supabaseClient();
      if (!sb) throw new Error('auth not configured');
      const { data } = await sb.auth.getSession();
      const session = await toCanonicalPlatformSession(sb, data.session);
      if (!session) throw new Error('GitHub sign-in did not create a session');
      this.applySession(session);
      return session;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.phase = 'error';
      throw e;
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
