use crate::zjuam::AppState;
use reqwest::Response;
use serde_json::Value;

fn todo_endpoint() -> &'static str {
    "https://courses.zju.edu.cn/api/todos"
}

fn my_courses_base() -> &'static str {
    "https://courses.zju.edu.cn/api/my-courses?conditions=%7B%22status%22:%5B%22ongoing%22,%22notStarted%22%5D,%22keyword%22:%22%22,%22classify_type%22:%22recently_started%22,%22display_studio_list%22:false%7D&fields=id,name,course_code,department(id,name),grade(id,name),klass(id,name),course_type,cover,small_cover,start_date,end_date,is_started,is_closed,academic_year_id,semester_id,credit,compulsory,second_name,display_name,created_user(id,name),org(is_enterprise_or_organization),org_id,public_scope,audit_status,audit_remark,can_withdraw_course,imported_from,allow_clone,is_instructor,is_team_teaching,is_default_course_cover,instructors(id,name,email,avatar_small_url),course_attributes(teaching_class_name,is_during_publish_period,copy_status,tip,data),user_stick_course_record(id),classroom_schedule"
}

async fn courses_cookie(state: &AppState) -> Result<String, String> {
    let session = state.courses_session.lock().await;
    session
        .as_ref()
        .cloned()
        .ok_or("学在浙大未登录".to_string())
}

async fn courses_get_json(state: &AppState, url: &str) -> Result<Value, String> {
    let cookie = courses_cookie(state).await?;
    let res = state
        .client
        .get(url)
        .header("Cookie", cookie)
        .send()
        .await
        .map_err(|e| format!("学在浙大请求失败: {}", e))?;

    if !res.status().is_success() {
        return Err(format!(
            "学在浙大接口异常: HTTP {} ({url})",
            res.status().as_u16()
        ));
    }

    res.json::<Value>()
        .await
        .map_err(|e| format!("学在浙大 JSON 解析失败: {}", e))
}

/// Login to courses.zju.edu.cn by following CAS SSO redirects.
pub async fn login_courses(state: &AppState) -> Result<(), String> {
    let client = &state.client;
    let iplanet = state.iplanet_cookie.lock().await;
    let cookie_val = iplanet.as_ref().ok_or("未登录：缺少iPlanetDirectoryPro")?;

    let mut url = "https://courses.zju.edu.cn/user/index".to_string();
    let mut cookies: Vec<String> = vec![format!("iPlanetDirectoryPro={}", cookie_val)];
    let mut session_cookie: Option<String> = None;

    for _ in 0..10 {
        let res = client
            .get(&url)
            .header("Cookie", cookies.join("; "))
            .send()
            .await
            .map_err(|e| format!("学在浙大请求失败: {}", e))?;

        for hv in res.headers().get_all("set-cookie").iter() {
            if let Ok(s) = hv.to_str() {
                let part = s.split(';').next().unwrap_or("").to_string();
                if part.starts_with("session=") {
                    session_cookie = Some(part.clone());
                }
                cookies.push(part);
            }
        }

        if let Some(location) = res.headers().get("location").and_then(|v| v.to_str().ok()) {
            if location == "https://courses.zju.edu.cn/user/index" && session_cookie.is_some() {
                break;
            }
            url = location.to_string();
        } else {
            break;
        }
    }

    let session = session_cookie.ok_or("无法获取 session cookie")?;
    *state.courses_session.lock().await = Some(session);
    Ok(())
}

/// Fetch homework/todo list from 学在浙大.
pub async fn get_todos(state: &AppState) -> Result<Value, String> {
    courses_get_json(state, todo_endpoint()).await
}

/// Fetch current learning courses from 学在浙大.
pub async fn get_learning_courses(state: &AppState) -> Result<Vec<Value>, String> {
    let mut page = 1_i64;
    let mut items = Vec::new();

    loop {
        let url = format!(
            "{}&page={page}&page_size=100&showScorePassedStatus=false",
            my_courses_base()
        );
        let body = courses_get_json(state, &url).await?;
        if let Some(courses) = body.get("courses").and_then(Value::as_array) {
            items.extend(courses.iter().cloned());
        }
        let pages = body.get("pages").and_then(Value::as_i64).unwrap_or(1);
        if page >= pages {
            break;
        }
        page += 1;
    }

    Ok(items)
}

pub async fn get_course_activity_uploads(
    state: &AppState,
    course_id: i64,
) -> Result<Vec<Value>, String> {
    let url = format!("https://courses.zju.edu.cn/api/courses/{course_id}/activities");
    let body = courses_get_json(state, &url).await?;
    let mut uploads = Vec::new();
    if let Some(activities) = body.get("activities").and_then(Value::as_array) {
        for activity in activities {
            if let Some(activity_uploads) = activity.get("uploads").and_then(Value::as_array) {
                uploads.extend(activity_uploads.iter().cloned());
            }
        }
    }
    Ok(uploads)
}

pub async fn get_course_homework_uploads(
    state: &AppState,
    course_id: i64,
) -> Result<Vec<Value>, String> {
    let mut page = 1_i64;
    let mut uploads = Vec::new();

    loop {
        let url = format!(
            "https://courses.zju.edu.cn/api/courses/{course_id}/homework-activities?conditions=%7B%22itemsSortBy%22:%7B%22predicate%22:%22module%22,%22reverse%22:false%7D%7D&page={page}&page_size=20&reloadPage=false"
        );
        let body = courses_get_json(state, &url).await?;
        if let Some(homework_activities) = body.get("homework_activities").and_then(Value::as_array)
        {
            for activity in homework_activities {
                if let Some(activity_uploads) = activity.get("uploads").and_then(Value::as_array) {
                    uploads.extend(activity_uploads.iter().cloned());
                }
            }
        }
        let pages = body.get("pages").and_then(Value::as_i64).unwrap_or(1);
        if page >= pages {
            break;
        }
        page += 1;
    }

    Ok(uploads)
}

pub async fn get_upload_download_response(
    state: &AppState,
    upload_id: i64,
    reference_id: i64,
) -> Result<Response, String> {
    let cookie = courses_cookie(state).await?;
    let primary_url =
        format!("https://courses.zju.edu.cn/api/uploads/reference/{reference_id}/blob");
    let fallback_url = format!("https://courses.zju.edu.cn/api/uploads/{upload_id}/blob");

    let primary = state
        .client
        .get(&primary_url)
        .header("Cookie", cookie.clone())
        .send()
        .await
        .map_err(|e| format!("拉取资料失败: {}", e))?;

    if primary.status().is_success() {
        return Ok(primary);
    }

    let fallback = state
        .client
        .get(&fallback_url)
        .header("Cookie", cookie)
        .send()
        .await
        .map_err(|e| format!("拉取资料失败: {}", e))?;

    if fallback.status().is_success() {
        Ok(fallback)
    } else {
        Err(format!("拉取资料失败: HTTP {}", fallback.status().as_u16()))
    }
}
