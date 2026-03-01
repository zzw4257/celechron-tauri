use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn envelope(data: Value, source: &str) -> Value {
    json!({
        "data": data,
        "_meta": {
            "source": source,
            "timestamp": now_ts()
        }
    })
}

fn get_cache_path(app: &AppHandle, filename: &str) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|dir| {
            let _ = fs::create_dir_all(&dir);
            dir.join(filename)
        })
        .ok()
}

pub fn cache_write_envelope(app: &AppHandle, filename: &str, env: &Value) {
    if let Some(path) = get_cache_path(app, filename) {
        if let Ok(json_str) = serde_json::to_string(env) {
            let _ = fs::write(path, json_str);
        }
    }
}

pub fn cache_read_envelope(app: &AppHandle, filename: &str) -> Option<Value> {
    let path = get_cache_path(app, filename)?;
    let json_str = fs::read_to_string(path).ok()?;
    let mut val = serde_json::from_str::<Value>(&json_str).ok()?;

    // New format: { data: ..., _meta: ... }
    if val.get("data").is_some() {
        if let Some(obj) = val.as_object_mut() {
            let ts = obj
                .get("_meta")
                .and_then(|m| m.get("timestamp"))
                .and_then(|t| t.as_u64())
                .unwrap_or_else(now_ts);
            obj.insert(
                "_meta".to_string(),
                json!({
                    "source": "cache",
                    "timestamp": ts,
                }),
            );
        }
        return Some(val);
    }

    // Old format with top-level _meta + payload fields
    if let Some(obj) = val.as_object_mut() {
        let old_meta = obj.remove("_meta");
        let ts = old_meta
            .as_ref()
            .and_then(|m| m.get("timestamp"))
            .and_then(|t| t.as_u64())
            .unwrap_or_else(now_ts);

        return Some(json!({
            "data": Value::Object(obj.clone()),
            "_meta": {
                "source": "cache",
                "timestamp": ts,
            }
        }));
    }

    // Old array / scalar cache
    Some(envelope(val, "cache"))
}
