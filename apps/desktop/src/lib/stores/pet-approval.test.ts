// Approval-note store glue: applyApprovalResponse's affection commit and daily cap
// through the live store. Window/cap MATH is covered by utils/approval-note.test.ts;
// this checks the petData wiring (counter fields, day rollover, clamp).
import { describe, expect, it, vi } from 'vitest';

const harness = vi.hoisted(() => ({
  data: new Map<string, unknown>(),
}));

vi.mock('@tauri-apps/plugin-store', () => ({
  load: async () => ({
    get: async (key: string) => harness.data.get(key),
    set: async (key: string, value: unknown) => {
      harness.data.set(key, value);
    },
    save: async () => {},
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: async () => () => {},
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: async () => null,
}));

import {
  AFFECTION_APPROVAL,
  APPROVAL_DAILY_LIMIT,
  APPROVAL_FAST_RESPONSE_MS,
} from '../utils/approval-note';
import { AFFECTION_MAX, petStore } from './pet.svelte';

describe('petStore approval responses', () => {
  it('awards affection for a fast response and counts it', () => {
    petStore.loadPetData({ ...petStore.defaultPetData(), affection: 50 });
    expect(petStore.applyApprovalResponse(30_000)).toBe(true);
    expect(petStore.petData.affection).toBe(50 + AFFECTION_APPROVAL);
    expect(petStore.petData.approvalToday).toBe(1);
  });

  it('a slow response changes nothing — never punish, never reward late', () => {
    petStore.loadPetData({ ...petStore.defaultPetData(), affection: 50 });
    expect(petStore.applyApprovalResponse(APPROVAL_FAST_RESPONSE_MS + 1)).toBe(false);
    expect(petStore.petData.affection).toBe(50);
    expect(petStore.petData.approvalToday).toBe(0);
  });

  it('stops awarding at the daily cap', () => {
    petStore.loadPetData({
      ...petStore.defaultPetData(),
      affection: 50,
      approvalToday: APPROVAL_DAILY_LIMIT,
    });
    expect(petStore.applyApprovalResponse(1_000)).toBe(false);
    expect(petStore.petData.affection).toBe(50);
  });

  it('a stale counter date restarts the count', () => {
    petStore.loadPetData({
      ...petStore.defaultPetData(),
      affection: 50,
      approvalToday: APPROVAL_DAILY_LIMIT,
      approvalDate: '2020-01-01',
    });
    expect(petStore.applyApprovalResponse(1_000)).toBe(true);
    expect(petStore.petData.approvalToday).toBe(1);
  });

  it('clamps affection at the max', () => {
    petStore.loadPetData({ ...petStore.defaultPetData(), affection: AFFECTION_MAX - 1 });
    expect(petStore.applyApprovalResponse(1_000)).toBe(true);
    expect(petStore.petData.affection).toBe(AFFECTION_MAX);
  });
});
