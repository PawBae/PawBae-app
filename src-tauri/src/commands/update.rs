//! Tauri updater commands: HTTP manifest check + DMG/EXE download and install.

use std::time::SystemTime;
use tauri::Emitter;

#[cfg(target_os = "windows")]
use crate::platform::windows::hide_window_cmd;

/// Check for updates by fetching the version manifest from the official website.
/// The manifest is a static JSON file hosted on Vercel at /update/latest.json,
/// which is manually updated on each release — giving us full control over
/// when users see an update prompt (independent of GitHub Releases).
///
/// Expected manifest format:
///   {
///     "version": "1.6.0",
///     "notes": "...",
///     "platforms": {
///       "macos":   { "url": "https://github.com/.../PawBae_0.1.0_aarch64.dmg" },
///       "windows": { "url": "https://github.com/.../PawBae_0.1.0_x64-setup.exe" }
///     }
///   }
///
/// Legacy format (single "url" field) is still supported for backward compatibility.
fn normalize_lang_tag(lang: &str) -> String {
    lang.trim().to_lowercase().replace('_', "-")
}

fn pick_localized_notes(notes_i18n: &serde_json::Value, lang: Option<&str>) -> Option<String> {
    let obj = notes_i18n.as_object()?;
    let mut keys: Vec<String> = Vec::new();
    if let Some(raw) = lang {
        let normalized = normalize_lang_tag(raw);
        if !normalized.is_empty() {
            keys.push(normalized.clone());
            if let Some((prefix, _)) = normalized.split_once('-') {
                if !prefix.is_empty() {
                    keys.push(prefix.to_string());
                }
            }
        }
    }
    keys.push("en".to_string());
    keys.push("zh".to_string());
    for key in keys {
        if let Some(value) = obj.get(&key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    for value in obj.values() {
        if let Some(text) = value.as_str() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[tauri::command]
pub async fn check_for_update(
    app: tauri::AppHandle,
    lang: Option<String>,
) -> Result<serde_json::Value, String> {
    let current = app.config().version.clone().unwrap_or_default();

    let update_url = if cfg!(debug_assertions) {
        "http://[::1]:4321/update/latest.json"
    } else {
        "https://pawbae.ai/update/latest.json"
    };
    log::info!("[update] checking {} (current={})", update_url, current);
    let mut client_builder = reqwest::Client::builder().user_agent("pawbae");
    if cfg!(debug_assertions) {
        client_builder = client_builder.no_proxy();
    }
    let client = client_builder
        .build()
        .map_err(|e| format!("client build error: {e}"))?;
    let resp = client.get(update_url).send().await.map_err(|e| {
        log::warn!("[update] fetch error: {e}");
        format!("fetch error: {e}")
    })?;
    if !resp.status().is_success() {
        let msg = format!("update check failed: HTTP {}", resp.status());
        log::warn!("[update] {msg}");
        return Err(msg);
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| {
        log::warn!("[update] json parse error: {e}");
        format!("json parse error: {e}")
    })?;

    // Per-platform update: each platform has its own version and url under
    // json["platforms"]["<platform>"]["version"] and ["url"].
    // Falls back to legacy top-level json["version"] / json["url"] for compatibility.
    #[cfg(windows)]
    let platform_key = "windows";
    #[cfg(target_os = "macos")]
    let platform_key = "macos";
    #[cfg(not(any(windows, target_os = "macos")))]
    let platform_key = "linux";

    let platform = &json["platforms"][platform_key];
    let latest = platform["version"]
        .as_str()
        .or_else(|| json["version"].as_str())
        .unwrap_or("");
    let url = platform["url"]
        .as_str()
        .or_else(|| json["url"].as_str())
        .unwrap_or("");
    let notes = pick_localized_notes(&platform["notes_i18n"], lang.as_deref())
        .or_else(|| pick_localized_notes(&json["notes_i18n"], lang.as_deref()))
        .or_else(|| {
            platform["notes"]
                .as_str()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            json["notes"]
                .as_str()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();
    let has_update = version_cmp(latest, &current);
    log::info!(
        "[update] platform={} latest={} current={} hasUpdate={}",
        platform_key,
        latest,
        current,
        has_update
    );

    // Pass through any `ui` block as-is. This lets us push UI-level
    // config (e.g. the codex-pets.net URL inside the Create section)
    // without shipping a new app build, while keeping the namespace
    // separated from the platform-specific update metadata above.
    let ui = json.get("ui").cloned().unwrap_or(serde_json::Value::Null);

    Ok(serde_json::json!({
        "current": current,
        "latest": latest,
        "hasUpdate": has_update,
        "url": url,
        "notes": notes,
        "ui": ui,
    }))
}

fn version_cmp(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    let l = parse(latest);
    let c = parse(current);
    for i in 0..l.len().max(c.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv > cv {
            return true;
        }
        if lv < cv {
            return false;
        }
    }
    false
}

fn format_update_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0usize;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{value:.1} {}", UNITS[unit_idx])
    }
}

fn emit_update_progress(
    app: &tauri::AppHandle,
    stage: &str,
    progress: Option<u64>,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    message: &str,
) {
    let _ = app.emit(
        "update-progress",
        serde_json::json!({
            "stage": stage,
            "progress": progress,
            "downloadedBytes": downloaded_bytes,
            "totalBytes": total_bytes,
            "message": message,
        }),
    );
}

/// Run the actual update: download the installer package, install, and relaunch.
/// On macOS: downloads DMG, runs a bash helper script to swap the .app bundle.
/// On Windows: downloads MSI/EXE, runs the installer silently.
/// The `dmg_url` is passed from the frontend (originally from the website manifest).
#[tauri::command]
pub async fn run_update(app: tauri::AppHandle, dmg_url: String) -> Result<(), String> {
    if dmg_url.is_empty() {
        return Err("No download URL provided".to_string());
    }
    let client = reqwest::Client::builder()
        .user_agent("pawbae-updater")
        .build()
        .map_err(|e| format!("client build error: {e}"))?;
    // `message` is an untranslated fallback — the frontend localizes known
    // stages via `updateModal.progress.*` and only shows this for unknown ones.
    emit_update_progress(&app, "preparing", Some(0), 0, None, "Preparing update...");
    let mut resp = client
        .get(&dmg_url)
        .send()
        .await
        .map_err(|e| format!("download request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }

    let total_bytes = resp.content_length();
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let work_dir = std::env::temp_dir().join(format!("pawbae-update-{stamp}"));
    std::fs::create_dir_all(&work_dir).map_err(|e| format!("failed to create temp dir: {e}"))?;

    // Determine installer file extension based on URL and platform
    #[cfg(target_os = "macos")]
    let installer_filename = "pawbae-update.dmg";
    #[cfg(target_os = "windows")]
    let installer_filename = if dmg_url.ends_with(".msi") {
        "pawbae-update.msi"
    } else {
        "pawbae-update.exe"
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let installer_filename = "pawbae-update";

    let dmg_path = work_dir.join(installer_filename);
    #[cfg(target_os = "macos")]
    let helper_path = work_dir.join("install-update.sh");
    let log_path = work_dir.join("install.log");

    let mut file = tokio::fs::File::create(&dmg_path)
        .await
        .map_err(|e| format!("failed to create temp file: {e}"))?;
    let mut downloaded_bytes = 0u64;
    let mut last_progress: Option<u64> = None;

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("download stream failed: {e}"))?
    {
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("failed to write temp file: {e}"))?;
        downloaded_bytes += chunk.len() as u64;
        let progress = total_bytes
            .map(|total| ((downloaded_bytes.saturating_mul(100)) / total.max(1)).min(100));
        if progress != last_progress {
            let message = if let Some(total) = total_bytes {
                format!(
                    "Downloading update {} / {}",
                    format_update_bytes(downloaded_bytes),
                    format_update_bytes(total)
                )
            } else {
                format!(
                    "Downloading update {}",
                    format_update_bytes(downloaded_bytes)
                )
            };
            emit_update_progress(
                &app,
                "downloading",
                progress,
                downloaded_bytes,
                total_bytes,
                &message,
            );
            last_progress = progress;
        }
    }
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|e| format!("failed to flush temp file: {e}"))?;

    emit_update_progress(
        &app,
        "downloaded",
        Some(100),
        downloaded_bytes,
        total_bytes,
        "Download complete, preparing install...",
    );

    // Platform-specific: spawn a detached installer helper
    #[cfg(target_os = "macos")]
    {
        // Spawn a detached helper that waits for the app to quit, then swaps the bundle.
        let script = format!(
            r#"#!/bin/bash
set -euo pipefail
PID="{pid}"
APP_BUNDLE="/Applications/PawBae.app"
DMG_PATH="{dmg_path}"
LOG_PATH="{log_path}"
MOUNT_POINT=""

log() {{
  printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$1" >> "$LOG_PATH"
}}

cleanup() {{
  if [ -n "$MOUNT_POINT" ]; then
    hdiutil detach "$MOUNT_POINT" -quiet >/dev/null 2>&1 || true
  fi
}}

trap cleanup EXIT

log "Waiting for app pid $PID to exit"
for _ in $(seq 1 120); do
  if ! kill -0 "$PID" 2>/dev/null; then
    break
  fi
  sleep 0.5
done

if kill -0 "$PID" 2>/dev/null; then
  log "Timed out waiting for app to exit"
  exit 1
fi

if ! ATTACH_OUTPUT=$(hdiutil attach "$DMG_PATH" -nobrowse -readonly 2>&1); then
  log "$ATTACH_OUTPUT"
  exit 1
fi

MOUNT_POINT=$(printf '%s\n' "$ATTACH_OUTPUT" | awk 'match($0, /\/Volumes\/.*/) {{ print substr($0, RSTART); exit }}')
if [ -z "$MOUNT_POINT" ]; then
  log "Failed to determine DMG mount point"
  log "$ATTACH_OUTPUT"
  exit 1
fi

APP_PATH=""
for candidate in "$MOUNT_POINT"/*.app; do
  if [ -d "$candidate" ]; then
    APP_PATH="$candidate"
    break
  fi
done

if [ -z "$APP_PATH" ]; then
  log "No app bundle found in $MOUNT_POINT"
  /bin/ls -la "$MOUNT_POINT" >> "$LOG_PATH" 2>&1 || true
  exit 1
fi

log "Installing $APP_PATH"
rm -rf "$APP_BUNDLE"
ditto "$APP_PATH" "$APP_BUNDLE"
xattr -cr "$APP_BUNDLE" || true

log "Launching updated app"
open -n "$APP_BUNDLE"
"#,
            pid = std::process::id(),
            dmg_path = dmg_path.display(),
            log_path = log_path.display(),
        );
        std::fs::write(&helper_path, script)
            .map_err(|e| format!("failed to write helper script: {e}"))?;
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&helper_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("failed to chmod helper script: {e}"))?;
        }

        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("failed to open helper log: {e}"))?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("failed to clone helper log: {e}"))?;
        std::process::Command::new("bash")
            .arg(&helper_path)
            .stdout(std::process::Stdio::from(log_file))
            .stderr(std::process::Stdio::from(log_file_err))
            .spawn()
            .map_err(|e| format!("failed to start installer helper: {e}"))?;
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows: spawn the downloaded installer (MSI or EXE) with silent flags.
        // The installer will handle replacing the old version and relaunching.
        let helper_path = work_dir.join("install-update.ps1");
        let script = format!(
            r#"
$ErrorActionPreference = 'Stop'
# NOTE: $pid is a read-only automatic variable in PowerShell (current process PID).
# Use $appPid instead to avoid "VariableNotWritable" errors.
$appPid = {pid}
$installerPath = '{installer_path}'
$logPath = '{log_path}'

function Log($msg) {{
    "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $msg" | Out-File -Append $logPath
}}

Log "Waiting for app pid $appPid to exit"
$sw = [System.Diagnostics.Stopwatch]::StartNew()
while ($sw.Elapsed.TotalSeconds -lt 60) {{
    try {{
        $p = Get-Process -Id $appPid -ErrorAction SilentlyContinue
        if (-not $p) {{ break }}
    }} catch {{ break }}
    Start-Sleep -Milliseconds 500
}}

Log "Installing update from $installerPath"
if ($installerPath.EndsWith('.msi')) {{
    Start-Process msiexec.exe -ArgumentList '/i', "`"$installerPath`"", '/quiet', '/norestart' -Verb RunAs -Wait
}} else {{
    $installDir = $null
    foreach ($regPath in @(
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
    )) {{
        $entry = Get-ItemProperty $regPath -ErrorAction SilentlyContinue |
            Where-Object {{ $_.DisplayName -eq 'PawBae' }} | Select-Object -First 1
        if ($entry -and $entry.InstallLocation) {{
            $installDir = $entry.InstallLocation.Trim('"')
            break
        }}
    }}
    $nsisArgs = @('/S')
    if ($installDir) {{ $nsisArgs += "/D=$installDir" }}
    Log "Running installer with args: $($nsisArgs -join ' ')"
    try {{
        Start-Process $installerPath -ArgumentList $nsisArgs -Verb RunAs -Wait
    }} catch {{
        Log "Installer failed (UAC denied or error): $_"
        exit 1
    }}
}}

Log "Launching updated app"
# Find install location from registry (user may have chosen a custom path).
# The executable is named PawBae.exe (productName config produces this binary name).
$appPath = $null
foreach ($regPath in @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
)) {{
    $entry = Get-ItemProperty $regPath -ErrorAction SilentlyContinue |
        Where-Object {{ $_.DisplayName -eq 'PawBae' }} | Select-Object -First 1
    if ($entry -and $entry.InstallLocation) {{
        $loc = $entry.InstallLocation.Trim('"')
        $candidate = Join-Path $loc 'PawBae.exe'
        if (Test-Path $candidate) {{
            $appPath = $candidate
            break
        }}
    }}
}}
if (-not $appPath) {{
    # Fallback: check common locations
    foreach ($dir in @("$env:LOCALAPPDATA\PawBae", "$env:ProgramFiles\PawBae", "H:\PawBae")) {{
        $candidate = Join-Path $dir 'PawBae.exe'
        if (Test-Path $candidate) {{ $appPath = $candidate; break }}
    }}
}}
if ($appPath) {{
    Log "Relaunching from $appPath"
    Start-Process $appPath
}} else {{
    Log "Warning: could not find PawBae.exe to relaunch"
}}
"#,
            pid = std::process::id(),
            installer_path = dmg_path
                .display()
                .to_string()
                .replace('\\', "\\\\")
                .replace('\'', "''"),
            log_path = log_path
                .display()
                .to_string()
                .replace('\\', "\\\\")
                .replace('\'', "''"),
        );
        std::fs::write(&helper_path, &script)
            .map_err(|e| format!("failed to write helper script: {e}"))?;

        let mut update_cmd = std::process::Command::new("powershell");
        update_cmd
            .args(["-ExecutionPolicy", "Bypass", "-File"])
            .arg(&helper_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        hide_window_cmd(&mut update_cmd);
        update_cmd
            .spawn()
            .map_err(|e| format!("failed to start installer helper: {e}"))?;
    }

    emit_update_progress(
        &app,
        "ready_to_restart",
        Some(100),
        downloaded_bytes,
        total_bytes,
        "Update ready — restart to install",
    );
    Ok(())
}
