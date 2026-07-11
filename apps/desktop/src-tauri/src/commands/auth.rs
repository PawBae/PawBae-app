//! GitHub OAuth 桌面流的 loopback 回调半边（B 线 W3）。
//!
//! 流程：前端先 invoke `await_oauth_callback`（本命令立刻 bind
//! 127.0.0.1:port 再阻塞等请求），再用系统浏览器打开 Supabase 授权 URL
//! （redirectTo 指向本端口）；浏览器跳回来后本命令把查询串原样交还前端，
//! 由 supabase-js 的 PKCE `exchangeCodeForSession` 完成换票 —— 授权码
//! 即使被本机其他进程截获，没有 code_verifier 也无法使用。
//!
//! 安全边界：只 bind 回环地址；只认 CALLBACK_PATH；单次请求即关闭监听；
//! 超时自动放弃（用户关掉浏览器不至于让监听器悬挂）。

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

/// 与 Supabase Auth 后台 Redirect URLs 白名单里登记的路径保持一致。
pub const CALLBACK_PATH: &str = "/pawbae-auth";

/// 从 HTTP 请求行（`GET /pawbae-auth?code=... HTTP/1.1`）提取查询串。
/// 路径不匹配（favicon 等杂请求）返回 None。
fn parse_callback_query(request_line: &str) -> Option<String> {
    let target = request_line.split_whitespace().nth(1)?;
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };
    if path != CALLBACK_PATH {
        return None;
    }
    Some(query.to_string())
}

fn respond(stream: &mut std::net::TcpStream, body: &str) {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

const DONE_PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>PawBae</title><body style="font-family:system-ui;display:flex;align-items:center;justify-content:center;height:90vh;color:#333"><div style="text-align:center"><div style="font-size:40px">🐾</div><p>登录完成，请回到 PawBae。<br>Signed in — you can return to PawBae now.</p></div></body>"#;

/// 一次性 loopback 监听：等 OAuth 回调，返回回调 URL 的查询串。
/// 阻塞版实现跑在 blocking 线程池；`timeout_secs` 兜底（默认 180s）。
#[tauri::command]
pub async fn await_oauth_callback(port: u16, timeout_secs: Option<u64>) -> Result<String, String> {
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(180).clamp(10, 600));
    tauri::async_runtime::spawn_blocking(move || {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .map_err(|e| format!("oauth callback port {port} unavailable: {e}"))?;
        // accept 没有原生超时：非阻塞 + 短轮询近似（回环上开销可忽略）
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("listener setup failed: {e}"))?;

        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .ok_or_else(|| "oauth callback timed out".to_string())?;
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    // 部分平台 accept 出的 socket 继承非阻塞标志，读之前显式复位
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut line = String::new();
                    {
                        let mut reader = BufReader::new(&stream);
                        if reader.read_line(&mut line).is_err() {
                            continue;
                        }
                    }
                    match parse_callback_query(&line) {
                        Some(query) => {
                            respond(&mut stream, DONE_PAGE);
                            return Ok(query);
                        }
                        None => {
                            // 杂请求（favicon 等）：应答后继续等真正的回调
                            respond(&mut stream, "");
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if remaining.is_zero() {
                        return Err("oauth callback timed out".to_string());
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
                Err(e) => return Err(format!("oauth callback accept failed: {e}")),
            }
        }
    })
    .await
    .map_err(|e| format!("oauth callback task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_query_from_callback_path() {
        let q = parse_callback_query("GET /pawbae-auth?code=abc123&state=xyz HTTP/1.1");
        assert_eq!(q.as_deref(), Some("code=abc123&state=xyz"));
    }

    #[test]
    fn rejects_other_paths() {
        assert_eq!(parse_callback_query("GET /favicon.ico HTTP/1.1"), None);
        assert_eq!(
            parse_callback_query("GET /pawbae-auth-evil?code=x HTTP/1.1"),
            None
        );
    }

    #[test]
    fn empty_query_is_ok() {
        assert_eq!(
            parse_callback_query("GET /pawbae-auth HTTP/1.1").as_deref(),
            Some("")
        );
    }

    #[test]
    fn malformed_request_line_is_none() {
        assert_eq!(parse_callback_query("garbage"), None);
    }
}
