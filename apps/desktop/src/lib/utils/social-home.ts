import {
  MEMORY_PARAMETER_LOCALIZATIONS,
  MEMORY_TEMPLATE_LOCALIZATIONS,
  type MemoryLocale,
  type MemoryTemplateKey,
  type MemoryTemplateParams,
} from '@pawbae/shared';
import type { LocalVisitPhase } from '../platform/lease-machine';
import type {
  FriendEntry,
  PublicPetProjection,
  SharedMemoryEntry,
  VisitLease,
} from '../platform/types';
import { type AgentActivity, mascotStateFor } from './agent-activity';
import type { CodexPet } from './codex-pet';
import type { OfficialPetId } from './onboarding';

export type PublicAgentState = 'idle' | 'working' | 'waiting' | 'compacting' | 'offline';
export type RealtimeState = 'connected' | 'degraded' | 'reconnecting';
export type HomePanel = 'friends' | 'plaza' | 'album' | null;
export type HomeAction =
  | 'feed'
  | 'gift'
  | 'diary'
  | 'send-to-desktop'
  | 'play'
  | 'snack'
  | 'photo'
  | 'end-visit'
  | 'view-visit'
  | 'recall';

export interface HomePetIdentity {
  id: string;
  name: string;
  officialPetId?: OfficialPetId;
  ownerName?: string;
}

export type HomePresence =
  | { kind: 'home'; visitor: null }
  | {
      kind: 'home';
      visitor: HomePetIdentity;
      visitorOwnerName: string;
      visitorAgentState: PublicAgentState;
      endsAt: string;
      leaseMinutes: 30;
    }
  | { kind: 'away'; friendId: string; friendName: string; endsAt: string; leaseMinutes: 30 };

export interface FriendSummary {
  id: string;
  displayName: string;
  handle: string;
  pet: HomePetIdentity;
  availability: 'available' | 'visiting' | 'away' | 'offline';
  publicAgentState: PublicAgentState;
  visitDirection: 'visit-them' | 'invite-over';
}

export interface VisitRequest {
  id: string;
  friendId: string;
  ownerName: string;
  pet: HomePetIdentity;
}

/** 模板键与安全参数 = @pawbae/shared memories.ts 契约（A 线 W8 落地的单一来源）。 */
export type SharedMemoryTemplateKey = MemoryTemplateKey;

export interface SharedMemorySummary {
  id: string;
  occurredAt: number;
  /**
   * 参与者的展示名（v1 契约缺口同好友墙：平台尚无好友宠物身份，用主人名/
   * 本地宠物名占位），顺序恒为 [访客侧, 主人侧]。
   */
  petIds: string[];
  templateKey: MemoryTemplateKey;
  params: MemoryTemplateParams;
}

export interface SocialHomeModel {
  localPet: HomePetIdentity;
  presence: HomePresence;
  agentState: PublicAgentState;
  realtimeState: RealtimeState;
  affection: number;
  coins: number;
  togetherDays: number;
  growthCurrent: number;
  growthTarget: number;
  friends: FriendSummary[];
  pendingVisit: VisitRequest | null;
  latestMemory: SharedMemorySummary | null;
  memories: SharedMemorySummary[];
}

export type HomeEvent =
  | { kind: 'visit-request'; request: VisitRequest }
  | { kind: 'memory-ready'; memory: SharedMemorySummary }
  | { kind: 'invite-friend' };

export type FriendActionDisabledReason =
  | 'hosting'
  | 'away-elsewhere'
  | 'friend-offline'
  | 'friend-busy'
  | 'already-requested';

export type FriendContextAction =
  | { kind: 'visit'; disabledReason: FriendActionDisabledReason | null }
  | { kind: 'invite'; disabledReason: FriendActionDisabledReason | null }
  | { kind: 'recall'; disabledReason: FriendActionDisabledReason | null };

const OFFICIAL_IDS = new Set<OfficialPetId>(['solu', 'muru', 'riffi', 'luma']);

export function isOfficialPetId(value: string): value is OfficialPetId {
  return OFFICIAL_IDS.has(value as OfficialPetId);
}

export function deriveHomePetIdentity(
  selectedPetId: string,
  currentPet: CodexPet | null,
  currentPetName: string | null,
  officialName: string,
): HomePetIdentity {
  if (isOfficialPetId(selectedPetId) && currentPet?.id !== selectedPetId) {
    return { id: selectedPetId, name: officialName, officialPetId: selectedPetId };
  }
  if (currentPet) {
    return { id: currentPet.id, name: currentPetName ?? currentPet.displayName };
  }
  return { id: selectedPetId, name: 'PawBae' };
}

export function deriveLocalAgentState(
  enabled: boolean,
  activity: AgentActivity,
  anyHealthActive: boolean,
): PublicAgentState {
  if (!enabled) return 'offline';
  return mascotStateFor(activity, anyHealthActive);
}

// ---------- 平台契约 → Home 模型（B 线 W7 换线）----------

/** 好友展示名：display_name（自选昵称）优先，回落匿名代号 handle。 */
export function friendDisplayName(entry: FriendEntry): string {
  return entry.displayName ?? entry.handle;
}

/**
 * FriendEntry（§2 冻结契约）→ FriendSummary（Home UI 模型）。
 * v1 契约不含好友宠物身份与实时在线状态：宠物名用主人名占位（串门被接受后
 * 真实身份才经投影到达），availability/agent 状态给 available/idle——
 * 给 offline 会被 selectFriendAction 判成 friend-offline 灰掉串门入口，
 * 而发起请求必须始终可用（服务端仲裁 + 24h 过期兜底）；A 线补 presence 后再收紧。
 * 只上墙 accepted：pending 关系 v1 不进好友列表。
 */
export function friendSummaries(entries: readonly FriendEntry[]): FriendSummary[] {
  return entries
    .filter((entry) => entry.relation === 'accepted')
    .map((entry) => {
      const name = friendDisplayName(entry);
      return {
        id: entry.userId,
        displayName: name,
        handle: entry.handle,
        pet: { id: entry.userId, name },
        availability: 'available' as const,
        publicAgentState: 'idle' as const,
        visitDirection: 'visit-them' as const,
      };
    });
}

/**
 * 双槽租约 → Home 在场态。访客在场优先于自家外出（类型单值：家里有客先展示客，
 * 外出另有空窝牌/召回入口表达）。访客身份只信投影——接受前后端不暴露对方宠物。
 */
export function derivePresence(
  outbound: VisitLease | null,
  outboundPhase: LocalVisitPhase,
  inbound: VisitLease | null,
  inboundPhase: LocalVisitPhase,
  guestProjection: PublicPetProjection | null,
  nameOf: (userId: string) => string | null,
): HomePresence {
  if (inbound && inboundPhase === 'visiting' && guestProjection) {
    return {
      kind: 'home',
      visitor: { id: guestProjection.petId, name: guestProjection.displayName },
      visitorOwnerName: nameOf(inbound.visitorUserId) ?? guestProjection.displayName,
      visitorAgentState: guestProjection.status,
      endsAt: inbound.endsAt ?? '',
      leaseMinutes: 30,
    };
  }
  if (
    outbound &&
    (outboundPhase === 'traveling' || outboundPhase === 'visiting' || outboundPhase === 'returning')
  ) {
    return {
      kind: 'away',
      friendId: outbound.hostUserId,
      friendName: nameOf(outbound.hostUserId) ?? '',
      endsAt: outbound.endsAt ?? '',
      leaseMinutes: 30,
    };
  }
  return { kind: 'home', visitor: null };
}

/**
 * inbound 挂起租约 → 待客事件卡。主人名解析不到（好友列表未加载/关系已解除）
 * 时返回 null——宁可晚一拍出卡，不渲染残缺文案。
 */
export function deriveVisitRequest(
  inbound: VisitLease | null,
  inboundPhase: LocalVisitPhase,
  nameOf: (userId: string) => string | null,
): VisitRequest | null {
  if (!inbound || inboundPhase !== 'pending') return null;
  const ownerName = nameOf(inbound.visitorUserId);
  if (!ownerName) return null;
  // v1 契约在接受前不暴露对方宠物身份：pet.name 留空，事件卡走未知宠物文案
  return {
    id: inbound.id,
    friendId: inbound.visitorUserId,
    ownerName,
    pet: { id: inbound.visitorUserId, name: '' },
  };
}

export function selectHomeEvent(model: SocialHomeModel): HomeEvent | null {
  const visitRequest = authorizedVisitRequest(model);
  if (visitRequest) return { kind: 'visit-request', request: visitRequest };
  if (model.latestMemory) return { kind: 'memory-ready', memory: model.latestMemory };
  if (model.friends.length === 0) return { kind: 'invite-friend' };
  return null;
}

export function authorizedVisitRequest(model: SocialHomeModel): VisitRequest | null {
  const request = model.pendingVisit;
  if (!request) return null;
  if (model.presence.kind !== 'home' || model.presence.visitor !== null) return null;
  return model.friends.some((friend) => friend.id === request.friendId) ? request : null;
}

export function selectFriendAction(
  presence: HomePresence,
  friend: FriendSummary,
  pendingVisit: VisitRequest | null,
): FriendContextAction {
  if (presence.kind === 'away') {
    return {
      kind: 'recall',
      disabledReason: presence.friendId === friend.id ? null : 'away-elsewhere',
    };
  }
  const kind = friend.visitDirection === 'invite-over' ? 'invite' : 'visit';
  if (presence.visitor) return { kind, disabledReason: 'hosting' };
  if (pendingVisit?.friendId === friend.id) {
    return { kind, disabledReason: 'already-requested' };
  }
  return {
    kind,
    disabledReason:
      friend.availability === 'offline' || friend.publicAgentState === 'offline'
        ? 'friend-offline'
        : friend.availability !== 'available'
          ? 'friend-busy'
          : null,
  };
}

// ---------- 平台契约 → Home 模型：共同记忆（P4-C 数据面） ----------
// 网络边界的形状校验在平台层（toSharedMemoryEntry 走共享契约校验器）完成，
// 这里只做纯映射与展示派生。

/**
 * SharedMemoryEntry → 相册摘要。petIds 语义见 SharedMemorySummary；
 * created_at 不可解析的行丢弃（fail-closed，与坏帧同待遇）。
 */
export function memorySummaries(
  entries: SharedMemoryEntry[],
  selfUserId: string | null,
  localPetName: string,
  nameOf: (userId: string) => string,
): SharedMemorySummary[] {
  const summaries: SharedMemorySummary[] = [];
  for (const entry of entries) {
    const occurredAt = Date.parse(entry.createdAt);
    if (!Number.isFinite(occurredAt)) continue;
    const nameFor = (userId: string) => (userId === selfUserId ? localPetName : nameOf(userId));
    summaries.push({
      id: entry.id,
      occurredAt,
      petIds: [nameFor(entry.visitorUserId), nameFor(entry.hostUserId)],
      templateKey: entry.templateKey,
      params: entry.params,
    });
  }
  return summaries;
}

/** 「记忆已整理好」事件卡只播报这个窗口内的新记忆，过期只进相册不上卡。 */
export const MEMORY_ANNOUNCE_WINDOW_MS = 48 * 60 * 60 * 1000;

/**
 * 选出值得上事件卡的最新记忆：48h 内 + 未被本地收起（打开过即收起，
 * 与 dismissedVisitId 同款「本地暂时收起」语义——相册里永远还在）。
 */
export function announceableMemory(
  memories: SharedMemorySummary[],
  dismissedMemoryId: string | null,
  nowMs: number,
): SharedMemorySummary | null {
  const latest = memories.reduce<SharedMemorySummary | null>(
    (best, m) => (best === null || m.occurredAt > best.occurredAt ? m : best),
    null,
  );
  if (!latest) return null;
  if (latest.id === dismissedMemoryId) return null;
  if (nowMs - latest.occurredAt > MEMORY_ANNOUNCE_WINDOW_MS) return null;
  return latest;
}

/**
 * 记忆卡文案：共享契约的本地化表（模板文案 + 参数词表）就是渲染的单一来源，
 * 不进 svelte-i18n——A 线拥有这份文案，fixture 与线上数据同一条渲染路径。
 */
export function memoryCardCopy(
  memory: Pick<SharedMemorySummary, 'templateKey' | 'params'>,
  locale: string | null | undefined,
): { title: string; body: string } {
  const lang: MemoryLocale = locale?.toLowerCase().startsWith('zh') ? 'zh' : 'en';
  const copy = MEMORY_TEMPLATE_LOCALIZATIONS[lang][memory.templateKey];
  const words = MEMORY_PARAMETER_LOCALIZATIONS[lang];
  const body = copy.body
    .replaceAll('{durationBucket}', words.durationBucket[memory.params.durationBucket])
    .replaceAll('{timeOfDay}', words.timeOfDay[memory.params.timeOfDay])
    .replaceAll('{interactionCount}', String(memory.params.interactionCount));
  return { title: copy.title, body };
}

export function allowedHomeActions(model: SocialHomeModel): HomeAction[] {
  if (model.presence.kind === 'away') return ['view-visit', 'recall'];
  if (model.presence.visitor) return ['play', 'snack', 'photo', 'end-visit'];
  return ['feed', 'gift', 'diary', 'send-to-desktop'];
}
