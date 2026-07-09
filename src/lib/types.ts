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

// Batched global-input fact emitted by the Rust backend as the Tauri "user-input" event.
// Mirrors src-tauri/src/input/aggregator.rs `UserInputEvent` (serde rename_all = "lowercase").
// `count` is already aggregated per ~80ms tick — one event drives at most ONE animation.
export interface UserInputEvent {
  kind: 'keyboard' | 'mouse';
  count: number;
  at: number;
}

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
  // Approval note (叼来审批单): fast responses awarded today. Ephemeral daily counter
  // like headpat's — survives day rollover within a run, not restarts.
  approvalToday: number;
  approvalDate: string;
  pomodoroCoins: number;
  // Phase 6 growth: consecutive daily-gift days (FROZEN since the daily task board
  // absorbed it — kept for old-build rollback safety) and the adoption moment
  // (persisted once, drives "days together" memories + achievements).
  giftStreak: number;
  firstMeetAt: number;
  // Daily task board + forgiving streak (utils/daily-board.ts BoardState, flattened).
  // `streak` is THE app-wide streak; boardDone holds today's ticked task ids.
  boardDate: string;
  boardDone: import('./utils/daily-board').BoardTaskId[];
  streak: number;
  streakDate: string;
  shields: number;
}

// A queued growth moment the mascot celebrates (evolution flash / achievement toast /
// full task board). Newest-last; MascotView displays the head and shifts it after the
// show beat.
export type GrowthCelebration =
  | { kind: 'evolution'; stageIndex: number }
  | { kind: 'achievement'; id: string }
  | { kind: 'perfect_day' };

export interface PomodoroState {
  active: boolean;
  duration: number;
  remaining: number;
  startedAt: number;
}

// Wire shape of `get_claude_sessions` (state.rs ClaudeSession: serde renames
// session_id → sessionId). The field was previously typed `id`, which the wire
// never carried — every consumer read undefined.
export interface ClaudeSession {
  sessionId: string;
  cwd?: string;
  source?: string;
  status?: string;
  model?: string;
  updatedAt?: number;
  nickname?: string;
}

export type ClaudeStatsSource = 'cc' | 'codex' | 'cursor';

// Wire shape of one day in `get_claude_stats` dailyStats. The Rust struct
// (ClaudeDailyStats) has NO serde renames, so the wire is snake_case — typed
// as-is on purpose (the ClaudeSession `id`-vs-`sessionId` lesson).
export interface ClaudeDailyStats {
  date: string; // YYYY-MM-DD
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
  messages: number;
  sessions: number;
}

// Wire shape of the `get_claude_stats` Tauri command (claude_sessions.rs ClaudeStats,
// serde camelCase renames on the totals; dailyStats entries stay snake_case).
// dailyStats covers the trailing 14 days, zero-filled for missing days, oldest first.
export interface ClaudeStats {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheWriteTokens: number;
  totalMessages: number;
  totalSessions: number;
  dailyStats: ClaudeDailyStats[];
}

// Wire payload of the Tauri "claude-task-complete" event
// (src-tauri/src/commands/hook/event_process.rs — camelCase keys). Rust pre-filters:
// it emits only on a genuine main-agent completion (Stop with no pending sub-agents,
// not ESC-interrupted, not compacting) or a permission-wait (`waiting: true`).
// Reward rule: a completion is `waiting === false`.
export interface ClaudeTaskCompleteEvent {
  sessionId: string;
  waiting: boolean;
  source: ClaudeStatsSource;
}

// ── P1-C reward model ──────────────────────────────────────────────

// Where a coin delta came from. Negative deltas (feed) flow through the same pipeline.
export type CoinSource =
  | 'agent_stop'
  | 'focus_minutes'
  | 'input_milestone'
  | 'pomodoro'
  | 'daily_gift'
  | 'task_board'
  | 'feed';

// One reward-ledger entry. `amount` is the EFFECTIVE delta after the clamp-at-zero
// (positive = earned, negative = spent); zero-effect awards are never ledgered.
export interface CoinAward {
  source: CoinSource;
  amount: number;
  at: number;
  reason?: string;
  sessionId?: string;
}

// Lifetime per-source aggregate, kept even after old `recent` entries are trimmed.
export interface CoinSourceTotals {
  earned: number;
  spent: number;
  count: number;
}

// Persisted shape of the reward ledger: totals + capped recent entries + milestone progress.
export interface RewardLedgerSnapshot {
  totals: Record<CoinSource, CoinSourceTotals>;
  recent: CoinAward[];
  lifetimeInputCount: number;
  lastAwardedMilestone: number;
}

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

export interface UpdateModalInfo {
  current: string;
  latest: string;
  hasUpdate: boolean;
  url?: string;
  notes?: string;
}
