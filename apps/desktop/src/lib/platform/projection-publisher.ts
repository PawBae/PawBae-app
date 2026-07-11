import { authConfigured, supabaseClient } from './auth';
import type { ProjectionStatus } from './types';

/**
 * 投影发布管道（B 线 W5）：agent-activity 聚合信号 → ProjectionStatus →
 * 去重 + 最小间隔上传 update_projection。
 *
 * 发送端守门（SV spec §6 / line-b W5-6）：**仅在存在活跃出访租约时发布**
 * ——门由 Main 的 $effect 喂（登录 + 连接总闸 + outbound 租约活跃）。门一
 * 关立即断流并清空待发/去重记忆：重开后的第一拍必然发出（好友端拿到的
 * 永远是新租约窗口里的新鲜状态，不吃上一次窗口的陈旧去重）。
 *
 * 节流设计：状态脉冲可能抖（tool_running 闪烁），去重挡住相同值；不同值
 * 走「立即发 + 间隔内尾发合并（last-write-wins）」——服务端限频 120/分钟，
 * 3s 最小间隔 = 20/分钟，留 6 倍边际。失败静默：投影是尽力而为的镜像，
 * 下一次状态变化就是重试。
 */

export const MIN_PUBLISH_INTERVAL_MS = 3_000;

export type ProjectionInput = Readonly<{
  petId: string;
  skinId: string;
  status: ProjectionStatus;
}>;

type RpcClient = {
  rpc(name: string, params?: Record<string, unknown>): PromiseLike<{ error: unknown }>;
};

function keyOf(i: ProjectionInput): string {
  return `${i.petId}\n${i.skinId}\n${i.status}`;
}

export function createProjectionPublisher(
  clientFn: () => RpcClient | null,
  minIntervalMs = MIN_PUBLISH_INTERVAL_MS,
) {
  let enabled = false;
  let lastSentKey = '';
  let lastSentAt = Number.NEGATIVE_INFINITY;
  let pending: ProjectionInput | null = null;
  let trailingTimer: ReturnType<typeof setTimeout> | null = null;

  function clearTrailing() {
    if (trailingTimer) {
      clearTimeout(trailingTimer);
      trailingTimer = null;
    }
  }

  async function send(input: ProjectionInput) {
    lastSentKey = keyOf(input);
    lastSentAt = Date.now();
    const client = clientFn();
    if (!client) return;
    try {
      const { error } = await client.rpc('update_projection', {
        p_pet_id: input.petId,
        p_skin_id: input.skinId,
        p_status: input.status,
      });
      // 未批准皮肤由服务端回落默认皮（D2），这里报错的是形状/权限类问题
      if (error) console.warn('[projection] update rejected:', error);
    } catch (e) {
      console.warn('[projection] update failed:', e);
    }
  }

  function flush() {
    clearTrailing();
    if (!enabled || pending === null) return;
    const next = pending;
    pending = null;
    void send(next);
  }

  function publish(input: ProjectionInput) {
    if (!enabled) return;
    if (keyOf(input) === lastSentKey) {
      // 状态弹回已发送值：撤销还没发出去的中间态（working→waiting→working）
      pending = null;
      clearTrailing();
      return;
    }
    pending = input; // last-write-wins：间隔内只留最新
    const wait = lastSentAt + minIntervalMs - Date.now();
    if (wait <= 0) {
      flush();
    } else if (!trailingTimer) {
      trailingTimer = setTimeout(flush, wait);
    }
  }

  /** 门（登录 && 总闸 && 出访租约活跃）。关门即断流；重开后首拍必发。 */
  function setEnabled(next: boolean) {
    if (enabled === next) return;
    enabled = next;
    if (!next) {
      clearTrailing();
      pending = null;
      lastSentKey = '';
      lastSentAt = Number.NEGATIVE_INFINITY;
    }
  }

  return { publish, setEnabled };
}

/** App 单例；测试用 createProjectionPublisher 注入假 client。 */
export const projectionPublisher = createProjectionPublisher(() =>
  authConfigured ? (supabaseClient() as unknown as RpcClient) : null,
);
