import { invoke } from '@tauri-apps/api/core';
import type { ClaudeSession, ClaudeStatsSource } from '../types';

class SessionStore {
  claudeSessions = $state.raw<ClaudeSession[]>([]);
  selectedClaudeSession = $state<string | null>(null);
  claudeConversation = $state.raw<unknown[]>([]);
  showClaudeStats = $state(false);
  claudeStatsSource = $state<ClaudeStatsSource>('cc');
  sessionNicknames = $state.raw<Record<string, string>>({});

  private pollInterval: ReturnType<typeof setInterval> | null = null;
  private pollBusy = false;

  async pollClaudeSessions() {
    if (this.pollBusy) return;
    this.pollBusy = true;
    try {
      const sessions = (await invoke('get_claude_sessions')) as ClaudeSession[];
      this.claudeSessions = sessions;
    } catch {
      // ignore
    } finally {
      this.pollBusy = false;
    }
  }

  async fetchConversation(sessionId: string) {
    try {
      const conv = (await invoke('get_claude_conversation', { sessionId })) as unknown[];
      this.claudeConversation = conv;
    } catch {
      this.claudeConversation = [];
    }
  }

  selectSession(sessionId: string | null) {
    this.selectedClaudeSession = sessionId;
    if (sessionId) {
      this.fetchConversation(sessionId);
    } else {
      this.claudeConversation = [];
    }
  }

  startPolling() {
    this.stopPolling();
    this.pollClaudeSessions();
    this.pollInterval = setInterval(() => this.pollClaudeSessions(), 2000);
  }

  stopPolling() {
    if (this.pollInterval) {
      clearInterval(this.pollInterval);
      this.pollInterval = null;
    }
  }

  setShowClaudeStats(v: boolean) {
    this.showClaudeStats = v;
  }

  setClaudeStatsSource(v: ClaudeStatsSource) {
    this.claudeStatsSource = v;
  }

  setNickname(sessionId: string, name: string) {
    this.sessionNicknames = { ...this.sessionNicknames, [sessionId]: name };
  }
}

export const sessionStore = new SessionStore();
