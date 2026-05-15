//! OpenClaw agent gateway helpers (invoke_tool, session checks, sessions.json paths, AgentHealth builder).

use std::path::PathBuf;

use crate::commands::agent::{AgentHealth, SessionHealth};
use crate::app_init::home_dir_string;

/// Generic helper: call OpenClaw remote API via /tools/invoke
pub(crate) async fn invoke_tool(
    url: &str,
    token: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/tools/invoke", url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({ "tool": tool, "args": args }))
        .send()
        .await
        .map_err(|e| format!("remote request failed: {}", e))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("remote API error ({}): {}", status, text));
    }
    serde_json::from_str(&text).map_err(|e| {
        format!(
            "parse remote response: {} body: {}",
            e,
            &text[..text.len().min(200)]
        )
    })
}

/// Extract sessions array from remote API response, handling both formats:
/// - Old: { "result": [ ... ] }
/// - New (MCP): { "result": { "content": [...], "details": { "sessions": [...] } } }
pub(crate) fn extract_sessions(result: &serde_json::Value) -> Vec<serde_json::Value> {
    let r = result.get("result").unwrap_or(result);
    if let Some(sessions) = r.pointer("/details/sessions").and_then(|v| v.as_array()) {
        return sessions.clone();
    }
    if let Some(arr) = r.as_array() {
        return arr.clone();
    }
    vec![]
}

/// Check if a session is active (local mode fallback).
pub(crate) fn is_session_active(s: &serde_json::Value) -> bool {
    if let Some(active) = s.get("active").and_then(|v| v.as_bool()) {
        return active;
    }
    if let Some(updated_at) = s.get("updatedAt").and_then(|v| v.as_u64()) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        return now_ms.saturating_sub(updated_at) < 3_000;
    }
    false
}

/// Check Queue status from session_status statusText.
fn is_queue_active(status_text: &str) -> bool {
    status_text.lines().any(|line| {
        if let Some(q) = line.split("Queue:").nth(1) {
            let q = q.trim();
            !q.starts_with("collect") && !q.starts_with("idle") && !q.starts_with("waiting")
        } else {
            false
        }
    })
}

/// Remote activity detection: Queue active (instant) OR updatedAt within 3s (smooth stop).
pub(crate) async fn is_remote_session_active(
    url: &str,
    token: &str,
    session_key: &str,
    s: &serde_json::Value,
) -> bool {
    if let Ok(status) = invoke_tool(
        url,
        token,
        "session_status",
        serde_json::json!({"sessionKey": session_key}),
    )
    .await
    {
        let sr = status.get("result").unwrap_or(&status);
        let det = sr.get("details").unwrap_or(sr);
        if let Some(text) = det["statusText"].as_str() {
            if is_queue_active(text) {
                return true;
            }
        }
    }
    // Queue says idle — use updatedAt as a brief buffer for smooth transition
    is_session_active(s)
}

/// Parse tail lines of a session .jsonl to determine if an agent is active.
///
/// OpenClaw JSONL format: each line is `{"type":"message","message":{...}}`
/// Key fields on `message`:
///   - `role`: "user" | "assistant" | "toolResult"
///   - `usage`: present (object) when an API call is complete
///   - `content`: array of `{type: "text"|"toolCall"|"thinking"|"image", ...}`
///   - NOTE: stop_reason is NOT present in OpenClaw JSONL
///
/// A single turn may involve multiple API calls (tool use loop):
///   1. user message          content=['text']           ← user prompt
///   2. assistant message     content=['toolCall']       ← calls a tool, NOT done
///   3. toolResult message    content=['text']           ← tool output
///   4. assistant message     content=['toolCall']       ← calls another tool, still NOT done
///   5. toolResult message    content=['text']           ← tool output
///   6. assistant message     content=['text']           ← final reply, turn done
///
/// Between steps 2→3 and 4→5 the queue briefly goes idle, but the turn is NOT over.
/// We check: if the last assistant message has "toolCall" content, the turn continues.
/// Also: if the last message is "toolResult", the agent is about to process it → active.
/// This affects: pet working/idle animation, completion sound, session active indicator.
pub(crate) fn check_agent_active_from_lines(lines: &[String]) -> bool {
    let mut last_role = String::new();
    let mut has_usage = false;
    let mut has_tool_call = false;
    for line in lines.iter().rev() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val["type"].as_str() == Some("message") {
                last_role = val["message"]["role"].as_str().unwrap_or("").to_string();
                has_usage = val["message"]["usage"].is_object();
                // Check if assistant message contains a toolCall content block
                if let Some(content) = val["message"]["content"].as_array() {
                    has_tool_call = content
                        .iter()
                        .any(|c| c["type"].as_str() == Some("toolCall"));
                }
                break;
            }
        }
    }
    // Active when:
    //   - last msg is "user" → waiting for assistant response
    //   - last msg is "toolResult" → agent will process tool output next
    //   - last msg is "assistant" without usage → still streaming
    //   - last msg is "assistant" with toolCall content → called a tool, turn continues
    // Inactive when:
    //   - last msg is "assistant" with usage, no toolCall → turn truly ended
    last_role == "user"
        || last_role == "toolResult"
        || (last_role == "assistant" && (!has_usage || has_tool_call))
}

/// Build AgentHealth with session-level data from sessions.json + tail outputs.
pub(crate) fn build_agent_health_from_meta(
    agent_id: &str,
    meta_json: &str,
    tails: &std::collections::HashMap<String, Vec<String>>,
) -> AgentHealth {
    let mut sessions = Vec::new();
    let mut any_active = false;

    if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(meta_json) {
        for (key, val) in map.iter() {
            let sf = val["sessionFile"].as_str().unwrap_or("");
            if sf.is_empty() {
                continue;
            }
            // Match session file path to tail output by basename
            #[cfg(windows)]
            let basename = sf
                .rsplit(|c: char| c == '/' || c == '\\')
                .next()
                .unwrap_or("");
            #[cfg(not(windows))]
            let basename = sf.rsplit('/').next().unwrap_or("");
            let active = if let Some(lines) = tails.get(basename) {
                check_agent_active_from_lines(lines)
            } else {
                false
            };
            if active {
                any_active = true;
            }
            sessions.push(SessionHealth {
                key: key.clone(),
                active,
            });
        }
    }

    // Fallback: no sessions.json or parse failed — check all tails directly (v1.3.3 behavior)
    if sessions.is_empty() && !tails.is_empty() {
        for (fname, lines) in tails {
            let active = check_agent_active_from_lines(lines);
            if active {
                any_active = true;
            }
            // Use filename (without .jsonl) as session key
            let key = fname.strip_suffix(".jsonl").unwrap_or(fname).to_string();
            sessions.push(SessionHealth { key, active });
        }
    }

    AgentHealth {
        agent_id: agent_id.to_string(),
        active: any_active,
        sessions,
    }
}

pub(crate) fn remote_sessions_json_path(agent_id: &str) -> String {
    let agent_dir = if agent_id.is_empty() {
        "main"
    } else {
        agent_id
    };
    format!(
        "$HOME/.openclaw/agents/{}/sessions/sessions.json",
        agent_dir
    )
}

pub(crate) fn sessions_json_path(agent_id: &str) -> PathBuf {
    let home = home_dir_string();
    let agent_dir = if agent_id.is_empty() {
        "main"
    } else {
        agent_id
    };
    PathBuf::from(home)
        .join(".openclaw")
        .join("agents")
        .join(agent_dir)
        .join("sessions")
        .join("sessions.json")
}
