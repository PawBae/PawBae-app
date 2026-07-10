//! Tauri agent-facing commands: status, chat send, agent listing, health, metrics, interrupt, extra-info.

mod control;
mod discovery;
mod gateway;
mod metrics;

pub use control::*;
pub use discovery::*;
pub use gateway::*;
pub use metrics::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayStatus {
    pub active: bool,
    pub sessions: Vec<crate::state::SessionInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentInfo {
    pub id: String,
    #[serde(rename = "identityName")]
    pub identity_name: Option<String>,
    #[serde(rename = "identityEmoji")]
    pub identity_emoji: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionHealth {
    pub key: String,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentHealth {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub active: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<SessionHealth>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResult {
    pub agents: Vec<AgentHealth>,
    /// Whether the local OpenClaw gateway process is running.
    /// Always `true` for remote connections (we can't check remote process).
    /// Frontend uses this to auto-remove the local connection when gateway is dead.
    #[serde(default = "default_true", rename = "gatewayAlive")]
    pub gateway_alive: bool,
}

pub(super) fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallStat {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecentAction {
    /// "tool" or "text"
    #[serde(rename = "type")]
    pub action_type: String,
    /// tool name (for tool) or text snippet (for text)
    pub summary: String,
    pub detail: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentMetrics {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub active: bool,
    #[serde(rename = "currentModel")]
    pub current_model: Option<String>,
    #[serde(rename = "thinkingLevel")]
    pub thinking_level: Option<String>,
    #[serde(rename = "activeSessionCount")]
    pub active_session_count: usize,
    #[serde(rename = "currentTask")]
    pub current_task: Option<String>,
    #[serde(rename = "currentTool")]
    pub current_tool: Option<String>,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u64,
    #[serde(rename = "inputTokens")]
    pub input_tokens: u64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(rename = "cacheReadTokens")]
    pub cache_read_tokens: u64,
    #[serde(rename = "cacheWriteTokens")]
    pub cache_write_tokens: u64,
    #[serde(rename = "totalCost")]
    pub total_cost: f64,
    #[serde(rename = "toolCalls")]
    pub tool_calls: Vec<ToolCallStat>,
    #[serde(rename = "recentActions")]
    pub recent_actions: Vec<RecentAction>,
    #[serde(rename = "errorCount")]
    pub error_count: usize,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    #[serde(rename = "sessionStart")]
    pub session_start: Option<String>,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<String>,
    pub channel: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DailyCount {
    pub(super) date: String,
    pub(super) count: u32,
    pub(super) tokens: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AgentExtraInfo {
    pub(super) skills: Vec<String>,
    pub(super) cron_jobs: Vec<serde_json::Value>,
    pub(super) daily_counts: Vec<DailyCount>,
}
