export const PROJECTION_STATUSES = Object.freeze([
  'idle',
  'working',
  'waiting',
  'compacting',
  'offline',
] as const);
export type ProjectionStatus = (typeof PROJECTION_STATUSES)[number];

export const VISIT_STATUSES = Object.freeze([
  'requested',
  'accepted',
  'traveling',
  'visiting',
  'returning',
  'completed',
  'declined',
  'cancelled',
  'expired',
  'recalled',
  'blocked',
] as const);
export type VisitStatus = (typeof VISIT_STATUSES)[number];

export const FRIEND_RELATIONS = Object.freeze(['pending_in', 'pending_out', 'accepted'] as const);
export type FriendRelation = (typeof FRIEND_RELATIONS)[number];

/**
 * v1 内置皮肤白名单的种子镜像（与 SQL 种子 private.approved_skins 同步）。
 * 权威来源是服务端数据表——白名单当数据管，不当类型管（契约 v0 评审决议 D2，
 * 见 PR #54）：wire 契约里 skinId 是 string；未批准的 skinId 不拒绝投影，
 * 服务端回落到 DEFAULT_PROJECTION_SKIN_ID。上新皮肤 = reviewed migration
 * 加行，不发契约版本。
 */
export const APPROVED_SKIN_IDS = Object.freeze([
  'shimeji-bola',
  'solu',
  'wukong',
  'yoonie',
] as const);
/** 未批准 skinId 的投影回落皮肤（原创默认宠）。 */
export const DEFAULT_PROJECTION_SKIN_ID = 'yoonie';
export type ApprovedSkinId = (typeof APPROVED_SKIN_IDS)[number];

export interface PublicPetProjection {
  readonly v: 1;
  readonly petId: string;
  readonly displayName: string;
  // string 而非枚举：白名单在服务端当数据校验 + 回落，见 APPROVED_SKIN_IDS 注释
  readonly skinId: string;
  readonly status: ProjectionStatus;
  readonly updatedAt: string;
}

export interface VisitLease {
  readonly id: string;
  readonly visitorUserId: string;
  readonly hostUserId: string;
  readonly status: VisitStatus;
  readonly startedAt: string | null;
  readonly endsAt: string | null;
}

export interface FriendEntry {
  readonly userId: string;
  readonly handle: string;
  readonly displayName: string | null;
  readonly relation: FriendRelation;
  readonly muted: boolean;
}
