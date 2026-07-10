import { createClient, type Session, type SupabaseClient } from '@supabase/supabase-js';
import { invoke } from '@tauri-apps/api/core';
import type { PlatformSession } from './types';

/**
 * GitHub OAuth 桌面流（B 线 W3）—— supabase-js PKCE + loopback 回调。
 *
 * 流程：先 invoke `await_oauth_callback` 让 Rust 绑定 127.0.0.1:53682，
 * 再用系统浏览器打开授权 URL；Supabase 跳回 loopback 后拿到授权码，
 * `exchangeCodeForSession` 换会话（code_verifier 只存在于本 webview 的
 * localStorage，授权码被截获也无法使用）。会话由 supabase-js 持久化，
 * 重启免登录。
 *
 * 配置经 Vite 环境变量注入（publishable key 公开设计，RLS 才是边界）：
 * 缺失时 `authConfigured` 为 false，账号区显示「云端配置未就绪」——
 * 诚实降级，绝不 no-op 假装能登录。
 */

const SUPABASE_URL = import.meta.env.VITE_SUPABASE_URL as string | undefined;
const SUPABASE_KEY = import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY as string | undefined;

/** 与 Rust CALLBACK_PATH、Supabase 后台 Redirect URLs 白名单三处一致。 */
export const OAUTH_CALLBACK_PORT = 53682;
export const OAUTH_CALLBACK_URL = `http://127.0.0.1:${OAUTH_CALLBACK_PORT}/pawbae-auth`;

export const authConfigured = Boolean(SUPABASE_URL && SUPABASE_KEY);

let client: SupabaseClient | null = null;

export function supabaseClient(): SupabaseClient | null {
  if (!authConfigured) return null;
  if (!client) {
    client = createClient(SUPABASE_URL as string, SUPABASE_KEY as string, {
      auth: {
        flowType: 'pkce',
        persistSession: true,
        autoRefreshToken: true,
        // 桌面 App 没有「当前 URL 带 token」的场景，回调走 loopback 显式换票
        detectSessionInUrl: false,
      },
    });
  }
  return client;
}

/** supabase 会话 → 契约 PlatformSession。handle 取 GitHub login，兜底邮箱前缀。 */
export function toPlatformSession(session: Session | null): PlatformSession | null {
  if (!session?.user) return null;
  const u = session.user;
  const meta = (u.user_metadata ?? {}) as Record<string, unknown>;
  const handle =
    (typeof meta.user_name === 'string' && meta.user_name) ||
    (typeof meta.preferred_username === 'string' && meta.preferred_username) ||
    u.email?.split('@')[0] ||
    u.id.slice(0, 8);
  return {
    userId: u.id,
    handle,
    displayName: typeof meta.full_name === 'string' && meta.full_name ? meta.full_name : null,
    avatarUrl: typeof meta.avatar_url === 'string' && meta.avatar_url ? meta.avatar_url : null,
  };
}

/** 从回调查询串提取授权码；Supabase 报错时抛出人类可读信息。 */
export function extractAuthCode(query: string): string {
  const params = new URLSearchParams(query);
  const err = params.get('error_description') || params.get('error');
  if (err) throw new Error(err);
  const code = params.get('code');
  if (!code) throw new Error('OAuth callback carried no code');
  return code;
}

/**
 * 整套 GitHub 登录流。resolve 即已登录（onAuthStateChange 会随之触发）。
 * 任何一步失败都抛出 —— 调用方负责把错误落到 UI，不静默。
 */
export async function loginWithGitHub(): Promise<void> {
  const sb = supabaseClient();
  if (!sb) throw new Error('auth not configured');

  // 先占住回调端口（bind 在命令入口同步发生），再打开浏览器
  const callback = invoke<string>('await_oauth_callback', {
    port: OAUTH_CALLBACK_PORT,
    timeoutSecs: 180,
  });
  const { data, error } = await sb.auth.signInWithOAuth({
    provider: 'github',
    options: { redirectTo: OAUTH_CALLBACK_URL, skipBrowserRedirect: true },
  });
  if (error || !data?.url) throw error ?? new Error('no authorize url');
  await invoke('open_url', { url: data.url });

  const query = await callback;
  const code = extractAuthCode(query);
  const { error: exchangeError } = await sb.auth.exchangeCodeForSession(code);
  if (exchangeError) throw exchangeError;
}

export async function logout(): Promise<void> {
  const sb = supabaseClient();
  if (!sb) return;
  await sb.auth.signOut();
}
