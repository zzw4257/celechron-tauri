use crate::classroom::{ClassroomSession, ClassroomSubject};
use crate::courses;
use crate::zjuam::AppState;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

const REMOTE_INDEX_VERSION: u32 = 2;
const REMOTE_INDEX_FILE: &str = "_remote_index_v2.json";
const SOURCE_PRIORITY: [&str; 3] = ["classroom", "activity", "homework"];

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMaterialAsset {
    pub id: String,
    pub course_id: i64,
    pub course_name: String,
    pub title: String,
    pub file_name: String,
    pub source_type: String,
    pub source_url: String,
    pub fallback_source_url: String,
    pub mime_type: Option<String>,
    pub size_bytes: u64,
    pub updated_at: u64,
    pub downloaded: bool,
    pub local_relative_path: Option<String>,
    pub week_bucket: String,
    #[serde(default)]
    pub preview_image_urls: Vec<String>,
    #[serde(default)]
    pub remote_ref: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CourseFilter {
    pub id: String,
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RemoteMaterialsIndex {
    version: u32,
    items: Vec<RemoteMaterialAsset>,
    last_synced_at: Option<u64>,
    warnings: Vec<String>,
    week_label: Option<String>,
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
pub struct RemoteMaterialDownloadInput {
    pub remote_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialPathInput {
    pub relative_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialContentInput {
    pub relative_path: String,
    pub max_chars: Option<usize>,
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

fn remote_index_path(root: &Path) -> PathBuf {
    root.join(REMOTE_INDEX_FILE)
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

fn classroom_assets_dir(asset_path: &Path) -> PathBuf {
    let stem = asset_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    asset_path.with_file_name(format!("{stem}_assets"))
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

fn read_remote_index(root: &Path) -> RemoteMaterialsIndex {
    let path = remote_index_path(root);
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<RemoteMaterialsIndex>(&content).ok())
        .unwrap_or_else(|| RemoteMaterialsIndex {
            version: REMOTE_INDEX_VERSION,
            ..RemoteMaterialsIndex::default()
        })
}

fn write_remote_index(root: &Path, index: &RemoteMaterialsIndex) -> Result<(), String> {
    fs::write(
        remote_index_path(root),
        serde_json::to_vec_pretty(index)
            .map_err(|error| format!("写入远程资料索引失败: {error}"))?,
    )
    .map_err(|error| format!("写入远程资料索引失败: {error}"))
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

fn guess_extension(url: &str, mime_type: Option<&str>) -> &'static str {
    if let Some(mime) = mime_type {
        if mime.contains("png") {
            return "png";
        }
        if mime.contains("jpeg") || mime.contains("jpg") {
            return "jpg";
        }
        if mime.contains("webp") {
            return "webp";
        }
        if mime.contains("gif") {
            return "gif";
        }
        if mime.contains("svg") {
            return "svg";
        }
    }
    let lower = url.to_ascii_lowercase();
    if lower.contains(".png") {
        "png"
    } else if lower.contains(".jpg") || lower.contains(".jpeg") {
        "jpg"
    } else if lower.contains(".webp") {
        "webp"
    } else if lower.contains(".gif") {
        "gif"
    } else if lower.contains(".svg") {
        "svg"
    } else {
        "png"
    }
}

fn source_key(source_url: &str) -> String {
    source_url.trim().to_lowercase()
}

fn course_file_key(course_name: &str, file_name: &str) -> String {
    format!(
        "{}::{}",
        course_name.trim().to_lowercase(),
        file_name.trim().to_lowercase()
    )
}

fn source_rank(source_type: &str) -> usize {
    SOURCE_PRIORITY
        .iter()
        .position(|value| *value == source_type)
        .unwrap_or(SOURCE_PRIORITY.len())
}

fn week_bucket_rank(week_bucket: &str) -> usize {
    match week_bucket {
        "current" => 0,
        "other" => 1,
        _ => 2,
    }
}

fn attach_download_status(
    mut remote_items: Vec<RemoteMaterialAsset>,
    local_items: &[MaterialAsset],
) -> Vec<RemoteMaterialAsset> {
    let mut source_lookup = HashMap::<String, String>::new();
    let mut course_file_lookup = HashMap::<String, String>::new();

    for item in local_items {
        if let Some(source_url) = item.source_url.as_deref() {
            source_lookup.insert(source_key(source_url), item.relative_path.clone());
        }
        course_file_lookup.insert(
            course_file_key(&item.course_name, &item.file_name),
            item.relative_path.clone(),
        );
    }

    for item in &mut remote_items {
        let match_path = source_lookup
            .get(&source_key(&item.source_url))
            .or_else(|| source_lookup.get(&source_key(&item.fallback_source_url)))
            .cloned()
            .or_else(|| {
                course_file_lookup
                    .get(&course_file_key(&item.course_name, &item.file_name))
                    .cloned()
            });
        item.downloaded = match_path.is_some();
        item.local_relative_path = match_path;
    }

    remote_items.sort_by(|left, right| {
        week_bucket_rank(&left.week_bucket)
            .cmp(&week_bucket_rank(&right.week_bucket))
            .then_with(|| source_rank(&left.source_type).cmp(&source_rank(&right.source_type)))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.course_name.cmp(&right.course_name))
            .then_with(|| left.title.cmp(&right.title))
    });
    remote_items
}

fn parse_timestamp_value(value: &Value) -> Option<u64> {
    if let Some(ts) = value.as_u64() {
        return Some(if ts > 10_000_000_000 { ts / 1000 } else { ts });
    }
    if let Some(ts) = value.as_i64() {
        if ts > 0 {
            return Some(if ts > 10_000_000_000 {
                (ts / 1000) as u64
            } else {
                ts as u64
            });
        }
    }
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(ts) = text.parse::<i64>() {
        if ts > 0 {
            return Some(if ts > 10_000_000_000 {
                (ts / 1000) as u64
            } else {
                ts as u64
            });
        }
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(dt.timestamp().max(0) as u64);
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S") {
        return Local
            .from_local_datetime(&dt)
            .single()
            .map(|value| value.timestamp().max(0) as u64);
    }
    None
}

fn first_nonempty_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(Value::as_str).and_then(|text| {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn normalize_learning_upload(
    course_id: i64,
    course_name: &str,
    source_type: &str,
    upload: &Value,
) -> Option<RemoteMaterialAsset> {
    let upload_id = upload.get("id").and_then(Value::as_i64)?;
    let reference_id = upload
        .get("reference_id")
        .and_then(Value::as_i64)
        .unwrap_or(upload_id);
    let file_name = sanitize_segment(first_nonempty_string(
        upload,
        &["name", "file_name", "fileName", "title"],
    )?);
    let title = first_nonempty_string(upload, &["title", "name", "file_name", "fileName"])
        .unwrap_or(file_name.as_str())
        .to_string();
    let mime_type = first_nonempty_string(
        upload,
        &["content_type", "contentType", "mime_type", "mimeType"],
    )
    .map(|value| value.to_string());
    let updated_at = [
        "updated_at",
        "updatedAt",
        "created_at",
        "createdAt",
        "publish_at",
        "publishAt",
    ]
    .iter()
    .find_map(|key| upload.get(*key).and_then(parse_timestamp_value))
    .unwrap_or_else(now_ts);
    Some(RemoteMaterialAsset {
        id: format!("learning:{source_type}:{course_id}:{upload_id}:{reference_id}"),
        course_id,
        course_name: course_name.to_string(),
        title,
        file_name: file_name.clone(),
        source_type: source_type.to_string(),
        source_url: format!("https://courses.zju.edu.cn/api/uploads/reference/{reference_id}/blob"),
        fallback_source_url: format!("https://courses.zju.edu.cn/api/uploads/{upload_id}/blob"),
        mime_type,
        size_bytes: upload
            .get("size")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        updated_at,
        downloaded: false,
        local_relative_path: None,
        week_bucket: "unknown".to_string(),
        preview_image_urls: Vec::new(),
        remote_ref: json!({
            "kind": "learning",
            "uploadId": upload_id,
            "referenceId": reference_id,
            "courseId": course_id,
            "sourceType": source_type,
        }),
    })
}

fn normalize_classroom_subject(subject: ClassroomSubject, updated_at: u64) -> RemoteMaterialAsset {
    let file_name = format!("{}.html", sanitize_segment(&subject.sub_name));
    RemoteMaterialAsset {
        id: format!("classroom:{}:{}", subject.course_id, subject.sub_id),
        course_id: subject.course_id,
        course_name: subject.course_name.clone(),
        title: subject.sub_name.clone(),
        file_name,
        source_type: "classroom".to_string(),
        source_url: format!(
            "classroom://course/{}/sub/{}",
            subject.course_id, subject.sub_id
        ),
        fallback_source_url: format!(
            "classroom://course/{}/sub/{}",
            subject.course_id, subject.sub_id
        ),
        mime_type: Some("text/html".to_string()),
        size_bytes: subject.ppt_image_urls.len() as u64,
        updated_at,
        downloaded: false,
        local_relative_path: None,
        week_bucket: subject.week_bucket,
        preview_image_urls: subject.ppt_image_urls.clone(),
        remote_ref: json!({
            "kind": "classroom",
            "courseId": subject.course_id,
            "subId": subject.sub_id,
            "lecturerName": subject.lecturer_name,
            "pptImageUrls": subject.ppt_image_urls,
        }),
    }
}

fn build_material_meta(
    course_name: &str,
    title: &str,
    source_url: String,
    mime_type: Option<String>,
    size_bytes: u64,
    relative_path: &Path,
    file_name: &str,
    downloaded_at: u64,
) -> Value {
    json!({
        "id": format!("material-{}-{}", downloaded_at, file_name),
        "courseName": course_name,
        "title": title,
        "sourceUrl": source_url,
        "mimeType": mime_type,
        "sizeBytes": size_bytes,
        "downloadedAt": downloaded_at,
        "updatedAt": downloaded_at,
        "relativePath": to_unix_path(relative_path),
    })
}

fn write_material_from_bytes(
    root: &Path,
    course_name: &str,
    title: &str,
    source_url: String,
    mime_type: Option<String>,
    file_name: &str,
    bytes: &[u8],
) -> Result<MaterialAsset, String> {
    let course_dir = root.join(sanitize_segment(course_name));
    fs::create_dir_all(&course_dir).map_err(|error| format!("无法创建课程资料目录: {error}"))?;
    let asset_path = course_dir.join(file_name);
    let mut file =
        fs::File::create(&asset_path).map_err(|error| format!("写入资料失败: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("写入资料失败: {error}"))?;

    let now = now_ts();
    let relative_path = asset_path
        .strip_prefix(root)
        .map_err(|error| format!("生成资料路径失败: {error}"))?;
    let meta = build_material_meta(
        course_name,
        title,
        source_url,
        mime_type,
        bytes.len() as u64,
        relative_path,
        file_name,
        now,
    );
    fs::write(
        material_meta_path(&asset_path),
        serde_json::to_vec_pretty(&meta).map_err(|error| format!("写入资料元数据失败: {error}"))?,
    )
    .map_err(|error| format!("写入资料元数据失败: {error}"))?;

    hydrate_asset(root, &asset_path, &meta).ok_or_else(|| "资料元数据构建失败".to_string())
}

fn write_classroom_html_asset(
    root: &Path,
    course_name: &str,
    title: &str,
    source_url: String,
    file_name: &str,
    html: &str,
) -> Result<MaterialAsset, String> {
    write_material_from_bytes(
        root,
        course_name,
        title,
        source_url,
        Some("text/html".to_string()),
        file_name,
        html.as_bytes(),
    )
}

fn is_text_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()).map(|value| value.to_ascii_lowercase()),
        Some(ext)
            if matches!(
                ext.as_str(),
                "txt" | "md" | "markdown" | "json" | "csv" | "tsv" | "log" | "yaml" | "yml" | "xml" | "html" | "htm" | "js" | "ts" | "jsx" | "tsx" | "py" | "rs" | "java" | "c" | "cpp" | "h" | "hpp"
            )
    )
}

fn build_course_filters(items: &[RemoteMaterialAsset]) -> Vec<CourseFilter> {
    let mut counts = BTreeMap::<String, usize>::new();
    for item in items {
        *counts.entry(item.course_name.clone()).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(label, count)| CourseFilter {
            id: label.clone(),
            label,
            count,
        })
        .collect()
}

fn build_materials_payload(items: Vec<MaterialAsset>, mut index: RemoteMaterialsIndex) -> Value {
    index.version = REMOTE_INDEX_VERSION;
    index.items = attach_download_status(index.items, &items);
    let default_scope = if index.items.iter().any(|item| item.week_bucket == "current") {
        "current-week"
    } else {
        "all"
    };
    let available_remote_count = index.items.iter().filter(|item| !item.downloaded).count();
    let course_filters = build_course_filters(&index.items);
    json!({
        "defaultScope": default_scope,
        "weekLabel": index.week_label,
        "courseFilters": course_filters,
        "sourcePriority": SOURCE_PRIORITY,
        "items": items,
        "remoteItems": index.items,
        "lastSyncedAt": index.last_synced_at,
        "warnings": index.warnings,
        "remoteCount": index.items.len(),
        "availableRemoteCount": available_remote_count,
    })
}

fn refresh_remote_index_status(root: &Path) -> Result<RemoteMaterialsIndex, String> {
    let local_items = read_materials(root)?;
    let mut index = read_remote_index(root);
    index.version = REMOTE_INDEX_VERSION;
    index.items = attach_download_status(index.items, &local_items);
    if !index.items.is_empty() || index.last_synced_at.is_some() {
        write_remote_index(root, &index)?;
    }
    Ok(index)
}

pub fn fetch_materials(app: &AppHandle) -> Result<Value, String> {
    let root = materials_root(app)?;
    let items = read_materials(&root)?;
    let index = read_remote_index(&root);
    Ok(build_materials_payload(items, index))
}

pub async fn sync_materials_index(app: &AppHandle, state: &AppState) -> Result<Value, String> {
    let root = materials_root(app)?;
    let local_items = read_materials(&root)?;
    let courses = courses::get_learning_courses(state).await?;
    let mut warnings = Vec::<String>::new();
    let mut remote_items = Vec::<RemoteMaterialAsset>::new();
    let mut seen_learning = HashSet::<(i64, i64)>::new();
    let mut course_ids = Vec::<i64>::new();

    for course in courses {
        let course_id = course.get("id").and_then(Value::as_i64).unwrap_or_default();
        if course_id <= 0 {
            continue;
        }
        course_ids.push(course_id);
        let course_name = first_nonempty_string(&course, &["display_name", "name", "second_name"])
            .unwrap_or("未命名课程")
            .to_string();

        match courses::get_course_activity_uploads(state, course_id).await {
            Ok(uploads) => {
                for upload in uploads {
                    if let Some(item) =
                        normalize_learning_upload(course_id, &course_name, "activity", &upload)
                    {
                        let upload_id = item
                            .remote_ref
                            .get("uploadId")
                            .and_then(Value::as_i64)
                            .unwrap_or_default();
                        let reference_id = item
                            .remote_ref
                            .get("referenceId")
                            .and_then(Value::as_i64)
                            .unwrap_or_default();
                        if seen_learning.insert((upload_id, reference_id)) {
                            remote_items.push(item);
                        }
                    }
                }
            }
            Err(error) => warnings.push(format!("{course_name} 活动资料同步失败: {error}")),
        }

        match courses::get_course_homework_uploads(state, course_id).await {
            Ok(uploads) => {
                for upload in uploads {
                    if let Some(item) =
                        normalize_learning_upload(course_id, &course_name, "homework", &upload)
                    {
                        let upload_id = item
                            .remote_ref
                            .get("uploadId")
                            .and_then(Value::as_i64)
                            .unwrap_or_default();
                        let reference_id = item
                            .remote_ref
                            .get("referenceId")
                            .and_then(Value::as_i64)
                            .unwrap_or_default();
                        if seen_learning.insert((upload_id, reference_id)) {
                            remote_items.push(item);
                        }
                    }
                }
            }
            Err(error) => warnings.push(format!("{course_name} 作业资料同步失败: {error}")),
        }
    }

    match ClassroomSession::login(state).await {
        Ok(session) => match session.fetch_material_subjects(&course_ids).await {
            Ok(result) => {
                if !result.warnings.is_empty() {
                    warnings.extend(result.warnings);
                }
                let updated_at = now_ts();
                for subject in result.items {
                    remote_items.push(normalize_classroom_subject(subject, updated_at));
                }
                let mut index = RemoteMaterialsIndex {
                    version: REMOTE_INDEX_VERSION,
                    items: remote_items,
                    last_synced_at: Some(updated_at),
                    warnings: warnings.clone(),
                    week_label: Some(result.week_label),
                };
                index.items = attach_download_status(index.items, &local_items);
                write_remote_index(&root, &index)?;
                return Ok(build_materials_payload(local_items, index));
            }
            Err(error) => warnings.push(format!("智云课堂资料同步失败: {error}")),
        },
        Err(error) => warnings.push(format!("智云课堂登录失败: {error}")),
    }

    let synced_at = now_ts();
    let index = RemoteMaterialsIndex {
        version: REMOTE_INDEX_VERSION,
        items: attach_download_status(remote_items, &local_items),
        last_synced_at: Some(synced_at),
        warnings: warnings.clone(),
        week_label: None,
    };
    write_remote_index(&root, &index)?;
    Ok(build_materials_payload(local_items, index))
}

pub async fn download_material_asset(
    app: &AppHandle,
    input: DownloadMaterialInput,
) -> Result<Value, String> {
    let root = materials_root(app)?;
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
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取资料内容失败: {error}"))?;
    let asset = write_material_from_bytes(
        &root,
        &input.course_name,
        &input.title,
        input.source.unwrap_or(input.url),
        mime_type,
        &file_name,
        bytes.as_ref(),
    )?;

    let index = refresh_remote_index_status(&root)?;
    Ok(json!({
        "item": asset,
        "remoteItems": index.items,
        "lastSyncedAt": index.last_synced_at,
        "warnings": index.warnings,
    }))
}

async fn cache_learning_remote_item(
    root: &Path,
    state: &AppState,
    item: &RemoteMaterialAsset,
) -> Result<MaterialAsset, String> {
    let upload_id = item
        .remote_ref
        .get("uploadId")
        .and_then(Value::as_i64)
        .ok_or_else(|| "学习资料 uploadId 缺失".to_string())?;
    let reference_id = item
        .remote_ref
        .get("referenceId")
        .and_then(Value::as_i64)
        .ok_or_else(|| "学习资料 referenceId 缺失".to_string())?;
    let response = courses::get_upload_download_response(state, upload_id, reference_id).await?;
    let mime_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let file_name = infer_filename(&item.source_url, &response, Some(&item.file_name));
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取资料内容失败: {error}"))?;
    write_material_from_bytes(
        root,
        &item.course_name,
        &item.title,
        format!("{}#{}", item.source_url, item.source_type),
        mime_type,
        &file_name,
        bytes.as_ref(),
    )
}

async fn cache_classroom_remote_item(
    root: &Path,
    state: &AppState,
    item: &RemoteMaterialAsset,
) -> Result<MaterialAsset, String> {
    let urls = item.preview_image_urls.clone();
    if urls.is_empty() {
        return Err("智云课堂资料缺少 PPT 预览图，暂时无法缓存".to_string());
    }

    let session = ClassroomSession::login(state).await?;
    let course_dir = root.join(sanitize_segment(&item.course_name));
    fs::create_dir_all(&course_dir).map_err(|error| format!("无法创建 Classroom 目录: {error}"))?;
    let asset_path = course_dir.join(&item.file_name);
    let assets_dir = classroom_assets_dir(&asset_path);
    fs::create_dir_all(&assets_dir)
        .map_err(|error| format!("无法创建 Classroom 图片目录: {error}"))?;

    let mut html_lines = vec![
        "<!doctype html>".to_string(),
        "<html lang=\"zh-CN\"><head><meta charset=\"utf-8\" />".to_string(),
        format!("<title>{}</title>", item.title),
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />".to_string(),
        "<style>body{margin:0;padding:16px;background:#0f172a;color:#e2e8f0;font-family:-apple-system,BlinkMacSystemFont,Segoe UI,system-ui,sans-serif;}header{position:sticky;top:0;background:rgba(15,23,42,.88);backdrop-filter:blur(12px);padding:12px 0 16px;z-index:10;}h1{margin:0;font-size:20px;}p{margin:6px 0 0;color:#94a3b8;font-size:13px;}main{display:flex;flex-direction:column;gap:12px;}img{display:block;width:100%;height:auto;border-radius:14px;background:#020617;box-shadow:0 10px 30px rgba(2,6,23,.28);} </style></head><body>".to_string(),
        format!("<header><h1>{}</h1><p>{} · 智云课堂课件缓存</p></header><main>", item.title, item.course_name),
    ];

    for (index, url) in urls.iter().enumerate() {
        let (bytes, mime) = session.download_bytes(url).await?;
        let ext = guess_extension(url, mime.as_deref());
        let file_name = format!("page-{:03}.{}", index + 1, ext);
        let image_path = assets_dir.join(&file_name);
        fs::write(&image_path, bytes)
            .map_err(|error| format!("写入 Classroom 图片失败: {error}"))?;
        let relative = image_path
            .strip_prefix(root)
            .map_err(|error| format!("构建 Classroom 图片相对路径失败: {error}"))?;
        html_lines.push(format!(
            "<img src=\"{}\" alt=\"{} - 第 {} 页\" loading=\"lazy\" />",
            to_unix_path(relative),
            item.title,
            index + 1
        ));
    }

    html_lines.push("</main></body></html>".to_string());
    write_classroom_html_asset(
        root,
        &item.course_name,
        &item.title,
        item.source_url.clone(),
        &item.file_name,
        &html_lines.join("\n"),
    )
}

pub async fn cache_remote_material(
    app: &AppHandle,
    state: &AppState,
    input: RemoteMaterialDownloadInput,
) -> Result<Value, String> {
    let root = materials_root(app)?;
    let index = read_remote_index(&root);
    let item = index
        .items
        .iter()
        .find(|candidate| candidate.id == input.remote_id)
        .cloned()
        .ok_or_else(|| "远程资料不存在，请先重新同步资料索引".to_string())?;

    let asset = match item.source_type.as_str() {
        "classroom" => cache_classroom_remote_item(&root, state, &item).await?,
        _ => cache_learning_remote_item(&root, state, &item).await?,
    };

    let refreshed = refresh_remote_index_status(&root)?;
    Ok(json!({
        "item": asset,
        "remoteItems": refreshed.items,
        "lastSyncedAt": refreshed.last_synced_at,
        "warnings": refreshed.warnings,
    }))
}

pub fn read_material_text(app: &AppHandle, input: MaterialContentInput) -> Result<Value, String> {
    let root = materials_root(app)?;
    let asset_path = resolve_asset_path(&root, &input.relative_path)?;
    if !asset_path.exists() {
        return Err("资料文件不存在".to_string());
    }
    if !is_text_extension(&asset_path) {
        return Err("当前资料不是可直接读取的文本文件，请改用预览或外部打开".to_string());
    }

    let max_chars = input.max_chars.unwrap_or(24_000).clamp(2_000, 120_000);
    let content =
        fs::read_to_string(&asset_path).map_err(|error| format!("读取资料文本失败: {error}"))?;
    let mut chars = content.chars();
    let preview = chars.by_ref().take(max_chars).collect::<String>();
    let truncated = chars.next().is_some();

    Ok(json!({
        "content": preview,
        "truncated": truncated,
        "relativePath": input.relative_path,
        "absolutePath": asset_path.to_string_lossy().to_string(),
    }))
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
    let assets_dir = classroom_assets_dir(&asset_path);
    if asset_path.exists() {
        fs::remove_file(&asset_path).map_err(|error| format!("删除资料失败: {error}"))?;
    }
    if meta_path.exists() {
        fs::remove_file(&meta_path).map_err(|error| format!("删除资料元数据失败: {error}"))?;
    }
    if assets_dir.exists() {
        fs::remove_dir_all(&assets_dir)
            .map_err(|error| format!("删除资料附属文件失败: {error}"))?;
    }
    let _ = refresh_remote_index_status(&root);
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timestamp_supports_number_and_text() {
        assert_eq!(
            parse_timestamp_value(&json!(1700000000)),
            Some(1_700_000_000)
        );
        assert_eq!(
            parse_timestamp_value(&json!("1700000000000")),
            Some(1_700_000_000)
        );
        assert!(parse_timestamp_value(&json!("2026-03-06T12:34:56+08:00")).is_some());
    }

    #[test]
    fn attach_download_status_matches_by_source_or_name() {
        let local = vec![MaterialAsset {
            id: "1".to_string(),
            course_name: "高等数学".to_string(),
            title: "Lecture 1".to_string(),
            file_name: "lecture1.pdf".to_string(),
            relative_path: "高等数学/lecture1.pdf".to_string(),
            absolute_path: "/tmp/lecture1.pdf".to_string(),
            source_url: Some(
                "https://courses.zju.edu.cn/api/uploads/reference/2/blob#activity".to_string(),
            ),
            mime_type: Some("application/pdf".to_string()),
            size_bytes: 100,
            downloaded_at: 1,
            updated_at: 1,
            exists: true,
        }];
        let remote = vec![RemoteMaterialAsset {
            id: "learning:activity:1:1:2".to_string(),
            course_id: 1,
            course_name: "高等数学".to_string(),
            title: "Lecture 1".to_string(),
            file_name: "lecture1.pdf".to_string(),
            source_type: "activity".to_string(),
            source_url: "https://courses.zju.edu.cn/api/uploads/reference/2/blob".to_string(),
            fallback_source_url: "https://courses.zju.edu.cn/api/uploads/1/blob".to_string(),
            mime_type: Some("application/pdf".to_string()),
            size_bytes: 100,
            updated_at: 1,
            downloaded: false,
            local_relative_path: None,
            week_bucket: "unknown".to_string(),
            preview_image_urls: Vec::new(),
            remote_ref: json!({"kind":"learning","uploadId":1,"referenceId":2}),
        }];

        let merged = attach_download_status(remote, &local);
        assert!(merged[0].downloaded);
        assert_eq!(
            merged[0].local_relative_path.as_deref(),
            Some("高等数学/lecture1.pdf")
        );
    }
}
