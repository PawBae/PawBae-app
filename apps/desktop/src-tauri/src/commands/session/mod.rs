//! Tauri session-facing commands: agent sessions, message lists, previews, Claude session bookkeeping and stats.

mod agent_sessions;
mod claude_sessions;
mod helpers;

use serde::{Deserialize, Serialize};

// ── Public types used by commands ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniSessionInfo {
    pub key: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub label: String,
    pub channel: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
    pub active: bool,
    #[serde(rename = "lastUserMsg")]
    pub last_user_msg: Option<String>,
    #[serde(rename = "lastAssistantMsg")]
    pub last_assistant_msg: Option<String>,
    #[serde(
        rename = "sessionFile",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub session_file: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionPreview {
    pub active: bool,
    #[serde(rename = "lastUserMsg")]
    pub last_user_msg: Option<String>,
    #[serde(rename = "lastAssistantMsg")]
    pub last_assistant_msg: Option<String>,
}

// ── Re-exports: all #[tauri::command] functions ──

pub use agent_sessions::{
    get_active_sessions, get_agent_sessions, get_session_messages, get_session_preview,
};
pub use claude_sessions::{
    get_claude_conversation, get_claude_sessions, get_claude_stats, remove_claude_session,
};
