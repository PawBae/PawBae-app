import type { ProjectionStatus } from '../platform/types';

export type VisitInteraction = 'arrival' | 'nose-touch' | 'side-by-side' | 'celebrate' | 'rest';

export interface VisitStageInput {
  leaseId: string;
  localStatus: ProjectionStatus;
  guestStatus: ProjectionStatus;
  timeBucket: number;
  reducedMotion: boolean;
}

function stableHash(value: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/**
 * Both peers can derive the same gentle interaction without broadcasting
 * coordinates or animation frames. Agent mood only influences the local
 * choice; the lease remains the correctness source.
 */
export function visitInteractionFor(input: VisitStageInput): VisitInteraction {
  if (input.reducedMotion || input.localStatus === 'offline' || input.guestStatus === 'offline') {
    return 'rest';
  }
  if (input.localStatus === 'working' || input.guestStatus === 'working') {
    return 'side-by-side';
  }
  if (input.localStatus === 'waiting' || input.guestStatus === 'waiting') {
    return 'nose-touch';
  }
  const choices: VisitInteraction[] = ['arrival', 'nose-touch', 'celebrate', 'rest'];
  return choices[stableHash(`${input.leaseId}:${input.timeBucket}`) % choices.length];
}
