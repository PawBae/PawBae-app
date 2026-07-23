import type { AppMode } from '../types';

export type OnboardingStep = 'welcome' | 'github' | 'invite' | 'agents' | 'adopt';
export type OfficialPetId = 'solu' | 'muru' | 'riffi' | 'luma';
export type AgentId = 'claude' | 'codex' | 'cursor';
export type AgentInstallStatus = 'idle' | 'installing' | 'connected' | 'failed';

export interface GithubProfile {
  login: string;
  displayName?: string;
  avatarUrl?: string;
}

export interface OnboardingResult {
  mode: AppMode;
  shareTelemetry: boolean;
  selectedAgents: AgentId[];
  starterPetId: OfficialPetId | null;
  githubProfile: GithubProfile | null;
}

export interface OfficialPet {
  id: OfficialPetId;
  posterIndex: 0 | 1 | 2 | 3;
  color: string;
  strongColor: string;
}

export const ONBOARDING_STEPS: readonly OnboardingStep[] = [
  'welcome',
  'github',
  'invite',
  'agents',
  'adopt',
] as const;

export const OFFICIAL_PETS: readonly OfficialPet[] = [
  { id: 'solu', posterIndex: 0, color: '#F58F5E', strongColor: '#9C472F' },
  { id: 'muru', posterIndex: 1, color: '#B3C7F0', strongColor: '#455A96' },
  { id: 'riffi', posterIndex: 2, color: '#A8E0C0', strongColor: '#2E6C58' },
  { id: 'luma', posterIndex: 3, color: '#F5AFC8', strongColor: '#7E4160' },
] as const;

/** Only pets with a complete reviewed atlas may be selected in the beta. */
export const SELECTABLE_OFFICIAL_PETS: readonly OfficialPet[] = OFFICIAL_PETS.filter(
  (pet) => pet.id === 'solu',
);

export function nextOnboardingStep(step: OnboardingStep): OnboardingStep {
  const index = ONBOARDING_STEPS.indexOf(step);
  return ONBOARDING_STEPS[Math.min(index + 1, ONBOARDING_STEPS.length - 1)];
}

export function previousOnboardingStep(step: OnboardingStep): OnboardingStep {
  const index = ONBOARDING_STEPS.indexOf(step);
  return ONBOARDING_STEPS[Math.max(index - 1, 0)];
}

export function deriveOnboardingMode(selectedAgents: readonly AgentId[]): AppMode {
  return selectedAgents.length > 0 ? 'coding' : 'pet';
}

export function hookCommandForAgent(
  agent: AgentId,
): 'install_claude_hooks' | 'install_cursor_hooks' {
  return agent === 'cursor' ? 'install_cursor_hooks' : 'install_claude_hooks';
}

export function agentAvailableOnPlatform(agent: AgentId, isWindows: boolean): boolean {
  return !isWindows || agent === 'claude';
}
