import type { PawBaeEvent } from '@pawbae/shared';
import { authConfigured, supabaseClient } from './auth';

/**
 * B 线数据出口的唯一通道（W4）：心跳 + opt-in 事件上传。
 *
 * 隐私红线（line-b §隐私）：上传载荷只能来自 @pawbae/shared 的字典构造器
 * （PawBaeEvent），本模块不接受裸对象——不存在「顺手多传一个字段」的代码
 * 路径。心跳只证明存活（RPC 无参数），不带会话内容/项目标识/工作计数。
 *
 * 三重门，任何一道关着都不出网：
 *   1. authConfigured && 已登录（未配置/未登录 = 静默 no-op）
 *   2. 「连接你的 agent」总闸（settings.platformConnectEnabled，默认关）
 *   3. 事件上传另有逐类分项开关（默认关）
 *
 * 门的状态由 Main.svelte 的 $effect 通过 configure() 喂进来——本模块不
 * import 任何 store（保持可单测、无 runes 依赖）。
 */

/** 心跳间隔。服务端限频 2 次/分钟（line-b 文档），60s 留一倍安全边际。 */
export const HEARTBEAT_INTERVAL_MS = 60_000;

/** 连胜里程碑（跨过才上传，不是每天发）。 */
export const STREAK_MILESTONES = Object.freeze([3, 7, 14, 30, 60, 100]);

/** 打卡后是否恰好跨过一个里程碑（宽容连胜可跳变，用区间判断）。 */
export function crossedStreakMilestone(prevStreak: number, nextStreak: number): number | null {
  if (nextStreak <= prevStreak) return null;
  for (const m of STREAK_MILESTONES) {
    if (prevStreak < m && nextStreak >= m) return m;
  }
  return null;
}

export type EventUploadSwitches = Readonly<{
  task_completed: boolean;
  egg_hatched: boolean;
  souvenir_found: boolean;
  streak_milestone: boolean;
}>;

type ConnectorGates = {
  signedIn: boolean;
  connectEnabled: boolean;
  uploads: EventUploadSwitches;
};

type RpcClient = {
  rpc(name: string): PromiseLike<{ error: unknown }>;
  from(table: string): { insert(row: Record<string, unknown>): PromiseLike<{ error: unknown }> };
};

const CLOSED: ConnectorGates = {
  signedIn: false,
  connectEnabled: false,
  uploads: {
    task_completed: false,
    egg_hatched: false,
    souvenir_found: false,
    streak_milestone: false,
  },
};

export function createConnector(
  clientFn: () => RpcClient | null,
  intervalMs = HEARTBEAT_INTERVAL_MS,
) {
  let gates: ConnectorGates = CLOSED;
  let timer: ReturnType<typeof setInterval> | null = null;

  function heartbeatOpen() {
    return gates.signedIn && gates.connectEnabled;
  }

  async function beat() {
    const client = clientFn();
    if (!client || !heartbeatOpen()) return;
    try {
      // 失败静默：下一个 60s tick 就是重试，不需要即时补发（限频 2/min）
      const { error } = await client.rpc('connector_heartbeat');
      if (error) console.warn('[connector] heartbeat rejected:', error);
    } catch (e) {
      console.warn('[connector] heartbeat failed:', e);
    }
  }

  /** Main 的 $effect 在门状态变化时调用；开闸沿立刻先跳一拍。 */
  function configure(next: ConnectorGates) {
    const wasOpen = heartbeatOpen();
    gates = next;
    const isOpen = heartbeatOpen();
    if (isOpen && !timer) {
      timer = setInterval(() => void beat(), intervalMs);
    } else if (!isOpen && timer) {
      clearInterval(timer);
      timer = null;
    }
    if (isOpen && !wasOpen) void beat();
  }

  /**
   * opt-in 事件上传。fire-and-forget：任何失败静默丢弃（事件是锦上添花的
   * 分享时刻，绝不能反过来影响本地体验），也绝不排队重放。
   */
  function uploadEvent(event: PawBaeEvent) {
    const client = clientFn();
    if (!client || !heartbeatOpen() || !gates.uploads[event.kind]) return;
    void (async () => {
      try {
        const { error } = await client.from('events').insert({
          kind: event.kind,
          params: event.params,
        });
        if (error) console.warn('[connector] event upload rejected:', error);
      } catch (e) {
        console.warn('[connector] event upload failed:', e);
      }
    })();
  }

  function stop() {
    configure(CLOSED);
  }

  return { configure, uploadEvent, stop, heartbeatOpen };
}

/** App 单例；测试用 createConnector 注入假 client，不 import 本导出。 */
export const connector = createConnector(() =>
  authConfigured ? (supabaseClient() as unknown as RpcClient) : null,
);
