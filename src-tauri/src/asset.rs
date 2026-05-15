//! `localasset:` / `codexpet:` URI scheme handler and asset response helpers.

use percent_encoding::percent_decode_str;
use tauri::Manager;

use crate::commands::codex_pet::codex_pets_dir;

fn asset_mime_for_path(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".mp4") {
        "video/mp4"
    } else if lower.ends_with(".mov") {
        "video/quicktime"
    } else {
        "application/octet-stream"
    }
}

fn build_asset_response(
    req: &tauri::http::Request<Vec<u8>>,
    path: &str,
    file_path: &std::path::Path,
    add_cors: bool,
    log_label: &str,
) -> tauri::http::Response<Vec<u8>> {
    match std::fs::read(file_path) {
        Ok(data) => {
            let mime = asset_mime_for_path(path);
            let total_len = data.len();
            let mut status = 200;
            let mut body = data;
            let mut content_range: Option<String> = None;

            // Serve byte ranges for media files so WKWebView/Safari can stream
            // video containers like HEVC .mov/.mp4 reliably.
            if total_len > 0 {
                if let Some(range_header) = req
                    .headers()
                    .get("Range")
                    .or_else(|| req.headers().get("range"))
                {
                    if let Ok(range) = range_header.to_str() {
                        if let Some(spec) = range.strip_prefix("bytes=") {
                            let mut parts = spec.splitn(2, '-');
                            let start_part = parts.next().unwrap_or("");
                            let end_part = parts.next().unwrap_or("");
                            let parsed = if start_part.is_empty() {
                                end_part.parse::<usize>().ok().map(|suffix_len| {
                                    let suffix_len = suffix_len.min(total_len);
                                    let start = total_len.saturating_sub(suffix_len);
                                    (start, total_len.saturating_sub(1))
                                })
                            } else if let Ok(start) = start_part.parse::<usize>() {
                                let end = if end_part.is_empty() {
                                    total_len.saturating_sub(1)
                                } else {
                                    end_part
                                        .parse::<usize>()
                                        .unwrap_or(total_len.saturating_sub(1))
                                        .min(total_len.saturating_sub(1))
                                };
                                if start < total_len && start <= end {
                                    Some((start, end))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            if let Some((start, end)) = parsed {
                                body = body[start..=end].to_vec();
                                status = 206;
                                content_range =
                                    Some(format!("bytes {}-{}/{}", start, end, total_len));
                            }
                        }
                    }
                }
            }

            let mut resp = tauri::http::Response::builder()
                .status(status)
                .header("Content-Type", mime)
                .header("Content-Length", body.len().to_string())
                .header("Accept-Ranges", "bytes");
            if let Some(content_range) = content_range {
                resp = resp.header("Content-Range", content_range);
            }
            if add_cors {
                resp = resp.header("Access-Control-Allow-Origin", "*");
            }
            resp.body(body).unwrap()
        }
        Err(e) => {
            log::warn!("[{}] 404: {} err={}", log_label, file_path.display(), e);
            tauri::http::Response::builder()
                .status(404)
                .body(Vec::new())
                .unwrap()
        }
    }
}

pub(crate) fn register<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .register_uri_scheme_protocol("localasset", |ctx, req| {
            let raw_path = req.uri().path();
            let path = percent_decode_str(raw_path).decode_utf8_lossy();
            let resource_dir = ctx.app_handle().path().resource_dir().unwrap_or_default();
            let file_path = resource_dir
                .join("assets")
                .join("builtin")
                .join(path.trim_start_matches('/'));
            log::info!(
                "[localasset] request={} resolved={}",
                raw_path,
                file_path.display()
            );
            build_asset_response(
                &req,
                path.as_ref(),
                &file_path,
                cfg!(target_os = "windows"),
                "localasset",
            )
        })
        .register_uri_scheme_protocol("codexpet", |_ctx, req| {
            // Custom codex pets the user dropped into `~/.codex/pets`.
            // Avatars are loaded through this protocol so the picker can
            // display sprites that live outside the bundled assets dir.
            let raw_path = req.uri().path();
            let path = percent_decode_str(raw_path).decode_utf8_lossy();
            let root = codex_pets_dir().unwrap_or_default();
            let file_path = root.join(path.trim_start_matches('/'));
            build_asset_response(
                &req,
                path.as_ref(),
                &file_path,
                cfg!(target_os = "windows"),
                "codexpet",
            )
        })
}
