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
    #[serde(rename = "petJsonUrl")]
    pub pet_json_url: String,
}

/// Path to the user's codex CLI pets directory (`~/.codex/pets`). Mirrors
/// the layout used by the codex CLI hatch-pet skill so users can drop the
/// same pet folders here and have them show up in the picker.
pub(crate) fn codex_pets_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("pets"))
}

/// Skin ids double as folder names under `~/.codex/pets`, and skins are
/// stranger-supplied content. Unicode is welcome (creators name skins in
/// Chinese); separators, dot-tricks, drive colons, and control chars are not.
/// Mirrors `isSafeSkinId` in `src/lib/utils/skin-validate.ts`.
pub(crate) fn is_safe_skin_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= 64
        && !id.starts_with('.')
        && !id.contains("..")
        && !id
            .chars()
            .any(|c| c == '/' || c == '\\' || c == ':' || c.is_control())
}

/// Staging area for imports awaiting frontend validation. Lives under the pets
/// root so `codexpet://` can serve staged files for the validator; `.staging`
/// can never collide with a real skin because leading-dot ids are rejected.
pub(crate) fn staging_dir() -> Option<PathBuf> {
    codex_pets_dir().map(|r| r.join(".staging"))
}

/// A source inside the pets root would be destroyed by the overwrite dance —
/// e.g. re-importing an installed skin straight out of "Open skins folder".
fn reject_src_inside_pets_root(src: &std::path::Path) -> Result<(), String> {
    let Some(root) = codex_pets_dir() else {
        return Ok(());
    };
    let (Ok(src_c), Ok(root_c)) = (src.canonicalize(), root.canonicalize()) else {
        return Ok(()); // pets root may not exist yet — nothing to protect
    };
    if src_c.starts_with(&root_c) {
        return Err("source is inside the skins directory (already installed)".into());
    }
    Ok(())
}

/// Fold an image filename stem into a safe skin id: separators/whitespace
/// collapse to dashes, unicode survives. Falls back to a deterministic hash id
/// when nothing safe remains.
fn slug_skin_id(stem: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true; // suppress leading dashes
    for ch in stem.chars() {
        let dashy = ch == '/' || ch == '\\' || ch == ':' || ch.is_control() || ch.is_whitespace();
        if dashy {
            if !last_dash {
                out.push('-');
            }
            last_dash = true;
        } else {
            out.push(ch);
            last_dash = false;
        }
    }
    let trimmed: String = out
        .trim_matches(|c| c == '-' || c == '.')
        .chars()
        .take(48)
        .collect();
    if is_safe_skin_id(&trimmed) {
        return trimmed;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in stem.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("skin-{:08x}", (h & 0xffff_ffff) as u32)
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
        let pet_json_url = codex_asset_url(&pet_json);
        out.push(CodexPetMeta {
            id,
            display_name,
            description,
            spritesheet_url: url,
            pet_json_url,
        });
    }
    out.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
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
    let result = picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned());

    // The dialog briefly steals focus and the OS can demote our floating
    // mini window back to the normal level. Re-apply always-on-top so the
    // settings panel doesn't visually sink under other apps.
    crate::pet_core::reassert_mini_floating(&app);
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
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(path)
}
/// Stage a dropped pet folder into `~/.codex/pets/.staging/<id>` for the
/// frontend validator. The source must be a directory containing at minimum a
/// `pet.json` and a spritesheet image. Nothing under the installed skins is
/// touched until `commit_staged_skin` — a failed upgrade must never destroy
/// the working copy (and a source picked from inside the skins dir is
/// rejected outright, or the overwrite would delete it before the copy).
#[tauri::command]
pub async fn import_codex_pet(src_path: String) -> Result<CodexPetMeta, String> {
    let src = PathBuf::from(&src_path);
    if !src.is_dir() {
        return Err(format!("not a directory: {}", src_path));
    }
    reject_src_inside_pets_root(&src)?;
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
    if !is_safe_skin_id(&id) {
        return Err(format!("unsafe pet id: {id:?}"));
    }
    let Some(staging_root) = staging_dir() else {
        return Err("home directory not found".into());
    };
    std::fs::create_dir_all(&staging_root).map_err(|e| e.to_string())?;
    let dst = staging_root.join(&id);
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
    let pet_json_url = codex_asset_url(&dst.join("pet.json"));
    Ok(CodexPetMeta {
        id,
        display_name,
        description,
        spritesheet_url: url,
        pet_json_url,
    })
}

/// Promote a staged skin into the installed set: only now is a previous copy
/// of the same id replaced (the validator has already passed the staged one).
#[tauri::command]
pub async fn commit_staged_skin(id: String) -> Result<(), String> {
    if !is_safe_skin_id(&id) {
        return Err(format!("unsafe skin id: {id:?}"));
    }
    let (Some(root), Some(staging_root)) = (codex_pets_dir(), staging_dir()) else {
        return Err("home directory not found".into());
    };
    let staged = staging_root.join(&id);
    if !staged.is_dir() {
        return Err(format!("no staged skin: {id}"));
    }
    let dst = root.join(&id);
    if dst.exists() {
        std::fs::remove_dir_all(&dst).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&staged, &dst).map_err(|e| e.to_string())?;
    Ok(())
}

/// Drop a staged skin that failed validation. Installed skins are untouched.
#[tauri::command]
pub async fn discard_staged_skin(id: String) -> Result<(), String> {
    if !is_safe_skin_id(&id) {
        return Err(format!("unsafe skin id: {id:?}"));
    }
    let Some(staging_root) = staging_dir() else {
        return Err("home directory not found".into());
    };
    let staged = staging_root.join(&id);
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(|e| e.to_string())?;
    }
    Ok(())
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
        .map(|c| {
            utf8_percent_encode(&c.as_os_str().to_string_lossy(), NON_ALPHANUMERIC).to_string()
        })
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
const SKIN_IMAGE_EXTS: [&str; 5] = ["png", "webp", "jpg", "jpeg", "gif"];

/// Native file picker for the single-image skin import ("宽进"). Same
/// parent-window + floating-reassert dance as `pick_codex_pet_folder`.
#[tauri::command]
pub async fn pick_skin_image(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut builder = app
        .dialog()
        .file()
        .set_title("选择皮肤图片")
        .add_filter("Image", &SKIN_IMAGE_EXTS);
    if let Some(win) = app.get_webview_window("main") {
        builder = builder.set_parent(&win);
    }
    builder.pick_file(move |path| {
        let _ = tx.send(path);
    });
    let picked = rx.await.map_err(|e| e.to_string())?;
    let result = picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned());
    crate::pet_core::reassert_mini_floating(&app);
    Ok(result)
}

/// Wrap a single still image into a full skin folder: whole image = a 1×1 atlas
/// with a 1-frame `idle` row; every other animation falls back gracefully at
/// runtime. This is the lowest rung of the "宽进严出" creator funnel. Staged
/// like folder imports — installed skins are only touched on commit.
#[tauri::command]
pub async fn import_skin_image(src_path: String) -> Result<CodexPetMeta, String> {
    let src = PathBuf::from(&src_path);
    if !src.is_file() {
        return Err(format!("not a file: {}", src_path));
    }
    reject_src_inside_pets_root(&src)?;
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !SKIN_IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(format!("unsupported image type: .{ext}"));
    }
    let dim = imagesize::size(&src).map_err(|e| format!("cannot read image size: {e}"))?;
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "skin".into());
    let id = slug_skin_id(&stem);
    let Some(staging_root) = staging_dir() else {
        return Err("home directory not found".into());
    };
    let dst = staging_root.join(&id);
    if dst.exists() {
        let _ = std::fs::remove_dir_all(&dst);
    }
    std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
    let sheet_name = format!("spritesheet.{ext}");
    std::fs::copy(&src, dst.join(&sheet_name)).map_err(|e| e.to_string())?;
    let manifest = serde_json::json!({
        "id": id.clone(),
        "displayName": stem.clone(),
        "description": "",
        "spritesheetPath": sheet_name.clone(),
        "atlas": { "cellW": dim.width, "cellH": dim.height, "cols": 1, "rows": 1 },
        "animations": { "idle": { "row": 0, "frames": 1 } },
    });
    let pretty = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(dst.join("pet.json"), pretty).map_err(|e| e.to_string())?;
    Ok(CodexPetMeta {
        id,
        display_name: stem,
        description: String::new(),
        spritesheet_url: codex_asset_url(&dst.join(&sheet_name)),
        pet_json_url: codex_asset_url(&dst.join("pet.json")),
    })
}

/// Delete a custom skin folder. The id is sanitized so a hostile value can't
/// escape `~/.codex/pets` (defense in depth with the TS validator).
#[tauri::command]
pub async fn remove_custom_skin(id: String) -> Result<(), String> {
    if !is_safe_skin_id(&id) {
        return Err(format!("unsafe skin id: {id:?}"));
    }
    let Some(root) = codex_pets_dir() else {
        return Err("home directory not found".into());
    };
    let dst = root.join(&id);
    if dst.exists() {
        std::fs::remove_dir_all(&dst).map_err(|e| e.to_string())?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{is_safe_skin_id, slug_skin_id};

    #[test]
    fn safe_ids_accept_plain_and_unicode() {
        assert!(is_safe_skin_id("mimi-2"));
        assert!(is_safe_skin_id("muru.codex-pet"));
        assert!(is_safe_skin_id("云朵小猫"));
    }

    #[test]
    fn safe_ids_reject_traversal_shapes() {
        for bad in ["", "..", "../evil", "a/b", "a\\b", "C:evil", ".hidden"] {
            assert!(!is_safe_skin_id(bad), "should reject {bad:?}");
        }
    }

    #[test]
    fn slug_folds_separators_and_keeps_unicode() {
        assert_eq!(slug_skin_id("My Cool Cat"), "My-Cool-Cat");
        assert_eq!(slug_skin_id("云朵 小猫"), "云朵-小猫");
        assert_eq!(slug_skin_id("a/b:c"), "a-b-c");
    }

    #[test]
    fn slug_falls_back_to_deterministic_hash() {
        let a = slug_skin_id("...");
        let b = slug_skin_id("...");
        assert_eq!(a, b);
        assert!(a.starts_with("skin-"));
        assert!(is_safe_skin_id(&a));
    }
}
