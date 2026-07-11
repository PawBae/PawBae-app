// 契约类型。数据类型（投影/访问/好友）自 @pawbae/shared re-export——契约
// 单一来源（A 线拥有，#54 落地）；PlatformClient/PlatformSession/Unsubscribe
// 是 B 线接口（docs/team/line-b-connector.md §2 冻结版），不进 shared。

export type {
  FriendEntry,
  ProjectionStatus,
  PublicPetProjection,
  VisitLease,
  VisitStatus,
} from '@pawbae/shared';

import type { FriendEntry, PublicPetProjection, VisitLease } from '@pawbae/shared';

export interface PlatformSession {
  userId: string;
  handle: string;
  displayName: string | null;
  avatarUrl: string | null;
}

export type Unsubscribe = () => void;

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
}
