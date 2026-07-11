//! Agent metrics, message extraction, and string truncation helpers.

use std::sync::Arc;

use tauri::Manager;

use crate::agent_gateway::{
    extract_sessions, invoke_tool, is_session_active, remote_sessions_json_path, sessions_json_path,
};
use crate::lsof::lsof_active_agents;
use crate::ssh_core::{ssh_is_agent_active, ssh_read_file};
use crate::state::SshState;

use super::{AgentMetrics, RecentAction, ToolCallStat};

/// Extract the actual user message from openclaw's metadata-wrapped format.
/// Handles both direct messages and queued messages.
/// Formats:
///   - `Conversation info...\n[message_id: xxx]\nSender: actual message`
///   - `[Queued messages...]\n---\nQueued #N\n...\n[message_id: xxx]\nSender: msg\n---\nQueued #M\n...`
///   - `[timestamp] message` (simple format)
pub(super) fn extract_user_message(text: &str) -> Option<String> {
    // For queued messages, extract the last queued message's content
    if text.starts_with("[Queued messages") {
        // Find the last "[message_id: ...]" line and take the line after it
        let mut last_msg: Option<String> = None;
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("[message_id:") {
                // Next line is "sender: actual message"
                if let Some(next) = lines.get(i + 1) {
                    // Strip "Sender: " prefix if present
                    let content = if let Some(pos) = next.find(": ") {
                        &next[pos + 2..]
                    } else {
                        next
                    };
                    if !content.trim().is_empty() {
                        last_msg = Some(content.trim().to_string());
                    }
                }
            }
        }
        return last_msg.map(|m| truncate_str(&m, 100));
    }

    // For regular messages with metadata wrapper
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("[message_id:") {
            // Next line is "sender: actual message"
            if let Some(next) = lines.get(i + 1) {
                let content = if let Some(pos) = next.find(": ") {
                    &next[pos + 2..]
                } else {
                    next
                };
                if !content.trim().is_empty() {
                    return Some(truncate_str(content.trim(), 100));
                }
            }
        }
    }

    // Simple format: "[timestamp] message"
    if text.starts_with('[') {
        if let Some(end) = text.find(']') {
            let after = text[end + 1..].trim();
            if !after.is_empty() {
                return Some(truncate_str(after, 100));
            }
        }
    }

    // Fallback: first non-empty line
    text.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| truncate_str(l.trim(), 100))
}

pub(super) fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Truncate at char boundary
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

#[tauri::command]
pub async fn get_agent_metrics(
    app: tauri::AppHandle,
    agent_id: String,
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<AgentMetrics, String> {
    log::info!(
        "[get_agent_metrics] agent_id={} mode={:?} ssh_host={:?}",
        agent_id,
        mode,
        ssh_host
    );
    if mode.as_deref() == Some("remote") {
        let ssh = app.state::<Arc<SshState>>();
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let active = ssh_is_agent_active(&ssh, sh, su, &agent_id).await;

            let mut metrics = AgentMetrics {
                agent_id: agent_id.clone(),
                active,
                current_model: None,
                thinking_level: None,
                active_session_count: 0,
                current_task: None,
                current_tool: None,
                total_tokens: 0,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_cost: 0.0,
                tool_calls: vec![],
                recent_actions: vec![],
                error_count: 0,
                message_count: 0,
                session_start: None,
                last_activity: None,
                channel: None,
            };

            let sess_path = remote_sessions_json_path(&agent_id);
            let sess_content = match ssh_read_file(&ssh, sh, su, &sess_path).await {
                Ok(c) => c,
                Err(_) => return Ok(metrics),
            };
            let sess_map: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&sess_content).unwrap_or_default();

            metrics.active_session_count = sess_map.len();

            let best_entry = sess_map
                .values()
                .max_by_key(|v| v["updatedAt"].as_u64().unwrap_or(0));
            if let Some(entry) = best_entry {
                metrics.channel = entry["origin"]["surface"].as_str().map(|s| s.to_string());
                metrics.current_model = entry["model"].as_str().map(|s| s.to_string());
            }

            let mut best_session: Option<(String, u64)> = None;
            for val in sess_map.values() {
                if let (Some(file), Some(updated)) =
                    (val["sessionFile"].as_str(), val["updatedAt"].as_u64())
                {
                    if best_session.as_ref().map_or(true, |(_, t)| updated > *t) {
                        best_session = Some((file.to_string(), updated));
                    }
                }
            }

            let session_file = match best_session {
                Some((f, _)) => f,
                None => return Ok(metrics),
            };

            let content = match ssh_read_file(&ssh, sh, su, &session_file).await {
                Ok(c) => {
                    log::info!(
                        "[get_agent_metrics] SSH read session file OK, len={}",
                        c.len()
                    );
                    c
                }
                Err(e) => {
                    log::error!("[get_agent_metrics] SSH read session file failed: {}", e);
                    return Ok(metrics);
                }
            };

            let mut tool_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            let mut last_user_text: Option<String> = None;
            let mut last_tool_name: Option<String> = None;
            let mut last_timestamp: Option<String> = None;
            let mut recent_actions: Vec<RecentAction> = vec![];
            #[allow(unused_assignments)]
            let mut current_msg_timestamp: Option<String> = None;

            for line in content.lines() {
                let val: serde_json::Value = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let event_type = val["type"].as_str().unwrap_or("");
                if let Some(ts) = val["timestamp"].as_str() {
                    last_timestamp = Some(ts.to_string());
                }
                match event_type {
                    "session" => {
                        metrics.session_start = val["timestamp"].as_str().map(|s| s.to_string());
                    }
                    "model_change" => {
                        metrics.current_model = val["modelId"].as_str().map(|s| s.to_string());
                    }
                    "thinking_level_change" => {
                        metrics.thinking_level =
                            val["thinkingLevel"].as_str().map(|s| s.to_string());
                    }
                    "message" => {
                        let msg = &val["message"];
                        let role = msg["role"].as_str().unwrap_or("");
                        current_msg_timestamp = val["timestamp"].as_str().map(|s| s.to_string());
                        if role == "user" {
                            if let Some(content_arr) = msg["content"].as_array() {
                                for item in content_arr {
                                    if item["type"].as_str() == Some("text") {
                                        if let Some(text) = item["text"].as_str() {
                                            last_user_text = extract_user_message(text);
                                        }
                                    }
                                }
                            }
                            metrics.message_count += 1;
                        } else if role == "assistant" {
                            if let Some(usage) = msg["usage"].as_object() {
                                metrics.input_tokens +=
                                    usage.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
                                metrics.output_tokens +=
                                    usage.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
                                metrics.cache_read_tokens +=
                                    usage.get("cacheRead").and_then(|v| v.as_u64()).unwrap_or(0);
                                metrics.cache_write_tokens += usage
                                    .get("cacheWrite")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                metrics.total_tokens += usage
                                    .get("totalTokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                if let Some(cost) =
                                    usage.get("cost").and_then(|c| c["total"].as_f64())
                                {
                                    metrics.total_cost += cost;
                                }
                            }
                            if let Some(content_arr) = msg["content"].as_array() {
                                for item in content_arr {
                                    match item["type"].as_str() {
                                        Some("toolCall") => {
                                            if let Some(name) = item["name"].as_str() {
                                                *tool_counts
                                                    .entry(name.to_string())
                                                    .or_insert(0) += 1;
                                                last_tool_name = Some(name.to_string());
                                                let detail = item["input"]
                                                    .as_object()
                                                    .map(|obj| {
                                                        obj.iter()
                                                            .map(|(k, v)| {
                                                                let val_str = match v.as_str() {
                                                                    Some(s) => truncate_str(s, 300),
                                                                    None => {
                                                                        let j = v.to_string();
                                                                        truncate_str(&j, 100)
                                                                    }
                                                                };
                                                                format!("{}: {}", k, val_str)
                                                            })
                                                            .collect::<Vec<_>>()
                                                            .join("\n")
                                                    })
                                                    .filter(|s| !s.is_empty());
                                                recent_actions.push(RecentAction {
                                                    action_type: "tool".to_string(),
                                                    summary: name.to_string(),
                                                    detail,
                                                    timestamp: current_msg_timestamp.clone(),
                                                });
                                            }
                                        }
                                        Some("text") => {
                                            if let Some(text) = item["text"].as_str() {
                                                let trimmed = text.trim();
                                                if !trimmed.is_empty() {
                                                    let summary = truncate_str(trimmed, 60);
                                                    let detail = if trimmed.len() > 60 {
                                                        Some(truncate_str(trimmed, 500))
                                                    } else {
                                                        None
                                                    };
                                                    recent_actions.push(RecentAction {
                                                        action_type: "text".to_string(),
                                                        summary,
                                                        detail,
                                                        timestamp: current_msg_timestamp.clone(),
                                                    });
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            metrics.message_count += 1;
                        }
                    }
                    "custom"
                        if val["customType"]
                            .as_str()
                            .is_some_and(|t| t.contains("error")) =>
                    {
                        metrics.error_count += 1;
                    }
                    _ => {}
                }
            }

            metrics.current_task = last_user_text;
            metrics.current_tool = last_tool_name;
            metrics.last_activity = last_timestamp;

            let len = recent_actions.len();
            if len > 3 {
                metrics.recent_actions = recent_actions[len - 3..].to_vec();
            } else {
                metrics.recent_actions = recent_actions;
            }
            metrics.recent_actions.reverse();

            let mut tool_vec: Vec<ToolCallStat> = tool_counts
                .into_iter()
                .map(|(name, count)| ToolCallStat { name, count })
                .collect();
            tool_vec.sort_by_key(|t| std::cmp::Reverse(t.count));
            metrics.tool_calls = tool_vec;

            log::info!("[get_agent_metrics] SSH result: active={} recent_actions={} tool_calls={} message_count={} current_task={:?}",
                metrics.active, metrics.recent_actions.len(), metrics.tool_calls.len(), metrics.message_count, metrics.current_task);
            return Ok(metrics);
        }
        // Gateway API fallback
        let url = url.as_deref().unwrap_or("");
        let tok = token.as_deref().unwrap_or("");
        let result = invoke_tool(
            url,
            tok,
            "sessions_list",
            serde_json::json!({"agentId": agent_id, "activeMinutes": 60}),
        )
        .await?;
        let sessions = extract_sessions(&result);
        let active_count = sessions.iter().filter(|s| is_session_active(s)).count();
        let total_tokens: u64 = sessions
            .iter()
            .map(|s| s["totalTokens"].as_u64().unwrap_or(0))
            .sum();
        let model = sessions
            .iter()
            .find_map(|s| s["model"].as_str().map(|s| s.to_string()));
        let channel = sessions
            .iter()
            .find_map(|s| s["channel"].as_str().map(|s| s.to_string()));
        let last_updated = sessions
            .iter()
            .filter_map(|s| s["updatedAt"].as_u64())
            .max();
        let last_activity = last_updated.map(|ms| {
            let secs = (ms / 1000) as i64;
            chrono::DateTime::from_timestamp(secs, 0)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                .unwrap_or_default()
        });
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut current_task: Option<String> = None;
        let default_key = format!("agent:{}:main", agent_id);
        let session_key = sessions
            .first()
            .and_then(|s| s["key"].as_str())
            .unwrap_or(&default_key);
        if let Ok(status_result) = invoke_tool(
            url,
            tok,
            "session_status",
            serde_json::json!({"sessionKey": session_key}),
        )
        .await
        {
            let sr = status_result.get("result").unwrap_or(&status_result);
            let det = sr.get("details").unwrap_or(sr);
            if let Some(text) = det["statusText"].as_str() {
                for line in text.lines() {
                    if line.contains("Tokens:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        for (i, p) in parts.iter().enumerate() {
                            if *p == "in" && i > 0 {
                                input_tokens = parts[i - 1]
                                    .replace(",", "")
                                    .replace("k", "000")
                                    .parse()
                                    .unwrap_or(0);
                            }
                            if *p == "out" && i > 0 {
                                output_tokens = parts[i - 1]
                                    .replace(",", "")
                                    .replace("k", "000")
                                    .parse()
                                    .unwrap_or(0);
                            }
                        }
                    }
                    if line.contains("Queue:") {
                        let queue_part = line.split("Queue:").nth(1).unwrap_or("").trim();
                        if queue_part.starts_with("running")
                            || queue_part.starts_with("thinking")
                            || queue_part.starts_with("streaming")
                        {
                            current_task = Some(queue_part.to_string());
                        }
                    }
                }
            }
        }
        let metrics = AgentMetrics {
            agent_id: agent_id.clone(),
            active: active_count > 0,
            current_model: model,
            thinking_level: None,
            active_session_count: active_count,
            current_task,
            current_tool: None,
            total_tokens,
            input_tokens,
            output_tokens,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            total_cost: 0.0,
            tool_calls: vec![],
            recent_actions: vec![],
            error_count: 0,
            message_count: sessions.len(),
            session_start: None,
            last_activity,
            channel,
        };
        return Ok(metrics);
    }

    // === local mode (original) ===
    let active_set = lsof_active_agents().await;
    let agent_dir = if agent_id.is_empty() {
        "main"
    } else {
        &agent_id
    };
    let active = active_set.contains(agent_dir);

    let mut metrics = AgentMetrics {
        agent_id: agent_id.clone(),
        active,
        current_model: None,
        thinking_level: None,
        active_session_count: 0,
        current_task: None,
        current_tool: None,
        total_tokens: 0,
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        total_cost: 0.0,
        tool_calls: vec![],
        recent_actions: vec![],
        error_count: 0,
        message_count: 0,
        session_start: None,
        last_activity: None,
        channel: None,
    };

    // Read sessions.json to find active sessions
    let sess_path = sessions_json_path(&agent_id);
    let sess_map: serde_json::Map<String, serde_json::Value> =
        match tokio::fs::read_to_string(&sess_path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => return Ok(metrics),
        };

    metrics.active_session_count = sess_map.len();

    // Get model + channel from most recently updated session in sessions.json
    let best_entry = sess_map
        .values()
        .max_by_key(|v| v["updatedAt"].as_u64().unwrap_or(0));
    if let Some(entry) = best_entry {
        metrics.channel = entry["origin"]["surface"].as_str().map(|s| s.to_string());
        // Model is stored directly in sessions.json
        if metrics.current_model.is_none() {
            metrics.current_model = entry["model"].as_str().map(|s| s.to_string());
        }
    }

    // Find the most recently updated session file
    let mut best_session: Option<(String, u64)> = None;
    for val in sess_map.values() {
        if let (Some(file), Some(updated)) =
            (val["sessionFile"].as_str(), val["updatedAt"].as_u64())
        {
            if best_session.as_ref().map_or(true, |(_, t)| updated > *t) {
                best_session = Some((file.to_string(), updated));
            }
        }
    }

    let session_file = match best_session {
        Some((f, _)) => f,
        None => return Ok(metrics),
    };

    // Parse the .jsonl file
    let content = match tokio::fs::read_to_string(&session_file).await {
        Ok(c) => c,
        Err(_) => return Ok(metrics),
    };

    let mut tool_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut last_user_text: Option<String> = None;
    let mut last_tool_name: Option<String> = None;
    let mut last_timestamp: Option<String> = None;
    let mut recent_actions: Vec<RecentAction> = vec![];
    #[allow(unused_assignments)]
    let mut current_msg_timestamp: Option<String> = None;

    for line in content.lines() {
        let val: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = val["type"].as_str().unwrap_or("");
        if let Some(ts) = val["timestamp"].as_str() {
            last_timestamp = Some(ts.to_string());
        }

        match event_type {
            "session" => {
                metrics.session_start = val["timestamp"].as_str().map(|s| s.to_string());
            }
            "model_change" => {
                metrics.current_model = val["modelId"].as_str().map(|s| s.to_string());
            }
            "thinking_level_change" => {
                metrics.thinking_level = val["thinkingLevel"].as_str().map(|s| s.to_string());
            }
            "message" => {
                let msg = &val["message"];
                let role = msg["role"].as_str().unwrap_or("");
                current_msg_timestamp = val["timestamp"].as_str().map(|s| s.to_string());

                if role == "user" {
                    if let Some(content_arr) = msg["content"].as_array() {
                        for item in content_arr {
                            if item["type"].as_str() == Some("text") {
                                if let Some(text) = item["text"].as_str() {
                                    last_user_text = extract_user_message(text);
                                }
                            }
                        }
                    }
                    metrics.message_count += 1;
                } else if role == "assistant" {
                    // Extract usage
                    if let Some(usage) = msg["usage"].as_object() {
                        metrics.input_tokens +=
                            usage.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
                        metrics.output_tokens +=
                            usage.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
                        metrics.cache_read_tokens +=
                            usage.get("cacheRead").and_then(|v| v.as_u64()).unwrap_or(0);
                        metrics.cache_write_tokens += usage
                            .get("cacheWrite")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        metrics.total_tokens += usage
                            .get("totalTokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        if let Some(cost) = usage.get("cost").and_then(|c| c["total"].as_f64()) {
                            metrics.total_cost += cost;
                        }
                    }

                    // Extract tool calls and text actions
                    if let Some(content_arr) = msg["content"].as_array() {
                        for item in content_arr {
                            match item["type"].as_str() {
                                Some("toolCall") => {
                                    if let Some(name) = item["name"].as_str() {
                                        *tool_counts.entry(name.to_string()).or_insert(0) += 1;
                                        last_tool_name = Some(name.to_string());

                                        let detail = item["input"]
                                            .as_object()
                                            .map(|obj| {
                                                let mut parts: Vec<String> = vec![];
                                                for (k, v) in obj.iter() {
                                                    let val_str = match v.as_str() {
                                                        Some(s) => {
                                                            if s.len() > 300 {
                                                                let mut end = 300;
                                                                while end > 0
                                                                    && !s.is_char_boundary(end)
                                                                {
                                                                    end -= 1;
                                                                }
                                                                format!("{}...", &s[..end])
                                                            } else {
                                                                s.to_string()
                                                            }
                                                        }
                                                        None => {
                                                            let j = v.to_string();
                                                            if j.len() > 100 {
                                                                let mut end = 100;
                                                                while end > 0
                                                                    && !j.is_char_boundary(end)
                                                                {
                                                                    end -= 1;
                                                                }
                                                                format!("{}...", &j[..end])
                                                            } else {
                                                                j
                                                            }
                                                        }
                                                    };
                                                    parts.push(format!("{}: {}", k, val_str));
                                                }
                                                parts.join("\n")
                                            })
                                            .filter(|s| !s.is_empty());
                                        recent_actions.push(RecentAction {
                                            action_type: "tool".to_string(),
                                            summary: name.to_string(),
                                            detail,
                                            timestamp: current_msg_timestamp.clone(),
                                        });
                                    }
                                }
                                Some("text") => {
                                    if let Some(text) = item["text"].as_str() {
                                        let trimmed = text.trim();
                                        if !trimmed.is_empty() {
                                            let summary = if trimmed.len() > 60 {
                                                let mut end = 60;
                                                while end > 0 && !trimmed.is_char_boundary(end) {
                                                    end -= 1;
                                                }
                                                format!("{}...", &trimmed[..end])
                                            } else {
                                                trimmed.to_string()
                                            };
                                            let detail = if trimmed.len() > 60 {
                                                let full = if trimmed.len() > 500 {
                                                    let mut end = 500;
                                                    while end > 0 && !trimmed.is_char_boundary(end)
                                                    {
                                                        end -= 1;
                                                    }
                                                    format!("{}...", &trimmed[..end])
                                                } else {
                                                    trimmed.to_string()
                                                };
                                                Some(full)
                                            } else {
                                                None
                                            };
                                            recent_actions.push(RecentAction {
                                                action_type: "text".to_string(),
                                                summary,
                                                detail,
                                                timestamp: current_msg_timestamp.clone(),
                                            });
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    metrics.message_count += 1;
                }
            }
            "custom"
                if val["customType"]
                    .as_str()
                    .is_some_and(|t| t.contains("error")) =>
            {
                metrics.error_count += 1;
            }
            _ => {}
        }
    }

    metrics.current_task = last_user_text;
    metrics.current_tool = last_tool_name;
    metrics.last_activity = last_timestamp;

    // Keep only the last 3 actions (most recent first)
    let len = recent_actions.len();
    if len > 3 {
        metrics.recent_actions = recent_actions[len - 3..].to_vec();
    } else {
        metrics.recent_actions = recent_actions;
    }
    metrics.recent_actions.reverse();

    // Sort tool calls by count desc
    let mut tool_vec: Vec<ToolCallStat> = tool_counts
        .into_iter()
        .map(|(name, count)| ToolCallStat { name, count })
        .collect();
    tool_vec.sort_by_key(|t| std::cmp::Reverse(t.count));
    metrics.tool_calls = tool_vec;

    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_str_long() {
        let result = truncate_str("hello world", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn truncate_str_multibyte() {
        let result = truncate_str("你好世界test", 6);
        assert_eq!(result, "你好...");
    }

    #[test]
    fn extract_user_message_with_message_id() {
        let input = "Conversation info\n[message_id: abc]\nAlice: hello";
        let result = extract_user_message(input);
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn extract_user_message_simple_timestamp() {
        let input = "[2026-01-01T00:00:00Z] fix the bug";
        let result = extract_user_message(input);
        assert_eq!(result, Some("fix the bug".to_string()));
    }

    #[test]
    fn extract_user_message_queued() {
        let input = "[Queued messages]\n---\nQueued #1\n[message_id: a]\nBob: first\n---\nQueued #2\n[message_id: b]\nAlice: second";
        let result = extract_user_message(input);
        assert_eq!(result, Some("second".to_string()));
    }

    #[test]
    fn extract_user_message_fallback() {
        let input = "just a plain message";
        let result = extract_user_message(input);
        assert_eq!(result, Some("just a plain message".to_string()));
    }
}
