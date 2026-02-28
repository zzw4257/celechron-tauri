mod courses;
mod zdbk;
mod zjuam;

use serde_json::Value;
use std::sync::Arc;
#[cfg(desktop)]
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
    let practice = practice_r.unwrap_or(zdbk::PracticeScores {
        pt2: 0.0,
        pt3: 0.0,
        pt4: 0.0,
    });

    let (five_point, four_point, hundred_point, total_credits) = calculate_gpa(&transcript);
    let (major_gpa, major_credits) = calculate_major_gpa(&major_grades);

    // Build majorCourseIds set - collect xkkh from major_grades
    let major_course_ids: Vec<String> = major_grades
        .iter()
        .filter_map(|g| {
            g.get("xkkh")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    // Group transcript by semester (xnm = academic year, xqm = term)
    // And enrich each grade with computed GPA fields the frontend template expects
    let mut semester_map: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();
    for grade in &transcript {
        let xnm = grade.get("xnm").and_then(|v| v.as_str()).unwrap_or("未知");
        let xqm = grade.get("xqm").and_then(|v| v.as_str()).unwrap_or("?");
        let term_name = match xqm {
            "1" => format!("{}-{} 秋", xnm, xnm.parse::<u32>().unwrap_or(0) + 1),
            "2" => format!("{}-{} 冬", xnm, xnm.parse::<u32>().unwrap_or(0) + 1),
            "3" => format!("{}-{} 秋冬", xnm, xnm.parse::<u32>().unwrap_or(0) + 1),
            "4" => format!("{}-{} 春", xnm, xnm.parse::<u32>().unwrap_or(0) + 1),
            "8" => format!("{}-{} 夏", xnm, xnm.parse::<u32>().unwrap_or(0) + 1),
            "12" => format!("{}-{} 春夏", xnm, xnm.parse::<u32>().unwrap_or(0) + 1),
            "16" => format!("{}-{} 短学期", xnm, xnm.parse::<u32>().unwrap_or(0) + 1),
            _ => format!(
                "{}-{} 第{}学期",
                xnm,
                xnm.parse::<u32>().unwrap_or(0) + 1,
                xqm
            ),
        };

        // Extract credit (xf field from API)
        let credit = grade
            .get("xf")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| grade.get("xf").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);

        // Parse grade string into numeric score
        let cj_raw = grade
            .get("cj")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let hundred = parse_score(cj_raw);
        let five = to5(hundred);
        let four = to4(hundred);
        let legacy = to4_legacy(five);

        // Build enriched grade object
        let mut enriched = grade.clone();
        if let Some(obj) = enriched.as_object_mut() {
            obj.insert("credit".to_string(), serde_json::json!(credit));
            obj.insert("hundredPoint".to_string(), serde_json::json!(hundred));
            obj.insert("fivePoint".to_string(), serde_json::json!(five));
            obj.insert("fourPoint".to_string(), serde_json::json!(four));
            obj.insert("fourPointLegacy".to_string(), serde_json::json!(legacy));
        }

        semester_map.entry(term_name).or_default().push(enriched);
    }

    // Convert to array ordered by semester (oldest first)
    let semesters: Vec<Value> = semester_map
        .into_iter()
        .map(|(name, grades)| {
            serde_json::json!({
                "name": name,
                "grades": grades,
                "gpaArr": []  // frontend computes via fallback
            })
        })
        .collect();

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
        "majorCourseIds": major_course_ids,
        "semesters": semesters,
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
        let c = g
            .get("xf")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| g.get("xf").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);
        let s = parse_score(g.get("cj").and_then(|v| v.as_str()).unwrap_or(""));
        if s > 0.0 && c > 0.0 {
            tc += c;
            wh += c * s;
            w5 += c * to5(s);
            w4 += c * to4(s);
        }
    }
    if tc == 0.0 {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (w5 / tc, w4 / tc, wh / tc, tc)
    }
}

fn calculate_major_gpa(grades: &[Value]) -> (f64, f64) {
    let mut tc = 0.0_f64;
    let mut w = 0.0_f64;
    for g in grades {
        let c = g
            .get("xf")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| g.get("xf").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);
        let s = parse_score(g.get("cj").and_then(|v| v.as_str()).unwrap_or(""));
        if s > 0.0 && c > 0.0 {
            tc += c;
            w += c * to4(s);
        }
    }
    if tc == 0.0 {
        (0.0, 0.0)
    } else {
        (w / tc, tc)
    }
}

fn parse_score(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or_else(|_| match s {
        "优秀" => 95.0,
        "良好" => 85.0,
        "中等" => 75.0,
        "及格" => 65.0,
        "合格" => 75.0,
        _ => 0.0,
    })
}

fn to5(s: f64) -> f64 {
    if s >= 95.0 {
        5.0
    } else if s >= 92.0 {
        4.8
    } else if s >= 89.0 {
        4.5
    } else if s >= 86.0 {
        4.2
    } else if s >= 83.0 {
        3.9
    } else if s >= 80.0 {
        3.6
    } else if s >= 77.0 {
        3.3
    } else if s >= 74.0 {
        3.0
    } else if s >= 71.0 {
        2.7
    } else if s >= 68.0 {
        2.4
    } else if s >= 65.0 {
        2.1
    } else if s >= 62.0 {
        1.8
    } else if s >= 60.0 {
        1.5
    } else {
        0.0
    }
}

fn to4(s: f64) -> f64 {
    if s >= 85.0 {
        (s - 60.0) * 0.1
    } else if s >= 60.0 {
        (s - 60.0) * 0.06 + 1.5
    } else {
        0.0
    }
}

fn to4_legacy(five: f64) -> f64 {
    if five >= 4.0 {
        4.0
    } else if five >= 3.0 {
        3.0
    } else if five >= 2.0 {
        2.0
    } else if five >= 1.5 {
        1.5
    } else {
        0.0
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(AppState::new());
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(
            |app: &tauri::AppHandle, _argv, _cwd| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            },
        ));
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        builder = builder.plugin(tauri_plugin_biometric::init());
    }

    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
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
