// PlatformClient 注入点——line-c §3 说的「一行 DI 切换」就是这一行：
// W5-6 串门 UI 全部对着 MockPlatformClient 开发；W7 B 线真实实现落地后，
// 把下面换成 `new SupabasePlatformClient(...)`，其余代码零改动。

import { MockPlatformClient } from './mock';
import type { PlatformClient } from './types';

const mock = new MockPlatformClient({ startAtMs: Date.now() });

// Mock 的虚拟时钟在 App 运行时由真实时间驱动（测试里仍手动 advance()，
// 测试不要 import 本文件）。换真实实现时连同这段一起删。
let last = Date.now();
setInterval(() => {
  const now = Date.now();
  mock.advance(now - last);
  last = now;
}, 250);

export const platformClient: PlatformClient = mock;

/** 仅开发期暴露：给调试面板注入剧本（真实实现没有这些方法）。 */
export const platformMock: MockPlatformClient = mock;
