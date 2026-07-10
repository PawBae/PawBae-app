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

/** Built-in skins approved for the v1 public projection contract. */
export const APPROVED_SKIN_IDS = Object.freeze([
  'shimeji-bola',
  'yoonie',
  'doro.codex-pet',
  'elaina-2',
  'homie',
  'linnea-2',
  'mambo',
  'naruto',
  'nezuko',
  'phoebe.codex-pet',
  'skirk-2',
  'taffy',
  'wukong',
] as const);
export type ApprovedSkinId = (typeof APPROVED_SKIN_IDS)[number];

export interface PublicPetProjection {
  readonly v: 1;
  readonly petId: string;
  readonly displayName: string;
  readonly skinId: ApprovedSkinId;
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
