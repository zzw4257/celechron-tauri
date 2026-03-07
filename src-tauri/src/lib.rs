mod api;
mod classroom;
mod courses;
mod gpa;
mod integrations;
mod materials;
mod term;
mod zdbk;
mod zjuam;

use crate::api::{cache_read_envelope, cache_write_envelope, envelope};
use crate::gpa::{
    apply_simulated_score, compute_gpa_by_policy, enrich_grade, extract_semester_name, RetakePolicy,
};
use crate::integrations::{AiAnalysisInput, DingtalkTestInput};
use crate::materials::{
    DownloadMaterialInput, MaterialContentInput, MaterialPathInput, RemoteMaterialDownloadInput,
};
use crate::term::{
    descriptor_from_name, descriptor_from_parts, load_term_time_config,
    normalize_academic_semester, normalize_timetable_sessions,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const SCHOLAR_CACHE_FILE: &str = "cache_scholar_v2.json";
const TODOS_CACHE_FILE: &str = "cache_todos_v2.json";
use std::sync::Arc;
#[cfg(desktop)]
use tauri::Manager;
use tauri::{AppHandle, State};
use zjuam::AppState;

#[tauri::command]
async fn login_zju_command(
    state: State<'_, Arc<AppState>>,
    username: String,
    password: String,
) -> Result<String, String> {
    zjuam::login_zju(&state, &username, &password).await?;

    let zdbk_result = zdbk::login_zdbk(&state).await;
    let courses_result = courses::login_courses(&state).await;
    let classroom_result = classroom::ClassroomSession::login(&state).await;

    let mut warnings = Vec::new();
    if let Err(error) = zdbk_result {
        warnings.push(format!("教务网: {error}"));
    }
    if let Err(error) = courses_result {
        warnings.push(format!("学在浙大: {error}"));
    }
    if let Err(error) = classroom_result {
        warnings.push(format!("智云课堂: {error}"));
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

    if let Err(error) = transcript_r {
        if let Some(cached) = cache_read_envelope(&app, SCHOLAR_CACHE_FILE) {
            return Ok(cached);
        }
        return Err(error);
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

    let overall_first =
        compute_gpa_by_policy(&processed_grades, &major_course_set, RetakePolicy::First);
    let overall_highest =
        compute_gpa_by_policy(&processed_grades, &major_course_set, RetakePolicy::Highest);

    let mut semesters_map = HashMap::<String, Vec<Value>>::new();
    for grade in &processed_grades {
        let semester_name = extract_semester_name(grade).unwrap_or_else(|| "其他/认定".to_string());
        semesters_map
            .entry(semester_name)
            .or_default()
            .push(grade.clone());
    }

    let mut semesters = semesters_map
        .into_iter()
        .map(|(name, mut grades)| {
            grades.sort_by(|left, right| {
                left.get("kcmc")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .cmp(right.get("kcmc").and_then(Value::as_str).unwrap_or_default())
            });
            let descriptor = descriptor_from_name(&name);
            let first = compute_gpa_by_policy(&grades, &major_course_set, RetakePolicy::First);
            let highest = compute_gpa_by_policy(&grades, &major_course_set, RetakePolicy::Highest);
            let term = descriptor.clone().map(|value| json!(value)).unwrap_or(Value::Null);
            let display_name = descriptor
                .as_ref()
                .map(|value| value.display_name.clone())
                .unwrap_or_else(|| name.clone());
            let rank = descriptor
                .as_ref()
                .map(|value| value.year.parse::<i64>().unwrap_or_default() * 100 + value.academic_semester.parse::<i64>().unwrap_or_default())
                .unwrap_or(-1);
            (
                rank,
                json!({
                    "name": name,
                    "displayName": display_name,
                    "term": term,
                    "grades": grades,
                    "gpaByPolicy": {
                        "first": first,
                        "highest": highest,
                    },
                    "gpa": [first.five_point, first.four_point, first.four_point_legacy, first.hundred_point],
                    "credits": first.total_credits,
                }),
            )
        })
        .collect::<Vec<_>>();

    semesters.sort_by(|left, right| right.0.cmp(&left.0));
    let semesters = semesters
        .into_iter()
        .map(|(_, value)| value)
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
        "retakePolicySupported": ["first", "highest"],
        "exams": exams,
        "practice": {
            "pt2": practice.pt2,
            "pt3": practice.pt3,
            "pt4": practice.pt4,
        },
        "semesters": semesters,
    });

    let env = envelope(payload, "network");
    cache_write_envelope(&app, SCHOLAR_CACHE_FILE, &env);
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
        .ok_or_else(|| format!("不支持的学期参数: {semester}"))?;
    let term = descriptor_from_parts(year.clone(), academic_semester);
    let cache_name = format!(
        "cache_timetable_v2_{}_{}.json",
        term.year, term.academic_semester
    );

    match zdbk::get_timetable(&state, &term.year, &term.timetable_semester).await {
        Ok(raw_timetable) => {
            let time_config = load_term_time_config(&app, &term).await;
            let sessions = normalize_timetable_sessions(&term, &raw_timetable);
            let env = envelope(
                json!({
                    "term": term,
                    "displayName": term.display_name,
                    "year": term.year,
                    "semester": term.academic_semester,
                    "xqm": term.timetable_semester,
                    "timeConfig": time_config,
                    "sessions": sessions,
                    "timetable": raw_timetable,
                }),
                "network",
            );
            cache_write_envelope(&app, &cache_name, &env);
            Ok(env)
        }
        Err(error) => {
            if let Some(cached) = cache_read_envelope(&app, &cache_name) {
                return Ok(cached);
            }
            Err(error)
        }
    }
}

fn normalize_todo_item(todo: &Value) -> Option<Value> {
    let title = todo
        .get("title")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let id = todo
        .get("id")
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_i64().map(|v| v.to_string()))
        })
        .unwrap_or_else(|| format!("todo-{}", title));
    let course_name = todo
        .get("course_name")
        .or_else(|| todo.get("courseName"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("学在浙大")
        .to_string();
    let end_time = todo
        .get("end_time")
        .or_else(|| todo.get("endTime"))
        .or_else(|| todo.get("expires"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let status = todo
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("pending")
        .to_string();
    let link_url = ["url", "html_url", "link", "link_url", "linkUrl"]
        .iter()
        .find_map(|key| todo.get(*key).and_then(Value::as_str).map(str::trim))
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(str::to_string);

    Some(json!({
        "id": id,
        "title": title,
        "courseName": course_name,
        "course_name": course_name,
        "endTime": end_time,
        "end_time": end_time,
        "status": status,
        "linkUrl": link_url,
        "raw": todo,
    }))
}

fn normalize_todos_payload(raw: Value) -> Value {
    let mut todo_list = raw
        .get("todo_list")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|todo| normalize_todo_item(&todo))
        .collect::<Vec<_>>();

    todo_list.sort_by(|left, right| {
        let left_time = left.get("endTime").and_then(Value::as_str).unwrap_or("");
        let right_time = right.get("endTime").and_then(Value::as_str).unwrap_or("");
        left_time.cmp(right_time)
    });

    json!({
        "todo_list": todo_list,
    })
}

#[tauri::command]
async fn fetch_todos(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    match courses::get_todos(&state).await {
        Ok(data) => {
            let env = envelope(normalize_todos_payload(data), "network");
            cache_write_envelope(&app, TODOS_CACHE_FILE, &env);
            Ok(env)
        }
        Err(error) => {
            if let Some(cached) = cache_read_envelope(&app, TODOS_CACHE_FILE) {
                return Ok(cached);
            }
            Err(error)
        }
    }
}

#[tauri::command]
fn fetch_materials(app: AppHandle) -> Result<Value, String> {
    Ok(envelope(materials::fetch_materials(&app)?, "network"))
}

#[tauri::command]
async fn sync_materials_index(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Value, String> {
    Ok(envelope(
        materials::sync_materials_index(&app, &state).await?,
        "network",
    ))
}

#[tauri::command]
async fn download_material_asset(
    app: AppHandle,
    input: DownloadMaterialInput,
) -> Result<Value, String> {
    Ok(envelope(
        materials::download_material_asset(&app, input).await?,
        "network",
    ))
}

#[tauri::command]
async fn cache_remote_material(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    input: RemoteMaterialDownloadInput,
) -> Result<Value, String> {
    Ok(envelope(
        materials::cache_remote_material(&app, &state, input).await?,
        "network",
    ))
}

#[tauri::command]
fn read_material_text(app: AppHandle, input: MaterialContentInput) -> Result<Value, String> {
    Ok(envelope(
        materials::read_material_text(&app, input)?,
        "network",
    ))
}

#[tauri::command]
fn open_material_asset(app: AppHandle, input: MaterialPathInput) -> Result<Value, String> {
    Ok(envelope(
        materials::open_material_asset(&app, input)?,
        "network",
    ))
}

#[tauri::command]
fn remove_material_cache(app: AppHandle, input: MaterialPathInput) -> Result<Value, String> {
    Ok(envelope(
        materials::remove_material_cache(&app, input)?,
        "network",
    ))
}

#[tauri::command]
async fn run_ai_analysis(input: AiAnalysisInput) -> Result<Value, String> {
    Ok(envelope(
        integrations::run_ai_analysis(input).await?,
        "network",
    ))
}

#[tauri::command]
async fn send_dingtalk_test(input: DingtalkTestInput) -> Result<Value, String> {
    Ok(envelope(
        integrations::send_dingtalk_test(input).await?,
        "network",
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpaPreviewInput {
    grades: Vec<Value>,
    selected_ids: Option<Vec<String>>,
    simulated_scores: Option<HashMap<String, f64>>,
    retake_policy: Option<String>,
    major_course_ids: Option<Vec<String>>,
    course_id_mappings: Option<HashMap<String, String>>,
}

#[tauri::command]
fn calculate_gpa_preview(input: GpaPreviewInput) -> Result<Value, String> {
    let selected = input
        .selected_ids
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let has_selection = !selected.is_empty();
    let simulated = input.simulated_scores.unwrap_or_default();
    let course_mappings = input.course_id_mappings.unwrap_or_default();
    let major_set = input
        .major_course_ids
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();

    let mut grades = Vec::new();
    for raw in &input.grades {
        let mut grade = enrich_grade(raw);
        let xkkh = grade
            .get("xkkh")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if has_selection && !selected.contains(&xkkh) {
            continue;
        }

        let kcdm = grade
            .get("kcdm")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let retake_key = grade
            .get("retakeKey")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if let Some(score) = simulated
            .get(&xkkh)
            .or_else(|| simulated.get(&kcdm))
            .or_else(|| simulated.get(&retake_key))
        {
            apply_simulated_score(&mut grade, *score);
        }

        apply_course_mapping(&mut grade, &course_mappings);
        grades.push(grade);
    }

    let policy = RetakePolicy::from_str(input.retake_policy.as_deref().unwrap_or("first"));
    Ok(json!(compute_gpa_by_policy(&grades, &major_set, policy)))
}

fn apply_course_mapping(grade: &mut Value, mappings: &HashMap<String, String>) {
    if mappings.is_empty() {
        return;
    }

    let xkkh = grade
        .get("xkkh")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let kcdm = grade
        .get("kcdm")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let retake_key = grade
        .get("retakeKey")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mapped = mappings
        .get(retake_key)
        .or_else(|| mappings.get(kcdm))
        .or_else(|| mappings.get(xkkh))
        .cloned();

    if let (Some(mapped_key), Some(object)) = (mapped, grade.as_object_mut()) {
        object.insert("retakeKey".to_string(), json!(mapped_key));
    }
}

fn collect_major_course_ids(major_grades: &[Value]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    for grade in major_grades {
        if let Some(xkkh) = grade.get("xkkh").and_then(Value::as_str) {
            if seen.insert(xkkh.to_string()) {
                ids.push(xkkh.to_string());
            }
        }
        if let Some(kcdm) = grade.get("kcdm").and_then(Value::as_str) {
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
            fetch_materials,
            sync_materials_index,
            download_material_asset,
            cache_remote_material,
            read_material_text,
            open_material_asset,
            remove_material_cache,
            calculate_gpa_preview,
            run_ai_analysis,
            send_dingtalk_test,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod local_smoke {
    use super::*;
    use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
    use std::collections::HashSet;

    fn parse_date_key(value: &str) -> Option<NaiveDate> {
        let trimmed = value.trim();
        let prefix = trimmed.get(..10).unwrap_or(trimmed);
        NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
    }

    fn parse_due_time(value: &str) -> Option<NaiveDateTime> {
        chrono::DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|date_time| date_time.with_timezone(&Local).naive_local())
            .or_else(|| NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%d %H:%M:%S").ok())
            .or_else(|| NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%d %H:%M").ok())
    }

    fn parse_slot_time(value: &str) -> Option<NaiveTime> {
        NaiveTime::parse_from_str(value.trim(), "%H:%M").ok()
    }

    fn monday_of(date: NaiveDate) -> NaiveDate {
        let diff = match date.weekday() {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        };
        date - Duration::days(diff)
    }

    fn fallback_anchor(term: &term::TermDescriptor) -> NaiveDate {
        let parsed_year = term
            .year
            .parse::<i32>()
            .unwrap_or_else(|_| Local::now().date_naive().year());
        let base = if term.academic_semester == "2" {
            NaiveDate::from_ymd_opt(parsed_year + 1, 2, 24)
        } else {
            NaiveDate::from_ymd_opt(parsed_year, 9, 9)
        }
        .unwrap_or_else(|| Local::now().date_naive());
        monday_of(base)
    }

    fn resolve_anchor(term: &term::TermDescriptor, config: &term::TermTimeConfig) -> NaiveDate {
        config
            .start_date
            .as_deref()
            .and_then(parse_date_key)
            .map(monday_of)
            .unwrap_or_else(|| fallback_anchor(term))
    }

    fn resolve_current_term(today: NaiveDate) -> term::TermDescriptor {
        let year = today.year();
        let month = today.month();
        if (2..=8).contains(&month) {
            term::descriptor_from_parts((year - 1).to_string(), "2")
        } else {
            let start_year = if month == 1 { year - 1 } else { year };
            term::descriptor_from_parts(start_year.to_string(), "1")
        }
    }

    fn term_rank(name: &str) -> i64 {
        term::descriptor_from_name(name)
            .and_then(|descriptor| {
                let year = descriptor.year.parse::<i64>().ok()?;
                let semester = descriptor.academic_semester.parse::<i64>().ok()?;
                Some(year * 100 + semester)
            })
            .unwrap_or(-1)
    }

    #[tokio::test]
    #[ignore = "requires local ZJU credentials and network"]
    async fn local_dev_smoke_report() -> Result<(), String> {
        let username = std::env::var("ZJU_USERNAME")
            .map_err(|_| "ZJU_USERNAME env var not set".to_string())?;
        let password = std::env::var("ZJU_PASSWORD")
            .map_err(|_| "ZJU_PASSWORD env var not set".to_string())?;

        let state = AppState::new();
        zjuam::login_zju(&state, &username, &password).await?;
        zdbk::login_zdbk(&state).await?;
        courses::login_courses(&state).await?;

        let (transcript_r, major_r, exams_r, todos_r, learning_courses_r, classroom_r) = tokio::join!(
            zdbk::get_transcript(&state),
            zdbk::get_major_grades(&state),
            zdbk::get_exams(&state),
            courses::get_todos(&state),
            courses::get_learning_courses(&state),
            classroom::ClassroomSession::login(&state),
        );

        let transcript_raw = transcript_r?;
        let major_grades = major_r?;
        let exams = exams_r?;
        let todos = todos_r?;
        let learning_courses = learning_courses_r.unwrap_or_default();

        let processed_grades = transcript_raw.iter().map(enrich_grade).collect::<Vec<_>>();
        let major_course_ids = collect_major_course_ids(&major_grades);
        let major_set = major_course_ids.iter().cloned().collect::<HashSet<_>>();
        let overall_first =
            compute_gpa_by_policy(&processed_grades, &major_set, RetakePolicy::First);
        let overall_highest =
            compute_gpa_by_policy(&processed_grades, &major_set, RetakePolicy::Highest);

        let mut semesters = processed_grades
            .iter()
            .fold(HashMap::<String, Vec<Value>>::new(), |mut acc, grade| {
                let semester_name =
                    extract_semester_name(grade).unwrap_or_else(|| "其他/认定".to_string());
                acc.entry(semester_name).or_default().push(grade.clone());
                acc
            })
            .into_iter()
            .map(|(name, grades)| {
                let descriptor = term::descriptor_from_name(&name);
                let display_name = descriptor
                    .as_ref()
                    .map(|value| value.display_name.clone())
                    .unwrap_or_else(|| name.clone());
                let first = compute_gpa_by_policy(&grades, &major_set, RetakePolicy::First);
                let highest = compute_gpa_by_policy(&grades, &major_set, RetakePolicy::Highest);
                (name, display_name, first, highest)
            })
            .collect::<Vec<_>>();
        semesters.sort_by(|left, right| term_rank(&right.0).cmp(&term_rank(&left.0)));

        let today = Local::now().date_naive();
        let current_term = resolve_current_term(today);
        let raw_timetable =
            zdbk::get_timetable(&state, &current_term.year, &current_term.timetable_semester)
                .await?;
        let sessions = term::normalize_timetable_sessions(&current_term, &raw_timetable);
        let time_config = term::load_remote_term_time_config(&current_term).await;
        let anchor = resolve_anchor(&current_term, &time_config);
        let range_end = today + Duration::days(7);

        let mut flow_items = Vec::<(NaiveDateTime, String)>::new();
        if let Some(todo_list) = todos.get("todo_list").and_then(Value::as_array) {
            for todo in todo_list {
                let Some(end_time) = todo
                    .get("end_time")
                    .and_then(Value::as_str)
                    .and_then(parse_due_time)
                else {
                    continue;
                };
                if end_time.date() < today || end_time.date() >= range_end {
                    continue;
                }
                let title = todo
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("未命名待办");
                let course_name = todo
                    .get("course_name")
                    .and_then(Value::as_str)
                    .unwrap_or("学在浙大");
                flow_items.push((
                    end_time,
                    format!(
                        "[DDL] {} | {}",
                        end_time.format("%Y-%m-%d %H:%M"),
                        format!("{title} @ {course_name}")
                    ),
                ));
            }
        }

        for session in &sessions {
            let start_slot = time_config
                .session_times
                .iter()
                .find(|slot| slot.index == session.start_period)
                .and_then(|slot| parse_slot_time(&slot.start));
            let end_slot = time_config
                .session_times
                .iter()
                .find(|slot| slot.index == session.end_period)
                .and_then(|slot| parse_slot_time(&slot.end));

            for week_number in &session.week_numbers {
                let original_date = anchor
                    + Duration::days(
                        (i64::from(*week_number) - 1) * 7 + i64::from(session.day_of_week) - 1,
                    );
                let original_key = original_date.format("%Y-%m-%d").to_string();
                if time_config.holidays.contains_key(&original_key) {
                    continue;
                }
                let actual_date = time_config
                    .exchanges
                    .get(&original_key)
                    .and_then(|value| parse_date_key(value))
                    .unwrap_or(original_date);
                if actual_date < today || actual_date >= range_end {
                    continue;
                }
                let Some(start_time) = start_slot else {
                    continue;
                };
                let start_at = actual_date.and_time(start_time);
                let time_label = match end_slot {
                    Some(end_time) => format!(
                        "{}-{}",
                        start_time.format("%H:%M"),
                        end_time.format("%H:%M")
                    ),
                    None => start_time.format("%H:%M").to_string(),
                };
                let teacher = if session.teacher.is_empty() {
                    String::new()
                } else {
                    format!(" | {}", session.teacher)
                };
                let location = if session.location.is_empty() {
                    "待定地点".to_string()
                } else {
                    session.location.clone()
                };
                flow_items.push((
                    start_at,
                    format!(
                        "[COURSE] {} {} | {} @ {}{}",
                        actual_date.format("%Y-%m-%d"),
                        time_label,
                        session.course_name,
                        location,
                        teacher,
                    ),
                ));
            }
        }
        flow_items.sort_by(|left, right| left.0.cmp(&right.0));

        println!("=== Celechron Local Smoke ===");
        println!(
            "Current term: {} ({})",
            current_term.display_name, current_term.name
        );
        println!(
            "Overall GPA: first={:.4} highest={:.4} | credits={:.1}",
            overall_first.five_point, overall_highest.five_point, overall_first.total_credits
        );
        println!(
            "Transcript rows: {} | Major rows: {} | Exams: {}",
            processed_grades.len(),
            major_grades.len(),
            exams.len()
        );

        let course_ids = learning_courses
            .iter()
            .filter_map(|course| course.get("id").and_then(Value::as_i64))
            .filter(|id| *id > 0)
            .collect::<Vec<_>>();

        match classroom_r {
            Ok(session) => match session.fetch_material_subjects(&course_ids).await {
                Ok(result) => {
                    println!(
                        "Classroom materials: {} items | week={} | warnings={}",
                        result.items.len(),
                        result.week_label,
                        result.warnings.len()
                    );
                    for warning in result.warnings.iter().take(5) {
                        println!("- [Classroom warning] {warning}");
                    }
                    for item in result.items.iter().take(8) {
                        println!(
                            "- [Classroom] {} | {} | week={} | pages={}",
                            item.course_name,
                            item.sub_name,
                            item.week_bucket,
                            item.ppt_image_urls.len()
                        );
                    }
                }
                Err(error) => println!("Classroom materials failed: {error}"),
            },
            Err(error) => println!("Classroom login failed: {error}"),
        }
        println!(
            "Term anchor: {} | source={} | sessions={} | rawTimetable={}",
            anchor.format("%Y-%m-%d"),
            time_config.source,
            sessions.len(),
            raw_timetable.len()
        );
        println!("Semesters:");
        for (name, display_name, first, highest) in &semesters {
            println!(
                "- {} | {} | first={:.4}/{:.4} credits={:.1} | highest={:.4}/{:.4} credits={:.1}",
                name,
                display_name,
                first.five_point,
                first.four_point,
                first.total_credits,
                highest.five_point,
                highest.four_point,
                highest.total_credits,
            );
        }
        println!("Upcoming 7 days:");
        if flow_items.is_empty() {
            println!("- (empty)");
        } else {
            for (_, line) in &flow_items {
                println!("- {}", line);
            }
        }

        assert!(
            !processed_grades.is_empty(),
            "transcript should not be empty"
        );
        assert!(!semesters.is_empty(), "semester list should not be empty");
        assert!(
            overall_first.total_credits > 0.0,
            "overall credits should be positive"
        );

        Ok(())
    }
}
