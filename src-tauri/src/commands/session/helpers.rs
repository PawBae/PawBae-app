//! Shared helper functions for session submodules.

/// Extract the actual user message from raw text, stripping all system/channel noise.
pub(super) fn clean_user_message(text: &str) -> String {
    // Skip system startup messages entirely
    if text.starts_with("A new session was started") {
        return String::new();
    }

    let mut s = text.to_string();

    // Queued messages: extract the last actual message from the queue
    if s.starts_with("[Queued messages") || s.starts_with("Queued #") {
        // Find the last "Queued #N" block and process it
        if let Some(idx) = s.rfind("Queued #") {
            s = s[idx..].to_string();
            // Skip the "Queued #N" line
            if let Some(nl) = s.find('\n') {
                s = s[nl + 1..].to_string();
            }
        }
        // Now s contains the last queued message content, fall through to normal cleaning
    }

    // Strip channel metadata blocks and extract actual message.
    // Formats:
    //   1) With [message_id:...] line -> actual message is after "Name: msg"
    //   2) Without [message_id:] but has ``` blocks -> actual message is after last ```
    if s.contains("(untrusted metadata)")
        || s.contains("Conversation info (untrusted metadata)")
        || s.contains("[message_id:")
    {
        if let Some(idx) = s.rfind("[message_id:") {
            if let Some(nl) = s[idx..].find('\n') {
                let after = s[idx + nl + 1..].trim();
                // Format: "Name: actual message" or just "actual message"
                if let Some(colon) = after.find(": ") {
                    let name_part = &after[..colon];
                    if name_part.len() < 40 && !name_part.contains('\n') {
                        s = after[colon + 2..].to_string();
                    } else {
                        s = after.to_string();
                    }
                } else {
                    s = after.to_string();
                }
            }
        } else {
            // Has metadata but no [message_id:], extract after last ``` block
            if let Some(idx) = s.rfind("```\n") {
                s = s[idx + 4..].trim().to_string();
            }
        }
    }

    // Strip [media attached: ...] prefix - keep text after it if any
    if s.starts_with("[media attached:") {
        if let Some(end) = s.find("]\n") {
            s = s[end + 2..].to_string();
        } else if let Some(end) = s.find(']') {
            s = s[end + 1..].trim().to_string();
        }
    }

    // Strip system prompt prefix
    if let Some(idx) = s.find("\n\nHuman: ") {
        s = s[idx + 9..].to_string();
    }

    // Strip all [[...]] markers anywhere in text (e.g. [[reply_to_current]])
    while let Some(start) = s.find("[[") {
        if let Some(end) = s[start..].find("]]") {
            s = format!("{}{}", &s[..start], &s[start + end + 2..]);
        } else {
            break;
        }
    }

    // Strip timestamp prefix like "[Mon 2026-03-16 01:58 GMT+8] "
    {
        let trimmed = s.trim_start();
        if trimmed.starts_with('[') {
            if let Some(end) = trimmed.find("] ") {
                let bracket_content = &trimmed[1..end];
                // Check if it looks like a timestamp (contains digits and GMT/UTC or day names)
                if bracket_content.len() < 50
                    && (bracket_content.contains("GMT")
                        || bracket_content.contains("UTC")
                        || bracket_content.contains("Mon")
                        || bracket_content.contains("Tue")
                        || bracket_content.contains("Wed")
                        || bracket_content.contains("Thu")
                        || bracket_content.contains("Fri")
                        || bracket_content.contains("Sat")
                        || bracket_content.contains("Sun"))
                {
                    s = trimmed[end + 2..].to_string();
                }
            }
        }
    }

    // Strip "Current time: ..." lines and everything after
    if let Some(idx) = s.find("\nCurrent time:") {
        s = s[..idx].to_string();
    }
    if let Some(idx) = s.find("Current time:") {
        if idx == 0 {
            return String::new();
        }
        s = s[..idx].to_string();
    }

    // Strip cron prefix like "[cron:xxx ...]"
    if s.starts_with("[cron:") {
        if let Some(end) = s.find("] ") {
            s = s[end + 2..].to_string();
        }
    }

    // Strip "Return your summary as plain text..." suffix
    if let Some(idx) = s.find("\nReturn your summary") {
        s = s[..idx].to_string();
    }
    if let Some(idx) = s.find("Return your summary") {
        if idx == 0 {
            return String::new();
        }
    }

    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_user_message_strips_system_startup() {
        let input = "A new session was started for agent X";
        assert_eq!(clean_user_message(input), "");
    }

    #[test]
    fn clean_user_message_strips_message_id_wrapper() {
        let input =
            "Conversation info (untrusted metadata)\n[message_id: abc123]\nAlice: hello world";
        assert_eq!(clean_user_message(input), "hello world");
    }

    #[test]
    fn clean_user_message_strips_brackets() {
        let input = "do something [[reply_to_current]] please";
        assert_eq!(clean_user_message(input), "do something  please");
    }

    #[test]
    fn clean_user_message_strips_timestamp_prefix() {
        let input = "[Mon 2026-03-16 01:58 GMT+8] fix the bug";
        assert_eq!(clean_user_message(input), "fix the bug");
    }

    #[test]
    fn clean_user_message_strips_cron_prefix() {
        let input = "[cron:daily-check] run the report";
        assert_eq!(clean_user_message(input), "run the report");
    }

    #[test]
    fn clean_user_message_plain_text_passthrough() {
        let input = "refactor the auth module";
        assert_eq!(clean_user_message(input), "refactor the auth module");
    }

    #[test]
    fn strip_brackets_removes_all_markers() {
        assert_eq!(strip_brackets("a [[b]] c [[d]] e"), "a  c  e");
    }

    #[test]
    fn strip_brackets_no_markers() {
        assert_eq!(strip_brackets("plain text"), "plain text");
    }

    #[test]
    fn strip_brackets_unclosed() {
        assert_eq!(strip_brackets("a [[unclosed"), "a [[unclosed");
    }

    #[test]
    fn extract_last_messages_finds_both() {
        let content = r#"{"type":"message","message":{"role":"user","content":[{"type":"text","text":"fix the bug"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"I fixed it."}]}}"#;
        let (user, assistant) = extract_last_messages(content);
        assert_eq!(user.as_deref(), Some("fix the bug"));
        assert_eq!(assistant.as_deref(), Some("I fixed it."));
    }

    #[test]
    fn extract_last_messages_skips_non_message() {
        let content = r#"{"type":"session","timestamp":"2026-01-01"}
{"type":"message","message":{"role":"user","content":[{"type":"text","text":"hello"}]}}"#;
        let (user, assistant) = extract_last_messages(content);
        assert_eq!(user.as_deref(), Some("hello"));
        assert!(assistant.is_none());
    }
}

/// Strip all [[...]] markers from text.
pub(super) fn strip_brackets(text: &str) -> String {
    let mut s = text.to_string();
    while let Some(start) = s.find("[[") {
        if let Some(end) = s[start..].find("]]") {
            s = format!("{}{}", &s[..start], &s[start + end + 2..]);
        } else {
            break;
        }
    }
    s.trim().to_string()
}

/// Extract last user + assistant message from a .jsonl session file (reads from end).
pub(super) fn extract_last_messages(content: &str) -> (Option<String>, Option<String>) {
    let mut last_user: Option<String> = None;
    let mut last_assistant: Option<String> = None;
    for line in content.lines() {
        let val: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if val["type"].as_str() != Some("message") {
            continue;
        }
        let msg = &val["message"];
        let role = msg["role"].as_str().unwrap_or("");
        let text = if let Some(arr) = msg["content"].as_array() {
            arr.iter()
                .filter(|i| i["type"].as_str() == Some("text"))
                .filter_map(|i| i["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else if let Some(s) = msg["content"].as_str() {
            s.to_string()
        } else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        match role {
            "user" => {
                let cleaned = clean_user_message(&text);
                if cleaned.is_empty() {
                    continue;
                }
                let truncated = if cleaned.chars().count() > 120 {
                    let s: String = cleaned.chars().take(120).collect();
                    format!("{}...", s)
                } else {
                    cleaned
                };
                last_user = Some(truncated);
            }
            "assistant" => {
                let cleaned = strip_brackets(&text);
                if cleaned.is_empty() {
                    continue;
                }
                let truncated = if cleaned.chars().count() > 120 {
                    let s: String = cleaned.chars().take(120).collect();
                    format!("{}...", s)
                } else {
                    cleaned
                };
                last_assistant = Some(truncated);
            }
            _ => {}
        }
    }
    (last_user, last_assistant)
}
