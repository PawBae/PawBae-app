// 契约类型。数据类型（投影/访问/好友）自 @pawbae/shared re-export——契约
// 单一来源（A 线拥有，#54 落地）；PlatformClient/PlatformSession/Unsubscribe
// 是 B 线接口（docs/team/line-b-connector.md §2 冻结版），不进 shared。

export type {
  FriendEntry,
  MemoryTemplateKey,
  MemoryTemplateParams,
  ProjectionStatus,
  PublicPetProjection,
  VisitLease,
  VisitStatus,
} from '@pawbae/shared';

import type {
  FriendEntry,
  MemoryTemplateKey,
  MemoryTemplateParams,
  PublicPetProjection,
  VisitLease,
} from '@pawbae/shared';

export interface PlatformSession {
  userId: string;
  handle: string;
  displayName: string | null;
  avatarUrl: string | null;
}

export type Unsubscribe = () => void;

/** shared_memories 行的客户端形状：模板键 + 安全参数，不含预渲染文本（A 线 W8 契约）。 */
export interface SharedMemoryEntry {
  id: string;
  visitId: string;
  visitorUserId: string;
  hostUserId: string;
  templateKey: MemoryTemplateKey;
  params: MemoryTemplateParams;
  createdAt: string;
}

export interface PlatformClient {
  // 会话
  session(): PlatformSession | null; // null = 未登录（App 必须照常工作）
  onSessionChange(cb: (s: PlatformSession | null) => void): Unsubscribe;

  // 串门——覆盖 A 线 RPC 清单的全部六个访问动作
  requestVisit(hostUserId: string, idempotencyKey: string): Promise<VisitLease>;
  respondVisit(leaseId: string, action: 'accept' | 'decline', key: string): Promise<VisitLease>;
  cancelVisit(leaseId: string, key: string): Promise<VisitLease>; // 访客撤回仍处 requested 的邀请
  recallVisit(leaseId: string, key: string): Promise<VisitLease>;
  endVisit(leaseId: string, key: string): Promise<VisitLease>; // 任一方提前结束访问
  onLeaseChange(cb: (lease: VisitLease) => void): Unsubscribe;
  subscribeGuestProjection(lease: VisitLease, cb: (p: PublicPetProjection) => void): Unsubscribe;

  // 邀请码（onboarding 消费）
  redeemInvite(code: string, key: string): Promise<void>;

  // 好友
  friends(): Promise<FriendEntry[]>;

  // 共同记忆（P4-C 数据面，W9 增补）——结算幂等：服务端按 visit_id 唯一，
  // 双端重复调用都拿回同一行；requested 系终局（declined/cancelled/expired/blocked）不可结算
  settleMemory(visitId: string, key: string): Promise<SharedMemoryEntry>;
  sharedMemories(): Promise<SharedMemoryEntry[]>;
  recordMemoryView(memoryId: string, key: string): Promise<void>; // SV §9 漏斗 memory_viewed
}
