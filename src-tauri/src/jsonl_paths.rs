//! JSONL session-file path resolution for Claude Code and Codex.

use std::path::PathBuf;

use crate::cursor::claude_session_file_path;

fn collect_jsonl_files_recursive(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
    }
    out
}

pub(crate) fn collect_claude_project_jsonl_files() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let claude_projects = home.join(".claude").join("projects");
    if !claude_projects.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Ok(project_dirs) = std::fs::read_dir(claude_projects) {
        for project_entry in project_dirs.flatten() {
            let project_dir = project_entry.path();
            if !project_dir.is_dir() {
                continue;
            }
            if let Ok(files) = std::fs::read_dir(project_dir) {
                for file_entry in files.flatten() {
                    let path = file_entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        out.push(path);
                    }
                }
            }
        }
    }
    out
}

pub(crate) fn collect_codex_session_jsonl_files() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let codex_sessions = home.join(".Codex").join("sessions");
    collect_jsonl_files_recursive(&codex_sessions)
}

fn find_claude_session_file(session_id: &str) -> Option<PathBuf> {
    let target = format!("{}.jsonl", session_id);
    collect_claude_project_jsonl_files()
        .into_iter()
        .find(|path| path.file_name().and_then(|n| n.to_str()) == Some(target.as_str()))
}

fn find_codex_session_file(session_id: &str) -> Option<PathBuf> {
    // Codex stores sessions as:
    //   ~/.Codex/sessions/YYYY/MM/DD/rollout-<timestamp>-<session_id>.jsonl
    // so we cannot derive the path from cwd; we must scan for a filename
    // containing the session id.
    for path in collect_codex_session_jsonl_files() {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.ends_with(".jsonl") && name.contains(session_id) {
            return Some(path);
        }
    }
    None
}

pub(crate) fn resolve_session_jsonl_path(session_id: &str, cwd: Option<&str>) -> Option<PathBuf> {
    // Prefer Claude's deterministic path when cwd is known, then fall back to
    // directory scans. This keeps existing behavior fast while adding Codex
    // compatibility.
    if let Some(cwd_str) = cwd {
        if !cwd_str.is_empty() {
            let by_cwd = claude_session_file_path(session_id, cwd_str);
            if by_cwd.exists() {
                return Some(by_cwd);
            }
        }
    }
    find_claude_session_file(session_id).or_else(|| find_codex_session_file(session_id))
}
