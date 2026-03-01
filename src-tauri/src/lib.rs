mod api;
mod courses;
mod gpa;
mod zdbk;
mod zjuam;

use crate::api::{cache_read_envelope, cache_write_envelope, envelope};
use crate::gpa::{
    apply_simulated_score, compute_gpa_by_policy, enrich_grade, extract_semester_name, RetakePolicy,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
#[cfg(desktop)]
use tauri::Manager;
use tauri::{AppHandle, State};
use zjuam::AppState;

fn normalize_academic_semester(semester: &str) -> Option<&'static str> {
    match semester.trim() {
        "1" | "3" => Some("1"),
        "2" | "12" => Some("2"),
        _ => None,
    }
}

fn to_timetable_semester(academic_semester: &str) -> &'static str {
    match academic_semester {
        "2" => "12",
        _ => "3",
    }
}

#[tauri::command]
async fn login_zju_command(
    state: State<'_, Arc<AppState>>,
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
async fn fetch_scholar_data(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Value, String> {
    let (transcript_r, major_r, exams_r, practice_r) = tokio::join!(
        zdbk::get_transcript(&state),
        zdbk::get_major_grades(&state),
        zdbk::get_exams(&state),
        zdbk::get_practice_scores(&state),
    );

    if let Err(err) = transcript_r {
        if let Some(cached) = cache_read_envelope(&app, "cache_scholar.json") {
            return Ok(cached);
        }
        return Err(err);
    }

    let transcript_raw = transcript_r.unwrap_or_default();
    let major_grades = major_r.unwrap_or_default();
    let exams = exams_r.unwrap_or_default();
    let practice = practice_r.unwrap_or(zdbk::PracticeScores {
        pt2: 0.0,
        pt3: 0.0,
        pt4: 0.0,
    });

    let processed_grades = transcript_raw.iter().map(enrich_grade).collect::<Vec<_>>();

    let major_course_ids = collect_major_course_ids(&major_grades);
    let major_course_set = major_course_ids.iter().cloned().collect::<HashSet<_>>();

    let overall_first = compute_gpa_by_policy(&processed_grades, &major_course_set, RetakePolicy::First);
    let overall_highest =
        compute_gpa_by_policy(&processed_grades, &major_course_set, RetakePolicy::Highest);

    let mut semesters_map: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for grade in processed_grades.iter() {
        let sem_name = extract_semester_name(grade).unwrap_or_else(|| "其他/认定".to_string());
        semesters_map
            .entry(sem_name)
            .or_default()
            .push(grade.clone());
    }

    let semesters = semesters_map
        .into_iter()
        .map(|(name, grades)| {
            let sem_first = compute_gpa_by_policy(&grades, &major_course_set, RetakePolicy::First);
            let sem_highest =
                compute_gpa_by_policy(&grades, &major_course_set, RetakePolicy::Highest);
            json!({
                "name": name,
                "grades": grades,
                "gpaByPolicy": {
                    "first": sem_first,
                    "highest": sem_highest,
                },
                "gpa": [
                    sem_first.five_point,
                    sem_first.four_point,
                    sem_first.four_point_legacy,
                    sem_first.hundred_point,
                ],
                "credits": sem_first.total_credits,
            })
        })
        .collect::<Vec<_>>();

    let payload = json!({
        "gpa": overall_first,
        "gpaByPolicy": {
            "first": overall_first,
            "highest": overall_highest,
        },
        "transcript": processed_grades,
        "majorGrades": major_grades,
        "majorCourseIds": major_course_ids,
        "exams": exams,
        "practice": {
            "pt2": practice.pt2,
            "pt3": practice.pt3,
            "pt4": practice.pt4,
        },
        "semesters": semesters,
    });

    let env = envelope(payload, "network");
    cache_write_envelope(&app, "cache_scholar.json", &env);
    Ok(env)
}

#[tauri::command]
async fn fetch_timetable(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    year: String,
    semester: String,
) -> Result<Value, String> {
    let academic_semester = normalize_academic_semester(&semester)
        .ok_or_else(|| format!("不支持的学期参数: {}", semester))?;
    let timetable_semester = to_timetable_semester(academic_semester);

    let cache_name = format!("cache_timetable_{}_{}.json", year, academic_semester);
    let legacy_cache_name = format!("cache_timetable_{}_{}.json", year, semester);

    match zdbk::get_timetable(&state, &year, timetable_semester).await {
        Ok(arr) => {
            let env = envelope(
                json!({
                    "timetable": arr,
                    "year": year,
                    "semester": academic_semester,
                    "xqm": timetable_semester,
                }),
                "network",
            );
            cache_write_envelope(&app, &cache_name, &env);
            Ok(env)
        }
        Err(e) => {
            if let Some(cached) = cache_read_envelope(&app, &cache_name) {
                return Ok(cached);
            }
            if legacy_cache_name != cache_name {
                if let Some(cached) = cache_read_envelope(&app, &legacy_cache_name) {
                    return Ok(cached);
                }
            }
            Err(e)
        }
    }
}

#[tauri::command]
async fn fetch_todos(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    match courses::get_todos(&state).await {
        Ok(data) => {
            let env = envelope(data, "network");
            cache_write_envelope(&app, "cache_todos.json", &env);
            Ok(env)
        }
        Err(e) => {
            if let Some(cached) = cache_read_envelope(&app, "cache_todos.json") {
                return Ok(cached);
            }
            Err(e)
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpaPreviewInput {
    grades: Vec<Value>,
    selected_ids: Option<Vec<String>>,
    simulated_scores: Option<HashMap<String, f64>>,
    retake_policy: Option<String>,
    major_course_ids: Option<Vec<String>>,
}

#[tauri::command]
fn calculate_gpa_preview(input: GpaPreviewInput) -> Result<Value, String> {
    let selected_set = input
        .selected_ids
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let has_selection = !selected_set.is_empty();

    let simulated = input.simulated_scores.unwrap_or_default();
    let major_set = input
        .major_course_ids
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();

    let mut grades = Vec::new();
    for raw in input.grades.iter() {
        let mut grade = enrich_grade(raw);
        let xkkh = grade
            .get("xkkh")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if has_selection && !selected_set.contains(&xkkh) {
            continue;
        }

        let cj = grade
            .get("cj")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();

        if ["待录", "缓考", "无效"].contains(&cj.as_str()) {
            if let Some(score) = simulated.get(&xkkh) {
                apply_simulated_score(&mut grade, *score);
            }
        }

        grades.push(grade);
    }

    let policy = RetakePolicy::from_str(input.retake_policy.as_deref().unwrap_or("first"));
    let summary = compute_gpa_by_policy(&grades, &major_set, policy);
    Ok(json!(summary))
}

fn collect_major_course_ids(major_grades: &[Value]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    for grade in major_grades {
        if let Some(xkkh) = grade.get("xkkh").and_then(|v| v.as_str()) {
            if seen.insert(xkkh.to_string()) {
                ids.push(xkkh.to_string());
            }
        }
        if let Some(kcdm) = grade.get("kcdm").and_then(|v| v.as_str()) {
            if seen.insert(kcdm.to_string()) {
                ids.push(kcdm.to_string());
            }
        }
    }

    ids
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
            calculate_gpa_preview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{normalize_academic_semester, to_timetable_semester};

    #[test]
    fn semester_aliases_are_normalized() {
        assert_eq!(normalize_academic_semester("1"), Some("1"));
        assert_eq!(normalize_academic_semester("3"), Some("1"));
        assert_eq!(normalize_academic_semester("2"), Some("2"));
        assert_eq!(normalize_academic_semester("12"), Some("2"));
    }

    #[test]
    fn timetable_semester_mapping_is_stable() {
        assert_eq!(to_timetable_semester("1"), "3");
        assert_eq!(to_timetable_semester("2"), "12");
    }

    #[test]
    fn invalid_semester_is_rejected() {
        assert_eq!(normalize_academic_semester(""), None);
        assert_eq!(normalize_academic_semester("0"), None);
        assert_eq!(normalize_academic_semester("spring"), None);
    }
}
