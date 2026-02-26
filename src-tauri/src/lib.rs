mod zjuam;
mod zdbk;
mod courses;

use std::sync::Arc;
use serde_json::Value;
use tauri::Manager;
use zjuam::AppState;

#[tauri::command]
async fn login_zju_command(
    state: tauri::State<'_, Arc<AppState>>,
    username: String,
    password: String,
) -> Result<String, String> {
    zjuam::login_zju(&state, &username, &password).await?;

    let zdbk_result = zdbk::login_zdbk(&state).await;
    let courses_result = courses::login_courses(&state).await;

    let mut warnings = Vec::new();
    if let Err(e) = zdbk_result { warnings.push(format!("教务网: {}", e)); }
    if let Err(e) = courses_result { warnings.push(format!("学在浙大: {}", e)); }

    if warnings.is_empty() {
        Ok("登录成功".to_string())
    } else {
        Ok(format!("登录成功，但部分服务异常: {}", warnings.join("; ")))
    }
}

#[tauri::command]
async fn fetch_scholar_data(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, String> {
    let (transcript_r, major_r, exams_r, practice_r) = tokio::join!(
        zdbk::get_transcript(&state),
        zdbk::get_major_grades(&state),
        zdbk::get_exams(&state),
        zdbk::get_practice_scores(&state),
    );

    let transcript = transcript_r.unwrap_or_default();
    let major_grades = major_r.unwrap_or_default();
    let exams = exams_r.unwrap_or_default();
    let practice = practice_r.unwrap_or(zdbk::PracticeScores { pt2: 0.0, pt3: 0.0, pt4: 0.0 });

    let (five_point, four_point, hundred_point, total_credits) = calculate_gpa(&transcript);
    let (major_gpa, major_credits) = calculate_major_gpa(&major_grades);

    Ok(serde_json::json!({
        "gpa": {
            "fivePoint": five_point,
            "fourPoint": four_point,
            "hundredPoint": hundred_point,
            "totalCredits": total_credits,
            "majorGpa": major_gpa,
            "majorCredits": major_credits,
        },
        "transcript": transcript,
        "majorGrades": major_grades,
        "exams": exams,
        "practice": { "pt2": practice.pt2, "pt3": practice.pt3, "pt4": practice.pt4 },
    }))
}

#[tauri::command]
async fn fetch_timetable(
    state: tauri::State<'_, Arc<AppState>>,
    year: String,
    semester: String,
) -> Result<Vec<Value>, String> {
    zdbk::get_timetable(&state, &year, &semester).await
}

#[tauri::command]
async fn fetch_todos(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, String> {
    courses::get_todos(&state).await
}

fn calculate_gpa(grades: &[Value]) -> (f64, f64, f64, f64) {
    let mut tc = 0.0_f64;
    let mut w5 = 0.0_f64;
    let mut w4 = 0.0_f64;
    let mut wh = 0.0_f64;
    for g in grades {
        let c = g.get("xf").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
            .or_else(|| g.get("xf").and_then(|v| v.as_f64())).unwrap_or(0.0);
        let s = parse_score(g.get("cj").and_then(|v| v.as_str()).unwrap_or(""));
        if s > 0.0 && c > 0.0 { tc += c; wh += c * s; w5 += c * to5(s); w4 += c * to4(s); }
    }
    if tc == 0.0 { (0.0, 0.0, 0.0, 0.0) } else { (w5/tc, w4/tc, wh/tc, tc) }
}

fn calculate_major_gpa(grades: &[Value]) -> (f64, f64) {
    let mut tc = 0.0_f64;
    let mut w = 0.0_f64;
    for g in grades {
        let c = g.get("xf").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
            .or_else(|| g.get("xf").and_then(|v| v.as_f64())).unwrap_or(0.0);
        let s = parse_score(g.get("cj").and_then(|v| v.as_str()).unwrap_or(""));
        if s > 0.0 && c > 0.0 { tc += c; w += c * to4(s); }
    }
    if tc == 0.0 { (0.0, 0.0) } else { (w/tc, tc) }
}

fn parse_score(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or_else(|_| match s {
        "优秀" => 95.0, "良好" => 85.0, "中等" => 75.0, "及格" => 65.0,
        "合格" => 75.0, _ => 0.0,
    })
}

fn to5(s: f64) -> f64 {
    if s >= 95.0 { 5.0 } else if s >= 92.0 { 4.8 } else if s >= 89.0 { 4.5 }
    else if s >= 86.0 { 4.2 } else if s >= 83.0 { 3.9 } else if s >= 80.0 { 3.6 }
    else if s >= 77.0 { 3.3 } else if s >= 74.0 { 3.0 } else if s >= 71.0 { 2.7 }
    else if s >= 68.0 { 2.4 } else if s >= 65.0 { 2.1 } else if s >= 62.0 { 1.8 }
    else if s >= 60.0 { 1.5 } else { 0.0 }
}

fn to4(s: f64) -> f64 {
    if s >= 85.0 { (s - 60.0) * 0.1 } else if s >= 60.0 { (s - 60.0) * 0.06 + 1.5 } else { 0.0 }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(AppState::new());
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app: &tauri::AppHandle, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));
    }

    builder
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
