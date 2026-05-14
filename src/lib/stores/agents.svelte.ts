import { invoke } from '@tauri-apps/api/core';
import type {
  AgentInfo,
  AgentMetrics,
  HealthResult,
  MiniSessionInfo,
  OcConnection,
} from '../types';

class AgentStore {
  agents = $state.raw<AgentInfo[]>([]);
  healthMap = $state.raw<Record<string, boolean>>({});
  allSessions = $state.raw<MiniSessionInfo[]>([]);
  anySessionActive = $state(false);
  selectedAgentId = $state<string | null>(null);
  metrics = $state<AgentMetrics | null>(null);
  connections = $state.raw<OcConnection[]>([{ id: 'local', type: 'local' }]);

  private agentRealIdMap = new Map<string, string>();
  private agentConnMap = new Map<string, Record<string, string>>();
  private pollIntervals: ReturnType<typeof setInterval>[] = [];
  private fetchBusy = false;
  private healthBusy = false;

  connParams(agentId: string): Record<string, string> {
    return this.agentConnMap.get(agentId) || {};
  }

  async fetchAgents() {
    if (this.fetchBusy) return;
    this.fetchBusy = true;
    try {
      const newAgents: AgentInfo[] = [];
      this.agentRealIdMap.clear();
      this.agentConnMap.clear();

      for (const conn of this.connections) {
        const params: Record<string, string | undefined> = {};
        if (conn.type === 'remote') {
          params.mode = 'remote';
          params.sshHost = conn.host;
          params.sshUser = conn.user;
        }
        try {
          const list = (await invoke('get_agents', params)) as AgentInfo[];
          for (const a of list) {
            const uniqueId = conn.type === 'remote' ? `${conn.id}:${a.id}` : a.id;
            this.agentRealIdMap.set(uniqueId, a.id);
            if (conn.type === 'remote') {
              this.agentConnMap.set(uniqueId, {
                mode: 'remote',
                sshHost: conn.host!,
                sshUser: conn.user!,
              });
            }
            newAgents.push({ ...a, id: uniqueId });
          }
        } catch {
          // connection failed, skip
        }
      }
      this.agents = newAgents;
    } finally {
      this.fetchBusy = false;
    }
  }

  async pollHealth() {
    if (this.healthBusy) return;
    this.healthBusy = true;
    try {
      const newMap: Record<string, boolean> = {};
      for (const conn of this.connections) {
        const params: Record<string, string | undefined> = {};
        if (conn.type === 'remote') {
          params.mode = 'remote';
          params.sshHost = conn.host;
          params.sshUser = conn.user;
        }
        try {
          const result = (await invoke('get_health', params)) as HealthResult;
          for (const ah of result.agents) {
            const uniqueId = conn.type === 'remote' ? `${conn.id}:${ah.agentId}` : ah.agentId;
            newMap[uniqueId] = ah.active;
          }
        } catch {
          // skip
        }
      }
      this.healthMap = newMap;
      this.anySessionActive = Object.values(newMap).some(Boolean);
    } finally {
      this.healthBusy = false;
    }
  }

  async fetchMetrics() {
    if (!this.selectedAgentId) {
      this.metrics = null;
      return;
    }
    const realId = this.agentRealIdMap.get(this.selectedAgentId) || this.selectedAgentId;
    try {
      const m = (await invoke('get_agent_metrics', {
        agentId: realId,
        ...this.connParams(this.selectedAgentId),
      })) as AgentMetrics;
      this.metrics = m;
    } catch {
      this.metrics = null;
    }
  }

  startPolling() {
    this.stopPolling();
    this.fetchAgents();
    this.pollHealth();
    this.pollIntervals.push(setInterval(() => this.fetchAgents(), 5000));
    this.pollIntervals.push(setInterval(() => this.pollHealth(), 1000));
  }

  stopPolling() {
    for (const id of this.pollIntervals) clearInterval(id);
    this.pollIntervals = [];
  }

  selectAgent(id: string | null) {
    this.selectedAgentId = id;
    if (id) this.fetchMetrics();
  }

  setConnections(conns: OcConnection[]) {
    this.connections = conns;
  }
}

export const agentStore = new AgentStore();
