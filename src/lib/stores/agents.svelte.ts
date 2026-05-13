import { invoke } from '@tauri-apps/api/core';
import type {
  AgentInfo,
  AgentHealth,
  AgentMetrics,
  HealthResult,
  MiniSessionInfo,
  OcConnection,
} from '../types';

let agents = $state<AgentInfo[]>([]);
let healthMap = $state<Record<string, boolean>>({});
let allSessions = $state<MiniSessionInfo[]>([]);
let anySessionActive = $state(false);
let selectedAgentId = $state<string | null>(null);
let metrics = $state<AgentMetrics | null>(null);
let connections = $state<OcConnection[]>([{ id: 'local', type: 'local' }]);

let agentRealIdMap = new Map<string, string>();
let agentConnMap = new Map<string, Record<string, string>>();

let pollIntervals: ReturnType<typeof setInterval>[] = [];
let fetchBusy = false;
let healthBusy = false;

function connParams(agentId: string): Record<string, string> {
  return agentConnMap.get(agentId) || {};
}

async function fetchAgents() {
  if (fetchBusy) return;
  fetchBusy = true;
  try {
    const newAgents: AgentInfo[] = [];
    agentRealIdMap.clear();
    agentConnMap.clear();

    for (const conn of connections) {
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
          agentRealIdMap.set(uniqueId, a.id);
          if (conn.type === 'remote') {
            agentConnMap.set(uniqueId, {
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
    agents = newAgents;
  } finally {
    fetchBusy = false;
  }
}

async function pollHealth() {
  if (healthBusy) return;
  healthBusy = true;
  try {
    const newMap: Record<string, boolean> = {};
    for (const conn of connections) {
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
    healthMap = newMap;
    anySessionActive = Object.values(newMap).some(Boolean);
  } finally {
    healthBusy = false;
  }
}

async function fetchMetrics() {
  if (!selectedAgentId) {
    metrics = null;
    return;
  }
  const realId = agentRealIdMap.get(selectedAgentId) || selectedAgentId;
  try {
    const m = (await invoke('get_agent_metrics', {
      agentId: realId,
      ...connParams(selectedAgentId),
    })) as AgentMetrics;
    metrics = m;
  } catch {
    metrics = null;
  }
}

function startPolling() {
  stopPolling();
  fetchAgents();
  pollHealth();
  pollIntervals.push(setInterval(fetchAgents, 5000));
  pollIntervals.push(setInterval(pollHealth, 1000));
}

function stopPolling() {
  for (const id of pollIntervals) clearInterval(id);
  pollIntervals = [];
}

function selectAgent(id: string | null) {
  selectedAgentId = id;
  if (id) fetchMetrics();
}

function setConnections(conns: OcConnection[]) {
  connections = conns;
}

export const agentStore = {
  get agents() { return agents; },
  get healthMap() { return healthMap; },
  get allSessions() { return allSessions; },
  get anySessionActive() { return anySessionActive; },
  get selectedAgentId() { return selectedAgentId; },
  get metrics() { return metrics; },
  get connections() { return connections; },
  startPolling,
  stopPolling,
  fetchAgents,
  pollHealth,
  fetchMetrics,
  selectAgent,
  setConnections,
  connParams,
  get agentRealIdMap() { return agentRealIdMap; },
};
