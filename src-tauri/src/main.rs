// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod courses;
mod zdbk;
mod zjuam;

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};
use zjuam::AppState;

#[tauri::command]
async fn login_zju_command(
    state: State<'_, Arc<AppState>>,
    username: String,
    password: String,
) -> Result<String, String> {
    // Step 1: CAS login
    zjuam::login_zju(&state, &username, &password).await?;

    // Step 2: Login to downstream services in parallel
    let zdbk_result = zdbk::login_zdbk(&state).await;
    let courses_result = courses::login_courses(&state).await;

    let mut warnings = Vec::new();
    if let Err(e) = zdbk_result {
        warnings.push(format!("教务网: {}", e));
    }
    if let Err(e) = courses_result {
        warnings.push(format!("学在浙大: {}", e));
    }

    if warnings.is_empty() {
        Ok("登录成功".to_string())
    } else {
        Ok(format!("登录成功，但部分服务异常: {}", warnings.join("; ")))
    }
}

#[tauri::command]
// Helper function to read/write cache
fn get_cache_path(app: &AppHandle, filename: &str) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|dir| {
            let _ = fs::create_dir_all(&dir);
            dir.join(filename)
        })
        .ok()
}

fn write_cache(app: &AppHandle, filename: &str, data: &Value) {
    if let Some(path) = get_cache_path(app, filename) {
        let mut cache_data = data.clone();
        if let Some(obj) = cache_data.as_object_mut() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            obj.insert(
                "_meta".to_string(),
                serde_json::json!({
                    "source": "cache",
                    "timestamp": now
                }),
            );
        } else if let Some(arr) = cache_data.as_array_mut() {
            // For arrays, we can't easily append a _meta key.
            // In our system, timetable is an array, we might just warp it or let frontend handle.
            // Actually, best to wrap array responses in an object. But to keep it simple and backwards compatible:
            // Just write raw array, but the frontend will know if it failed.
        }
        if let Ok(json_str) = serde_json::to_string(&cache_data) {
            let _ = fs::write(path, json_str);
        }
    }
}

fn read_cache(app: &AppHandle, filename: &str) -> Option<Value> {
    if let Some(path) = get_cache_path(app, filename) {
        if let Ok(json_str) = fs::read_to_string(path) {
            if let Ok(val) = serde_json::from_str::<Value>(&json_str) {
                return Some(val);
            }
        }
    }
    None
}

#[tauri::command]
async fn fetch_scholar_data(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Value, String> {
    // Fetch transcript, major grades, exams, practice scores in parallel
    let (transcript_r, major_r, exams_r, practice_r) = tokio::join!(
        zdbk::get_transcript(&state),
        zdbk::get_major_grades(&state),
        zdbk::get_exams(&state),
        zdbk::get_practice_scores(&state),
    );

    // Network Failure Check: If transcript fails, fallback to cache completely
    if transcript_r.is_err() {
        if let Some(cached) = read_cache(&app, "cache_scholar.json") {
            return Ok(cached);
        }
        return Err(transcript_r.err().unwrap());
    }

    let transcript = transcript_r.unwrap_or_default();
    let major_grades = major_r.unwrap_or_default();
    let exams = exams_r.unwrap_or_default();
    let practice = practice_r.unwrap_or(zdbk::PracticeScores {
        pt2: 0.0,
        pt3: 0.0,
        pt4: 0.0,
    });

    let (five_point, four_point, four_point_legacy, hundred_point, total_credits) =
        calculate_gpa(&transcript);
    let (major_gpa_43, major_gpa_legacy, major_credits) = calculate_major_gpa(&major_grades);

    // Provide complete grade objects with computed fields
    let mut processed_grades = Vec::new();
    for grade in &transcript {
        let mut g = grade.clone();
        if let Some(obj) = g.as_object_mut() {
            let credit = grade
                .get("xf")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .or_else(|| grade.get("xf").and_then(|v| v.as_f64()))
                .unwrap_or(0.0);

            let score_str = grade.get("cj").and_then(|v| v.as_str()).unwrap_or("");
            let hundred_p = parse_score(score_str);

            let five_p = grade
                .get("jd")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(to_five_point(hundred_p));

            obj.insert("credit".to_string(), serde_json::json!(credit));
            obj.insert("fivePoint".to_string(), serde_json::json!(five_p));
            obj.insert(
                "fourPoint".to_string(),
                serde_json::json!(to_four_point_43(five_p)),
            );
            obj.insert(
                "fourPointLegacy".to_string(),
                serde_json::json!(to_four_point_legacy(five_p)),
            );
            obj.insert("hundredPoint".to_string(), serde_json::json!(hundred_p));
        }
        processed_grades.push(g);
    }

    // Group grades by semester
    let mut semesters_map: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();
    for grade in processed_grades {
        if let Some(id) = grade.get("xkkh").and_then(|v| v.as_str()) {
            if id.len() >= 13 {
                let sem_key = &id[1..12]; // e.g. "2024-2025-2"
                semesters_map
                    .entry(sem_key.to_string())
                    .or_default()
                    .push(grade);
            }
        }
    }

    let mut major_course_ids = Vec::new();
    for grade in &major_grades {
        if let Some(id) = grade.get("xkkh").and_then(|v| v.as_str()) {
            major_course_ids.push(id.to_string());
        }
        if let Some(id) = grade.get("kcdm").and_then(|v| v.as_str()) {
            // Also push kcdm just in case xkkh is missing or frontend match relies on it
            major_course_ids.push(id.to_string());
        }
    }

    let mut semesters_list = Vec::new();
    for (name, grades) in semesters_map {
        let (s_five, s_four, s_legacy, s_hundred, s_credits) = calculate_gpa(&grades);
        semesters_list.push(serde_json::json!({
            "name": name,
            "grades": grades,
            "gpa": [s_five, s_four, s_legacy, s_hundred],
            "credits": s_credits
        }));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let result = serde_json::json!({
        "gpa": {
            "fivePoint": five_point,
            "fourPoint": four_point,
            "fourPointLegacy": four_point_legacy,
            "hundredPoint": hundred_point,
            "totalCredits": total_credits,
            "majorGpa": major_gpa_43,
            "majorGpaLegacy": major_gpa_legacy,
            "majorCredits": major_credits,
        },
        "transcript": transcript,
        "majorGrades": major_grades,
        "majorCourseIds": major_course_ids,
        "exams": exams,
        "practice": {
            "pt2": practice.pt2,
            "pt3": practice.pt3,
            "pt4": practice.pt4,
        },
        "semesters": semesters_list,
        "_meta": {
            "source": "network",
            "timestamp": now
        }
    });

    write_cache(&app, "cache_scholar.json", &result);
    Ok(result)
}

#[tauri::command]
async fn fetch_timetable(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    year: String,
    semester: String,
) -> Result<Value, String> {
    let cache_name = format!("cache_timetable_{}_{}.json", year, semester);
    match zdbk::get_timetable(&state, &year, &semester).await {
        Ok(arr) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let result = serde_json::json!({
                "timetable": arr,
                "_meta": {
                    "source": "network",
                    "timestamp": now
                }
            });
            write_cache(&app, &cache_name, &result);
            Ok(result)
        }
        Err(e) => {
            if let Some(cached) = read_cache(&app, &cache_name) {
                return Ok(cached);
            }
            Err(e)
        }
    }
}

#[tauri::command]
async fn fetch_todos(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    match courses::get_todos(&state).await {
        Ok(mut res) => {
            if let Some(obj) = res.as_object_mut() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                obj.insert(
                    "_meta".to_string(),
                    serde_json::json!({
                        "source": "network",
                        "timestamp": now
                    }),
                );
            }
            write_cache(&app, "cache_todos.json", &res);
            Ok(res)
        }
        Err(e) => {
            if let Some(cached) = read_cache(&app, "cache_todos.json") {
                return Ok(cached);
            }
            Err(e)
        }
    }
}

/// Calculate GPA from transcript items (ZJU style).
fn calculate_gpa(grades: &[Value]) -> (f64, f64, f64, f64, f64) {
    let mut total_credits = 0.0_f64;
    let mut weighted_five = 0.0_f64;
    let mut weighted_four = 0.0_f64;
    let mut weighted_legacy = 0.0_f64;
    let mut weighted_hundred = 0.0_f64;

    for grade in grades {
        let credit = grade
            .get("xf")
            .and_then(|v| v.as_f64())
            .or_else(|| {
                grade
                    .get("xf")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .or_else(|| grade.get("credit").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);

        let hundred_p = grade
            .get("hundredPoint")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| {
                let score_str = grade.get("cj").and_then(|v| v.as_str()).unwrap_or("");
                parse_score(score_str)
            });

        let five_p = grade
            .get("fivePoint")
            .and_then(|v| v.as_f64())
            .or_else(|| grade.get("jd").and_then(|v| v.as_f64()))
            .or_else(|| {
                grade
                    .get("jd")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .unwrap_or_else(|| to_five_point(hundred_p));

        // Skip non-GPA items
        let score_str = grade.get("cj").and_then(|v| v.as_str()).unwrap_or("");
        if score_str == "弃修"
            || score_str == "待录"
            || score_str == "缓考"
            || score_str == "无效"
            || score_str == "合格"
            || score_str == "不合格"
        {
            continue;
        }

        if credit > 0.0 {
            total_credits += credit;
            weighted_hundred += credit * hundred_p;
            weighted_five += credit * five_p;
            weighted_four += credit * to_four_point_43(five_p);
            weighted_legacy += credit * to_four_point_legacy(five_p);
        }
    }

    if total_credits == 0.0 {
        return (0.0, 0.0, 0.0, 0.0, 0.0);
    }

    (
        weighted_five / total_credits,
        weighted_four / total_credits,
        weighted_legacy / total_credits,
        weighted_hundred / total_credits,
        total_credits,
    )
}

fn calculate_major_gpa(grades: &[Value]) -> (f64, f64, f64) {
    let mut total_credits = 0.0_f64;
    let mut weighted_43 = 0.0_f64;
    let mut weighted_legacy = 0.0_f64;
    let mut seen_kcdm = std::collections::HashSet::new();

    for grade in grades {
        let score_str = grade
            .get("cj")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        // Skip un-taken courses from the training plan, and other non-GPA items
        if score_str.is_empty()
            || score_str == "弃修"
            || score_str == "待录"
            || score_str == "缓考"
            || score_str == "无效"
            || score_str == "合格"
            || score_str == "不合格"
        {
            continue;
        }

        let kcdm = grade.get("kcdm").and_then(|v| v.as_str()).unwrap_or("");
        if !kcdm.is_empty() && !seen_kcdm.insert(kcdm.to_string()) {
            continue; // Deduplicate returning credits for same course
        }

        let credit = grade
            .get("xf")
            .and_then(|v| v.as_f64())
            .or_else(|| {
                grade
                    .get("xf")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .or_else(|| grade.get("credit").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);

        let hundred_p = grade
            .get("hundredPoint")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| parse_score(score_str));

        let five_p = grade
            .get("fivePoint")
            .and_then(|v| v.as_f64())
            .or_else(|| grade.get("jd").and_then(|v| v.as_f64()))
            .or_else(|| {
                grade
                    .get("jd")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .unwrap_or_else(|| to_five_point(hundred_p));

        if credit > 0.0 {
            total_credits += credit;
            weighted_43 += credit * to_four_point_43(five_p);
            weighted_legacy += credit * to_four_point_legacy(five_p);
        }
    }

    if total_credits == 0.0 {
        return (0.0, 0.0, 0.0);
    }

    (
        weighted_43 / total_credits,
        weighted_legacy / total_credits,
        total_credits,
    )
}

fn parse_score(s: &str) -> f64 {
    let mapping = [
        ("A+", 95.0),
        ("A", 90.0),
        ("A-", 87.0),
        ("B+", 83.0),
        ("B", 80.0),
        ("B-", 77.0),
        ("C+", 73.0),
        ("C", 70.0),
        ("C-", 67.0),
        ("D+", 63.0),
        ("D", 60.0),
        ("F", 0.0),
        ("优秀", 90.0),
        ("良好", 80.0),
        ("中等", 70.0),
        ("及格", 60.0),
        ("不及格", 0.0),
        ("合格", 75.0),
        ("不合格", 0.0),
    ];
    for (k, v) in mapping.iter() {
        if s == *k {
            return *v;
        }
    }
    // Extract numbers if any
    let re = regex::Regex::new(r"\d+").unwrap();
    if let Some(caps) = re.captures(s) {
        if let Ok(v) = caps[0].parse::<f64>() {
            return v;
        }
    }
    0.0
}

fn to_five_point(score: f64) -> f64 {
    if score >= 95.0 {
        5.0
    } else if score >= 92.0 {
        4.8
    } else if score >= 89.0 {
        4.5
    } else if score >= 86.0 {
        4.2
    } else if score >= 83.0 {
        3.9
    } else if score >= 80.0 {
        3.6
    } else if score >= 77.0 {
        3.3
    } else if score >= 74.0 {
        3.0
    } else if score >= 71.0 {
        2.7
    } else if score >= 68.0 {
        2.4
    } else if score >= 65.0 {
        2.1
    } else if score >= 62.0 {
        1.8
    } else if score >= 60.0 {
        1.5
    } else {
        0.0
    }
}

fn to_four_point_43(five_point: f64) -> f64 {
    if five_point > 4.0 {
        if five_point >= 5.0 {
            4.3
        } else if five_point >= 4.8 {
            4.2
        } else if five_point >= 4.5 {
            4.1
        } else {
            4.0
        }
    } else {
        five_point
    }
}

fn to_four_point_legacy(five_point: f64) -> f64 {
    if five_point > 4.0 {
        4.0
    } else {
        five_point
    }
}

fn main() {
    let app_state = Arc::new(AppState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_biometric::init())
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            login_zju_command,
            fetch_scholar_data,
            fetch_timetable,
            fetch_todos,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_full_api_flow() {
        let state = Arc::new(AppState::new());
        let username = std::env::var("ZJU_USERNAME").expect("ZJU_USERNAME env var not set");
        let password = std::env::var("ZJU_PASSWORD").expect("ZJU_PASSWORD env var not set");

        println!("--- 1. Login to ZJU AM ---");
        let iplanet = zjuam::login_zju(&state, &username, &password).await;
        println!("Login ZJU AM Result: {:?}", iplanet);
        assert!(iplanet.is_ok());

        println!("\n--- 2. Login to ZDBK ---");
        let zdbk_res = zdbk::login_zdbk(&state).await;
        println!("Login ZDBK Result: {:?}", zdbk_res);
        println!("JSESSIONID: {:?}", state.zdbk_jsessionid.lock().await);
        println!("route: {:?}", state.zdbk_route.lock().await);

        if zdbk_res.is_ok() {
            println!("\n--- 3. Fetch Transcript & GPA ---");
            let transcript = zdbk::get_transcript(&state).await;
            match transcript {
                Ok(grades) => {
                    println!("Transcript fetched {} course(s).", grades.len());
                    let gpa = calculate_gpa(&grades);
                    println!("GPA Result (5.0, 4.3, 4.0, 100, credits): {:?}", gpa);
                }
                Err(e) => println!("Transcript Error: {}", e),
            }

            println!("\n--- 3.5 Fetch Major Grades & GPA ---");
            let major_grades = zdbk::get_major_grades(&state).await.unwrap_or_default();
            let major_gpa = calculate_major_gpa(&major_grades);
            println!("Major GPA (4-point, major_credits): {:?}", major_gpa);

            println!("\n--- 4. Fetch Timetable for 2024-2 ---");
            let timetable = zdbk::get_timetable(&state, "2024", "2").await;
            match timetable {
                Ok(sessions) => println!(
                    "Timetable fetched {} session(s). First session: {:?}",
                    sessions.len(),
                    sessions.first()
                ),
                Err(e) => println!("Timetable Error: {}", e),
            }
        }

        println!("\n--- 5. Login to Courses ---");
        let courses_res = courses::login_courses(&state).await;
        println!("Login Courses Result: {:?}", courses_res);
        println!("Courses Session: {:?}", state.courses_session.lock().await);

        if courses_res.is_ok() {
            println!("\n--- 6. Fetch Todos ---");
            let todos = courses::get_todos(&state).await;
            match todos {
                Ok(val) => println!(
                    "Todos fetched successfully. Keys: {:?}",
                    val.as_object().map(|o| o.keys().collect::<Vec<_>>())
                ),
                Err(e) => println!("Todos Error: {}", e),
            }
        }
    }
}
