export interface SessionInfo {
  id: string;
  label?: string;
  status: string;
  model?: string;
  channel?: string;
}

export interface AgentInfo {
  id: string;
  identityName?: string;
  identityEmoji?: string;
}

export interface AgentHealth {
  agentId: string;
  active: boolean;
  sessions?: SessionHealth[];
}

export interface SessionHealth {
  key: string;
  active: boolean;
}

export interface ToolCallStat {
  name: string;
  count: number;
}

export interface RecentAction {
  type: 'tool' | 'text';
  summary: string;
  detail?: string;
  timestamp?: string;
}

export interface AgentMetrics {
  agentId: string;
  active: boolean;
  currentModel?: string;
  thinkingLevel?: string;
  activeSessionCount: number;
  currentTask?: string;
  currentTool?: string;
  totalTokens: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  totalCost: number;
  toolCalls: ToolCallStat[];
  recentActions: RecentAction[];
  errorCount: number;
  messageCount: number;
  sessionStart?: string;
  lastActivity?: string;
  channel?: string;
}

export interface OcConnection {
  id: string;
  type: 'local' | 'remote';
  host?: string;
  user?: string;
}

export type AppMode = 'coding' | 'pet';

export type PetAction =
  | 'idle'
  | 'sleep'
  | 'work'
  | 'study'
  | 'watch'
  | 'music'
  | 'walk'
  | 'dance'
  | 'eat'
  | 'hungry'
  | 'headpat'
  | 'farewell'
  | 'grasp'
  | 'angry'
  | 'spin'
  | 'milktea'
  | 'rest'
  | 'peek'
  | 'walkout';

export interface PetData {
  hunger: number;
  affection: number;
  coins: number;
  lastTickAt: number;
  lastDailyGift: string;
  headpatToday: number;
  headpatDate: string;
  pomodoroCoins: number;
}

export interface PomodoroState {
  active: boolean;
  duration: number;
  remaining: number;
  startedAt: number;
}

export interface ClaudeSession {
  id: string;
  cwd?: string;
  source?: string;
  status?: string;
  model?: string;
  updatedAt?: number;
  nickname?: string;
}

export type ClaudeStatsSource = 'cc' | 'codex' | 'cursor';

export interface HealthResult {
  agents: AgentHealth[];
  gatewayAlive: boolean;
}

export interface MiniSessionInfo {
  agentId: string;
  connectionId: string;
  key: string;
  sessionId?: string;
  label?: string;
  channel?: string;
  preview?: string;
  active: boolean;
}
