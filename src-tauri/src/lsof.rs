//! lsof-based active-agent / active-jsonl detection (Unix); modtime fallback on Windows.

use std::path::PathBuf;
use std::time::SystemTime;

use crate::home_dir_string;

/// Returns the full set of open .jsonl file paths across all agents.
/// On macOS/Linux: uses `lsof +D` to detect open files.
/// On Windows: falls back to checking file modification time (recent = active).
pub(crate) async fn lsof_open_jsonl_paths() -> std::collections::HashSet<String> {
    #[cfg(unix)]
    {
        let home = home_dir_string();
        let agents_dir = format!("{}/.openclaw/agents", home);
        let lsof_bin = if std::path::Path::new("/usr/sbin/lsof").exists() { "/usr/sbin/lsof" } else { "lsof" };
        let Ok(output) = tokio::process::Command::new(lsof_bin)
            .args(["+D", &agents_dir])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .await
        else { return std::collections::HashSet::new() };
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines()
            .filter(|l| l.contains(".jsonl"))
            .filter_map(|l| l.split_whitespace().last().map(|s| s.to_string()))
            .collect()
    }
    #[cfg(windows)]
    {
        // Windows fallback: find .jsonl files modified in the last 5 seconds
        // (indicates an active agent writing to them)
        let home = home_dir_string();
        let agents_dir = PathBuf::from(&home).join(".openclaw").join("agents");
        let mut result = std::collections::HashSet::new();
        let now = SystemTime::now();
        if let Ok(agents) = std::fs::read_dir(&agents_dir) {
            for agent_entry in agents.flatten() {
                let sessions_dir = agent_entry.path().join("sessions");
                if let Ok(files) = std::fs::read_dir(&sessions_dir) {
                    for file_entry in files.flatten() {
                        let path = file_entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                            if let Ok(meta) = path.metadata() {
                                if let Ok(modified) = meta.modified() {
                                    if now.duration_since(modified).unwrap_or_default().as_secs() < 5 {
                                        result.insert(path.to_string_lossy().to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }
}
/// Single `lsof +D` over the entire agents dir → set of active agent directory names.
/// A .jsonl being held open by a process = that agent is working.
/// On Windows: uses file modification time heuristic instead of lsof.
pub(crate) async fn lsof_active_agents() -> std::collections::HashSet<String> {
    #[cfg(unix)]
    {
        let home = home_dir_string();
        let agents_dir = format!("{}/.openclaw/agents", home);
        let mut active = std::collections::HashSet::new();

        let lsof_bin = if std::path::Path::new("/usr/sbin/lsof").exists() {
            "/usr/sbin/lsof"
        } else {
            "lsof"
        };

        let Ok(output) = tokio::process::Command::new(lsof_bin)
            .args(["+D", &agents_dir])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .await
        else {
            return active;
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let prefix = ".openclaw/agents/";
        for line in stdout.lines() {
            if !line.contains(".jsonl") {
                continue;
            }
            if let Some(idx) = line.find(prefix) {
                let rest = &line[idx + prefix.len()..];
                if let Some(slash) = rest.find('/') {
                    active.insert(rest[..slash].to_string());
                }
            }
        }
        active
    }
    #[cfg(windows)]
    {
        // Windows: find agent directories that have recently modified .jsonl files
        let home = home_dir_string();
        let agents_dir = PathBuf::from(&home).join(".openclaw").join("agents");
        let mut active = std::collections::HashSet::new();
        let now = SystemTime::now();
        if let Ok(agents) = std::fs::read_dir(&agents_dir) {
            for agent_entry in agents.flatten() {
                let agent_name = agent_entry.file_name().to_string_lossy().to_string();
                let sessions_dir = agent_entry.path().join("sessions");
                if let Ok(files) = std::fs::read_dir(&sessions_dir) {
                    for file_entry in files.flatten() {
                        let path = file_entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                            if let Ok(meta) = path.metadata() {
                                if let Ok(modified) = meta.modified() {
                                    if now.duration_since(modified).unwrap_or_default().as_secs() < 5 {
                                        active.insert(agent_name.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        active
    }
}
