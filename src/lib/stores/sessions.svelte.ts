import { invoke } from '@tauri-apps/api/core';
import type { ClaudeSession, ClaudeStatsSource } from '../types';

let claudeSessions = $state<ClaudeSession[]>([]);
let selectedClaudeSession = $state<string | null>(null);
let claudeConversation = $state<unknown[]>([]);
let showClaudeStats = $state(false);
let claudeStatsSource = $state<ClaudeStatsSource>('cc');
let sessionNicknames = $state<Record<string, string>>({});

let pollInterval: ReturnType<typeof setInterval> | null = null;
let pollBusy = false;

async function pollClaudeSessions() {
  if (pollBusy) return;
  pollBusy = true;
  try {
    const sessions = (await invoke('get_claude_sessions')) as ClaudeSession[];
    claudeSessions = sessions;
  } catch {
    // ignore
  } finally {
    pollBusy = false;
  }
}

async function fetchConversation(sessionId: string) {
  try {
    const conv = (await invoke('get_claude_conversation', { sessionId })) as unknown[];
    claudeConversation = conv;
  } catch {
    claudeConversation = [];
  }
}

function selectSession(sessionId: string | null) {
  selectedClaudeSession = sessionId;
  if (sessionId) {
    fetchConversation(sessionId);
  } else {
    claudeConversation = [];
  }
}

function startPolling() {
  stopPolling();
  pollClaudeSessions();
  pollInterval = setInterval(pollClaudeSessions, 2000);
}

function stopPolling() {
  if (pollInterval) {
    clearInterval(pollInterval);
    pollInterval = null;
  }
}

function setShowClaudeStats(v: boolean) {
  showClaudeStats = v;
}

function setClaudeStatsSource(v: ClaudeStatsSource) {
  claudeStatsSource = v;
}

function setNickname(sessionId: string, name: string) {
  sessionNicknames = { ...sessionNicknames, [sessionId]: name };
}

export const sessionStore = {
  get claudeSessions() { return claudeSessions; },
  get selectedClaudeSession() { return selectedClaudeSession; },
  get claudeConversation() { return claudeConversation; },
  get showClaudeStats() { return showClaudeStats; },
  get claudeStatsSource() { return claudeStatsSource; },
  get sessionNicknames() { return sessionNicknames; },
  pollClaudeSessions,
  fetchConversation,
  selectSession,
  startPolling,
  stopPolling,
  setShowClaudeStats,
  setClaudeStatsSource,
  setNickname,
};
