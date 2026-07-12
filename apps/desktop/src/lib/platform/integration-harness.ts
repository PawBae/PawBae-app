// 联调共用底座：W6 全链路套件与 W9 恢复矩阵套件对 `supabase start` 本地
// 真实栈的公共零件。不是测试文件——两份 *.integration.test.ts 从这里取件。
//
// 时间快进（W9 专用）的原则：只拨 visits 的时钟列、绝不手改 status——
// 状态迁移永远交给 private.maintain_visits()，与生产 pg_cron 同一条代码路径。
// 拨时钟需要跳过 visits_validate_row 的“时间戳不可变”守卫，用
// `SET LOCAL session_replication_role = replica` 只对本事务关触发器。

import { execFileSync } from 'node:child_process';
import { createClient, type SupabaseClient } from '@supabase/supabase-js';
import { type SupabaseLike, SupabasePlatformClient } from './supabase-client';
import type { VisitLease } from './types';

// 桌面 tsconfig 面向 DOM，没有（也不该为联调基建引入）@types/node——
// 本模块只在 vitest 的 node 环境跑，就地声明 process.env 的最小形状；
// 随机数统一走 Web Crypto 全局（DOM lib 有类型，Node ≥19 运行时也有）。
declare const process: { env: Record<string, string | undefined> };

export const SUPABASE_URL = process.env.PAWBAE_SUPABASE_URL ?? '';
export const PUBLISHABLE_KEY = process.env.PAWBAE_SUPABASE_PUBLISHABLE_KEY ?? '';
/** 本地栈 db 容器名（W9 时间快进用），由 scripts/w9-recovery-matrix.sh 注入。 */
export const DB_CONTAINER = process.env.PAWBAE_SUPABASE_DB_CONTAINER ?? '';

export function integrationEnv(name: string): string {
  return process.env[name] ?? '';
}

/** 联调把 15s 生产轮询压到 2s：跨端感知断言不用等一刻钟。 */
export const POLL_MS = 2_000;

export async function until<T>(
  probe: () => T | undefined | null | false,
  what: string,
  timeoutMs = 15_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = probe();
    if (value) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
}

export interface Actor {
  raw: SupabaseClient;
  platform: SupabasePlatformClient;
  userId: string;
  /** onLeaseChange 全量流水：三源合流的观测窗口。restartActor 会换成新数组。 */
  leases: VisitLease[];
}

export async function signUpActor(label: string): Promise<Actor> {
  const raw = createClient(SUPABASE_URL, PUBLISHABLE_KEY, {
    auth: { persistSession: false, autoRefreshToken: false },
  });
  const suffix = crypto.randomUUID().replaceAll('-', '').slice(0, 10);
  const { data, error } = await raw.auth.signUp({
    email: `itest-${label}-${suffix}@example.test`,
    password: `T3st-${crypto.randomUUID()}`,
  });
  if (error || !data.user) throw new Error(`signUp(${label}) failed: ${error?.message}`);
  const platform = new SupabasePlatformClient(() => raw as unknown as SupabaseLike, POLL_MS);
  await platform.start();
  const actor: Actor = { raw, platform, userId: data.user.id, leases: [] };
  // 动态取 actor.leases：restartActor 换数组后回调自动写进新流水
  platform.onLeaseChange((lease) => actor.leases.push(lease));
  return actor;
}

/**
 * 模拟“应用关闭后再启动”：丢弃全部客户端内存状态（租约缓存、轮询、频道），
 * 只保留 raw 里的会话——重启后的一切认知必须从服务端真相重建。
 */
export async function restartActor(actor: Actor): Promise<void> {
  actor.platform.dispose();
  await actor.raw.removeAllChannels();
  actor.platform = new SupabasePlatformClient(() => actor.raw as unknown as SupabaseLike, POLL_MS);
  actor.leases = [];
  await actor.platform.start();
  actor.platform.onLeaseChange((lease) => actor.leases.push(lease));
}

export async function disposeActor(actor: Actor | undefined): Promise<void> {
  if (!actor) return;
  actor.platform.dispose();
  await actor.raw.removeAllChannels();
}

export async function rawRpc(
  actor: Actor,
  name: string,
  params: Record<string, unknown>,
): Promise<void> {
  const { error } = await actor.raw.rpc(name, params);
  if (error) throw new Error(`${name} failed: ${error.message}`);
}

/**
 * 等 Realtime 频道真正 join 完成再触发广播源：Broadcast 不回放，join 前发出的帧
 * 会永久丢失（生产里由轮询兜底，测试里则要确定性）。走 supabase-js 公开 API
 * 观察频道状态，不需要生产接口暴露 join 时机。
 */
export async function untilJoined(actor: Actor, lease: VisitLease): Promise<void> {
  const suffix = `pet:${lease.visitorUserId}:${lease.id}`;
  await until(
    () => actor.raw.getChannels().some((c) => c.topic.endsWith(suffix) && c.state === 'joined'),
    `Realtime 频道 ${suffix} join 完成`,
  );
}

/** 以 postgres 身份对本地栈执行 SQL，返回 -tAc 的裸输出（去首尾空白）。 */
export function sql(query: string): string {
  if (DB_CONTAINER === '') {
    throw new Error('PAWBAE_SUPABASE_DB_CONTAINER 未注入——用 scripts/w9-recovery-matrix.sh 运行');
  }
  return execFileSync(
    'docker',
    [
      'exec',
      DB_CONTAINER,
      'psql',
      '-U',
      'postgres',
      '-d',
      'postgres',
      '-v',
      'ON_ERROR_STOP=1',
      '-tAc',
      query,
    ],
    { encoding: 'utf8' },
  ).trim();
}

/** 手动执行生产 cron 的同一份维护函数：到期/过场迁移即刻发生，不等每分钟节拍。 */
export function maintainVisits(): void {
  sql('SELECT private.maintain_visits();');
}

export type VisitClockColumn =
  | 'requested_at'
  | 'request_expires_at'
  | 'started_at'
  | 'ends_at'
  | 'returning_started_at';

/**
 * 把某访问的一组时钟列整体拨回 seconds 秒——语义是“这次访问发生在更早时刻”。
 * 必须成对平移：validate_visit_row 有跨列不变量（租约恰 30 分钟、请求窗口恰
 * 24 小时），只拨单列会让下一次合法 UPDATE（maintainVisits）在触发器里炸掉，
 * 而且一行坏数据会拖垮整个 maintain_visits 连带生产 cron。
 * 只改时钟不改状态：状态迁移仍走 maintainVisits()。
 */
export function ageVisitClocks(
  visitId: string,
  seconds: number,
  columns: readonly VisitClockColumn[],
): void {
  const sets = columns
    .map((column) => `${column} = ${column} - interval '${seconds} seconds'`)
    .join(', ');
  sql(
    `BEGIN;
SET LOCAL session_replication_role = replica;
UPDATE public.visits
SET ${sets}
WHERE id = '${visitId}';
COMMIT;`,
  );
}

/** visits 行的当前状态：竞态断言的服务端单一真相。 */
export function visitRowStatus(visitId: string): string {
  return sql(`SELECT status FROM public.visits WHERE id = '${visitId}';`);
}
