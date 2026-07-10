//! 崩溃上报（最小自建，B 线 W2 P1）：Rust panic hook + webview 全局错误
//! → 本地脱敏崩溃日志（app_data_dir/crashes/）。不接外部服务；下一次启动
//! 时前端通过 `take_unseen_crashes` 读未读计数，经 utils/telemetry.ts 的
//! opt-in 门发匿名聚合事件（只有计数，无堆栈、无路径）。
//!
//! 隐私红线（交接文档 W2）：写盘内容一律过 `sanitize`，堆栈不带用户路径。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use tauri::Manager;

/// panic hook 里拿不到 AppHandle，启动时解析一次 crash 目录。
static CRASH_DIR: OnceLock<PathBuf> = OnceLock::new();
/// webview 错误单次运行上限 —— 防错误风暴刷盘/刷 IPC（前端另有去重）。
static WEBVIEW_REPORTS: AtomicU32 = AtomicU32::new(0);
const WEBVIEW_REPORT_CAP: u32 = 30;
/// crashes 目录里最多保留的报告数，超出删最旧。
const KEEP_REPORTS: usize = 20;
/// 上次消费时间戳的 marker 文件（内容为空，只用 mtime）。
const SEEN_MARKER: &str = ".last-seen";
/// 文件名 kind 白名单 —— kind 参与文件名，绝不能直接用调用方字符串。
const KINDS: [&str; 4] = [
    "rust-panic",
    "webview-error",
    "webview-rejection",
    "webview-other",
];

fn sanitize(text: &str) -> String {
    let mut out = text.to_string();
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();
        // Windows 日志里路径可能以转义反斜杠出现，先替换转义形式再替换原形。
        let escaped = home_str.replace('\\', "\\\\");
        if escaped != home_str {
            out = out.replace(&escaped, "~");
        }
        out = out.replace(&home_str, "~");
    }
    out
}

/// 只保留最新 KEEP_REPORTS 份报告。按文件名排序即按时间排序
/// （crash-<YYYYMMDD-HHMMSS.mmm>-<kind>.log，字典序 = 时间序）。
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("crash-") && n.ends_with(".log"))
        .collect();
    if names.len() <= KEEP_REPORTS {
        return;
    }
    names.sort();
    let excess = names.len() - KEEP_REPORTS;
    for name in names.into_iter().take(excess) {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

fn write_report(kind: &'static str, body: &str) {
    let Some(dir) = CRASH_DIR.get() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S%.3f");
    let path = dir.join(format!("crash-{ts}-{kind}.log"));
    let _ = std::fs::write(&path, sanitize(body));
    prune(dir);
}

/// 安装 Rust panic hook。在 setup（日志插件就绪后）调用一次；
/// 保留原 hook（默认 stderr 输出），本 hook 只负责落盘。
pub fn install(app: &tauri::AppHandle) {
    let dir = match app.path().app_data_dir() {
        Ok(d) => d.join("crashes"),
        Err(e) => {
            log::warn!("[crash] cannot resolve app data dir, crash reports disabled: {e}");
            return;
        }
    };
    if CRASH_DIR.set(dir).is_err() {
        return; // 已安装过（理论上 setup 只跑一次）
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let thread = std::thread::current();
        let backtrace = std::backtrace::Backtrace::force_capture();
        let body = format!(
            "version: {}\nos: {} {}\nthread: {}\nlocation: {}\nmessage: {}\n\nbacktrace:\n{}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            thread.name().unwrap_or("<unnamed>"),
            location,
            message,
            backtrace,
        );
        write_report("rust-panic", &body);
        previous(info);
    }));
    log::info!("[crash] panic hook installed");
}

/// webview 全局错误落盘（utils/crash-report.ts 调用）。
/// kind 只接受 "error"/"rejection"，其余归入 webview-other。
#[tauri::command]
pub fn report_frontend_error(kind: String, message: String, stack: Option<String>) {
    if WEBVIEW_REPORTS.fetch_add(1, Ordering::Relaxed) >= WEBVIEW_REPORT_CAP {
        return;
    }
    let kind: &'static str = match kind.as_str() {
        "error" => "webview-error",
        "rejection" => "webview-rejection",
        _ => "webview-other",
    };
    let body = format!(
        "version: {}\nos: {} {}\nmessage: {}\n\nstack:\n{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        message,
        stack.unwrap_or_default(),
    );
    write_report(kind, &body);
}

/// 返回自上次调用以来的新崩溃报告计数（按 kind 分组），并推进 marker。
/// 前端在启动时调用一次，若 total > 0 经 telemetry opt-in 门发匿名事件。
#[tauri::command]
pub fn take_unseen_crashes() -> serde_json::Value {
    let Some(dir) = CRASH_DIR.get() else {
        return serde_json::json!({ "total": 0, "kinds": {} });
    };
    let _ = std::fs::create_dir_all(dir);
    let marker = dir.join(SEEN_MARKER);
    let last_seen = std::fs::metadata(&marker).and_then(|m| m.modified()).ok();

    let mut total = 0u32;
    let mut kinds: std::collections::HashMap<&'static str, u32> = std::collections::HashMap::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("crash-") || !name.ends_with(".log") {
                continue;
            }
            if let (Some(seen), Ok(meta)) = (last_seen, entry.metadata()) {
                if meta.modified().map(|m| m <= seen).unwrap_or(false) {
                    continue;
                }
            }
            let kind = KINDS
                .iter()
                .find(|k| name.ends_with(&format!("-{k}.log")))
                .copied()
                .unwrap_or("webview-other");
            total += 1;
            *kinds.entry(kind).or_insert(0) += 1;
        }
    }
    // 推进 marker（mtime = now）；写失败只影响下次去重，不影响本次返回。
    let _ = std::fs::write(&marker, b"");
    serde_json::json!({ "total": total, "kinds": kinds })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_home_dir() {
        let home = dirs::home_dir().expect("home dir");
        let raw = format!(
            "thread panicked at {}/Documents/secret/app.rs:1",
            home.display()
        );
        let clean = sanitize(&raw);
        assert!(!clean.contains(&home.to_string_lossy().to_string()));
        assert!(clean.contains("~/Documents/secret/app.rs"));
    }

    #[test]
    fn prune_keeps_newest_reports() {
        let dir = std::env::temp_dir().join(format!("pawbae-crash-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..(KEEP_REPORTS + 5) {
            let name = format!("crash-20260101-0000{i:02}.000-rust-panic.log");
            std::fs::write(dir.join(name), "x").unwrap();
        }
        prune(&dir);
        let left = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(left, KEEP_REPORTS);
        // 幸存的应是字典序最大（=最新）的那批
        let survivors: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(survivors
            .iter()
            .all(|n| n.as_str() > "crash-20260101-000004.000-rust-panic.log"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
