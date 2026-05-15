//! Tauri commands for managing user-supplied codex CLI pets dropped into `~/.codex/pets`.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CodexPetMeta {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    #[serde(rename = "spritesheetUrl")]
    pub spritesheet_url: String,
}

/// Path to the user's codex CLI pets directory (`~/.codex/pets`). Mirrors
/// the layout used by the codex CLI hatch-pet skill so users can drop the
/// same pet folders here and have them show up in the picker.
pub(crate) fn codex_pets_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("pets"))
}
/// List custom codex pets the user has dropped into `~/.codex/pets`. Each
/// pet folder must contain a `pet.json` metadata file plus a spritesheet
/// (.webp/.png/.jpg). Missing pieces are skipped silently.
#[tauri::command]
pub async fn list_custom_codex_pets() -> Result<Vec<CodexPetMeta>, String> {
    let Some(root) = codex_pets_dir() else {
        return Ok(Vec::new());
    };
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let pet_json = path.join("pet.json");
        if !pet_json.is_file() {
            continue;
        }
        let raw = match std::fs::read_to_string(&pet_json) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let meta: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let id = meta
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
        let display_name = meta
            .get("displayName")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| id.clone());
        let description = meta
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_default();
        let sheet_path = meta
            .get("spritesheetPath")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| "spritesheet.webp".into());
        let abs = path.join(&sheet_path);
        if !abs.is_file() {
            continue;
        }
        let url = codex_asset_url(&abs);
        out.push(CodexPetMeta {
            id,
            display_name,
            description,
            spritesheet_url: url,
        });
    }
    out.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    Ok(out)
}
#[tauri::command]
pub async fn pick_codex_pet_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;

    // Use the official tauri-plugin-dialog so we get a real native folder
    // picker on every platform: NSOpenPanel on macOS, IFileOpenDialog
    // (modern explorer-style) on Windows, GTK on Linux.
    //
    // Bind the dialog to the mini window as its parent. Without an explicit
    // owner the dialog renders as a peer top-level window that sits below
    // our always-on-top mini frame on Windows (the user sees the picker
    // visually behind the settings panel on the second open). With a parent
    // window, the OS layers the dialog above its owner.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut builder = app.dialog().file().set_title("选择 codex 宠物文件夹");
    if let Some(win) = app.get_webview_window("main") {
        builder = builder.set_parent(&win);
    }
    builder.pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let picked = rx.await.map_err(|e| e.to_string())?;
    let result = picked.and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned());

    // The dialog briefly steals focus and the OS can demote our floating
    // mini window back to the normal level. Re-apply always-on-top so the
    // settings panel doesn't visually sink under other apps.
    crate::reassert_mini_floating(&app);
    Ok(result)
}
/// Open `~/.codex/pets` in the platform's file manager. Creates the
/// directory if it doesn't exist yet so the picker's "Open Folder" link
/// always lands somewhere usable.
#[tauri::command]
pub async fn open_codex_pets_dir() -> Result<String, String> {
    let Some(dir) = codex_pets_dir() else {
        return Err("home directory not found".into());
    };
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let path = dir.to_string_lossy().to_string();
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    Ok(path)
}
/// Import a dropped pet folder into `~/.codex/pets`. The source must be a
/// directory containing at minimum a `pet.json` and a spritesheet image.
/// Existing folders with the same id are overwritten so re-dropping a
/// pet upgrades it in place.
#[tauri::command]
pub async fn import_codex_pet(src_path: String) -> Result<CodexPetMeta, String> {
    let src = PathBuf::from(&src_path);
    if !src.is_dir() {
        return Err(format!("not a directory: {}", src_path));
    }
    let pet_json = src.join("pet.json");
    if !pet_json.is_file() {
        return Err("missing pet.json in dropped folder".into());
    }
    let raw = std::fs::read_to_string(&pet_json).map_err(|e| e.to_string())?;
    let meta: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let id = meta
        .get("id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            src.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "pet".into())
        });
    let Some(root) = codex_pets_dir() else {
        return Err("home directory not found".into());
    };
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let dst = root.join(&id);
    if dst.exists() {
        let _ = std::fs::remove_dir_all(&dst);
    }
    copy_dir_recursive(&src, &dst)?;
    let display_name = meta
        .get("displayName")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| id.clone());
    let description = meta
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_default();
    let sheet_path = meta
        .get("spritesheetPath")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| "spritesheet.webp".into());
    let url = codex_asset_url(&dst.join(&sheet_path));
    Ok(CodexPetMeta {
        id,
        display_name,
        description,
        spritesheet_url: url,
    })
}
/// Build a Tauri custom-protocol URL the webview can fetch. The path is
/// resolved relative to `~/.codex/pets/` by the `codexpet://` scheme
/// registered on the Tauri builder, so files outside the bundled
/// `public/` folder still load.
fn codex_asset_url(abs: &std::path::Path) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    let Some(root) = codex_pets_dir() else {
        return String::new();
    };
    let rel = match abs.strip_prefix(&root) {
        Ok(p) => p.to_path_buf(),
        Err(_) => return String::new(),
    };
    let parts: Vec<String> = rel
        .components()
        .map(|c| utf8_percent_encode(&c.as_os_str().to_string_lossy(), NON_ALPHANUMERIC).to_string())
        .collect();
    // Windows WebView2 cannot fetch `<scheme>://...` URLs registered via
    // `register_uri_scheme_protocol`; Tauri exposes them as
    // `http://<scheme>.localhost/...` instead. Match the same convention as
    // localasset/codexpet so codex pet sprites actually load.
    let prefix = if cfg!(target_os = "windows") {
        "http://codexpet.localhost"
    } else {
        "codexpet://localhost"
    };
    format!("{}/{}", prefix, parts.join("/"))
}
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
