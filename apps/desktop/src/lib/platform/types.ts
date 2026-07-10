// 契约 v0 本地镜像。权威定义在两份交接文档里：
//   数据类型     → docs/team/line-a-cloud.md §2（A 线拥有，落地为 @pawbae/shared）
//   PlatformClient → docs/team/line-b-connector.md §2（B 线拥有，W3 冻结）
// @pawbae/shared 随 PR-B 落地后，本文件的数据类型改为 re-export；接口以 B 线
// 冻结版为准——任何差异一律以文档/冻结版为权威，不以本镜像为准。

export type ProjectionStatus = 'idle' | 'working' | 'waiting' | 'compacting' | 'offline';

export interface PublicPetProjection {
  v: 1; // schema 版本，向后兼容演进
  petId: string;
  displayName: string; // profiles.display_name ?? handle
  skinId: string; // 内置皮肤 id（白名单枚举）
  status: ProjectionStatus;
  updatedAt: string; // ISO 8601
}

export type VisitStatus =
  | 'requested'
  | 'accepted'
  | 'traveling'
  | 'visiting'
  | 'returning' // ← 未结束态
  | 'completed'
  | 'declined'
  | 'cancelled'
  | 'expired'
  | 'recalled'
  | 'blocked'; // ← 终止态

export interface VisitLease {
  id: string;
  visitorUserId: string; // 访客宠物的主人
  hostUserId: string;
  status: VisitStatus;
  startedAt: string | null; // accepted 之前为 null
  endsAt: string | null; // 固定 30 分钟租约
}

export interface FriendEntry {
  userId: string;
  handle: string;
  displayName: string | null;
  relation: 'pending_in' | 'pending_out' | 'accepted';
  muted: boolean;
}

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
