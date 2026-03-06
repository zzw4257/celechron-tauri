use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialAsset {
    pub id: String,
    pub course_name: String,
    pub title: String,
    pub file_name: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub source_url: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: u64,
    pub downloaded_at: u64,
    pub updated_at: u64,
    pub exists: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadMaterialInput {
    pub url: String,
    pub course_name: String,
    pub title: String,
    pub file_name: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialPathInput {
    pub relative_path: String,
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn materials_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法获取应用数据目录: {error}"))?
        .join("materials");
    fs::create_dir_all(&dir).map_err(|error| format!("无法创建资料目录: {error}"))?;
    Ok(dir)
}

fn sanitize_segment(input: &str) -> String {
    let trimmed = input.trim();
    let fallback = if trimmed.is_empty() {
        "untitled"
    } else {
        trimmed
    };
    fallback
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

fn ensure_safe_relative(relative_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(relative_path);
    if path.is_absolute() {
        return Err("资料路径必须为相对路径".to_string());
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("资料路径非法".to_string());
    }
    Ok(path)
}

fn material_meta_path(asset_path: &Path) -> PathBuf {
    let file_name = asset_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    asset_path.with_file_name(format!("{file_name}.meta.json"))
}

fn to_unix_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn hydrate_asset(root: &Path, asset_path: &Path, stored: &Value) -> Option<MaterialAsset> {
    let relative_path = asset_path.strip_prefix(root).ok()?.to_path_buf();
    let file_name = asset_path.file_name()?.to_string_lossy().to_string();
    let title = stored
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or(file_name.as_str())
        .to_string();
    let course_name = stored
        .get("courseName")
        .and_then(Value::as_str)
        .unwrap_or("未分组课程")
        .to_string();
    let downloaded_at = stored
        .get("downloadedAt")
        .and_then(Value::as_u64)
        .unwrap_or_else(now_ts);
    let updated_at = stored
        .get("updatedAt")
        .and_then(Value::as_u64)
        .unwrap_or(downloaded_at);
    let size_bytes = stored
        .get("sizeBytes")
        .and_then(Value::as_u64)
        .or_else(|| fs::metadata(asset_path).ok().map(|meta| meta.len()))
        .unwrap_or_default();
    Some(MaterialAsset {
        id: stored
            .get("id")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .unwrap_or_else(|| format!("{}-{}", downloaded_at, file_name)),
        course_name,
        title,
        file_name,
        relative_path: to_unix_path(&relative_path),
        absolute_path: asset_path.to_string_lossy().to_string(),
        source_url: stored
            .get("sourceUrl")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        mime_type: stored
            .get("mimeType")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        size_bytes,
        downloaded_at,
        updated_at,
        exists: asset_path.exists(),
    })
}

fn read_materials(root: &Path) -> Result<Vec<MaterialAsset>, String> {
    let mut stack = vec![root.to_path_buf()];
    let mut items = Vec::new();

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if !file_name.ends_with(".meta.json") {
                continue;
            }
            let stored = fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_json::from_str::<Value>(&content).ok())
                .unwrap_or_else(|| json!({}));
            let asset_path = path.with_file_name(file_name.trim_end_matches(".meta.json"));
            if let Some(asset) = hydrate_asset(root, &asset_path, &stored) {
                items.push(asset);
            }
        }
    }

    items.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.course_name.cmp(&right.course_name))
            .then_with(|| left.title.cmp(&right.title))
    });
    Ok(items)
}

fn resolve_asset_path(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let relative = ensure_safe_relative(relative_path)?;
    let absolute = root.join(relative);
    if !absolute.starts_with(root) {
        return Err("资料路径越界".to_string());
    }
    Ok(absolute)
}

fn infer_filename(url: &str, response: &reqwest::Response, preferred_name: Option<&str>) -> String {
    let preferred = preferred_name.unwrap_or_default().trim();
    if !preferred.is_empty() {
        return sanitize_segment(preferred);
    }

    if let Some(disposition) = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
    {
        for segment in disposition.split(';') {
            let trimmed = segment.trim();
            if let Some(filename) = trimmed.strip_prefix("filename=") {
                return sanitize_segment(filename.trim_matches('"'));
            }
        }
    }

    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|segments| segments.last().map(|value| value.to_string()))
        })
        .map(|value| sanitize_segment(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("asset-{}.bin", now_ts()))
}

pub fn fetch_materials(app: &AppHandle) -> Result<Value, String> {
    let root = materials_root(app)?;
    let items = read_materials(&root)?;
    Ok(json!({ "items": items }))
}

pub async fn download_material_asset(
    app: &AppHandle,
    input: DownloadMaterialInput,
) -> Result<Value, String> {
    let root = materials_root(app)?;
    let course_dir = root.join(sanitize_segment(&input.course_name));
    fs::create_dir_all(&course_dir).map_err(|error| format!("无法创建课程资料目录: {error}"))?;

    let response = reqwest::Client::new()
        .get(&input.url)
        .send()
        .await
        .map_err(|error| format!("下载资料失败: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("下载资料失败: HTTP {}", status.as_u16()));
    }

    let mime_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let file_name = infer_filename(&input.url, &response, input.file_name.as_deref());
    let asset_path = course_dir.join(&file_name);
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取资料内容失败: {error}"))?;
    let mut file =
        fs::File::create(&asset_path).map_err(|error| format!("写入资料失败: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("写入资料失败: {error}"))?;

    let now = now_ts();
    let relative_path = asset_path
        .strip_prefix(&root)
        .map_err(|error| format!("生成资料路径失败: {error}"))?;
    let meta = json!({
        "id": format!("material-{}-{}", now, file_name),
        "courseName": input.course_name,
        "title": input.title,
        "sourceUrl": input.source.unwrap_or(input.url),
        "mimeType": mime_type,
        "sizeBytes": bytes.len() as u64,
        "downloadedAt": now,
        "updatedAt": now,
        "relativePath": to_unix_path(relative_path),
    });
    fs::write(
        material_meta_path(&asset_path),
        serde_json::to_vec_pretty(&meta).map_err(|error| format!("写入资料元数据失败: {error}"))?,
    )
    .map_err(|error| format!("写入资料元数据失败: {error}"))?;

    let asset =
        hydrate_asset(&root, &asset_path, &meta).ok_or_else(|| "资料元数据构建失败".to_string())?;
    Ok(json!({ "item": asset }))
}

pub fn open_material_asset(app: &AppHandle, input: MaterialPathInput) -> Result<Value, String> {
    let root = materials_root(app)?;
    let asset_path = resolve_asset_path(&root, &input.relative_path)?;
    if !asset_path.exists() {
        return Err("资料文件不存在".to_string());
    }
    app.opener()
        .open_path(asset_path.to_string_lossy().to_string(), None::<String>)
        .map_err(|error| format!("打开资料失败: {error}"))?;
    Ok(json!({ "ok": true }))
}

pub fn remove_material_cache(app: &AppHandle, input: MaterialPathInput) -> Result<Value, String> {
    let root = materials_root(app)?;
    let asset_path = resolve_asset_path(&root, &input.relative_path)?;
    let meta_path = material_meta_path(&asset_path);
    if asset_path.exists() {
        fs::remove_file(&asset_path).map_err(|error| format!("删除资料失败: {error}"))?;
    }
    if meta_path.exists() {
        fs::remove_file(&meta_path).map_err(|error| format!("删除资料元数据失败: {error}"))?;
    }
    Ok(json!({ "ok": true }))
}
