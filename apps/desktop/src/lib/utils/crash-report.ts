import { invoke } from '@tauri-apps/api/core';

/**
 * webview 全局错误上报（最小自建崩溃上报的前端半边）。
 * window error / unhandledrejection → Rust `report_frontend_error` → 本地脱敏
 * 崩溃日志（src-tauri/src/commands/crash.rs）。不接外部服务。
 *
 * 去重 + 限量：同一条消息只报一次，单次运行最多 MAX_REPORTS 条 —— 防止
 * 渲染循环里的错误风暴打爆 IPC（Rust 侧另有兜底上限）。
 */

const MAX_REPORTS = 30;
const MESSAGE_LIMIT = 2000;
const STACK_LIMIT = 16000;

type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

export function createErrorReporter(invokeFn: InvokeFn = invoke) {
  const seen = new Set<string>();
  let sent = 0;

  function report(kind: 'error' | 'rejection', message: string, stack?: string) {
    if (sent >= MAX_REPORTS) return;
    const key = `${kind}:${message}`;
    if (seen.has(key)) return;
    seen.add(key);
    sent += 1;
    void invokeFn('report_frontend_error', {
      kind,
      message: message.slice(0, MESSAGE_LIMIT),
      stack: stack ? stack.slice(0, STACK_LIMIT) : null,
    }).catch(() => {});
  }

  function install(target: Window = window) {
    target.addEventListener('error', (e) => {
      const err = e.error;
      report(
        'error',
        e.message || (err instanceof Error ? err.message : 'unknown error'),
        err instanceof Error ? err.stack : undefined,
      );
    });
    target.addEventListener('unhandledrejection', (e) => {
      const reason: unknown = e.reason;
      report(
        'rejection',
        reason instanceof Error ? reason.message : String(reason),
        reason instanceof Error ? reason.stack : undefined,
      );
    });
  }

  return { report, install };
}

/** 应用入口调用一次（main.ts，两个窗口共用入口所以 stage 窗口也覆盖）。 */
export function installGlobalErrorReporting() {
  createErrorReporter().install();
}
