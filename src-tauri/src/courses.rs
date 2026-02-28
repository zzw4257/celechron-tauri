use crate::zjuam::AppState;
use serde_json::Value;

/// Login to courses.zju.edu.cn by following CAS SSO redirects.
pub async fn login_courses(state: &AppState) -> Result<(), String> {
    let client = &state.client;
    let iplanet = state.iplanet_cookie.lock().await;
    let cookie_val = iplanet.as_ref().ok_or("未登录：缺少iPlanetDirectoryPro")?;

    let mut url = "https://courses.zju.edu.cn/user/index".to_string();
    let mut cookies: Vec<String> = vec![format!("iPlanetDirectoryPro={}", cookie_val)];
    let mut session_cookie: Option<String> = None;

    // Follow redirects manually, collecting cookies
    for _ in 0..10 {
        let res = client
            .get(&url)
            .header("Cookie", cookies.join("; "))
            .send()
            .await
            .map_err(|e| format!("学在浙大请求失败: {}", e))?;

        // Collect new cookies
        for hv in res.headers().get_all("set-cookie").iter() {
            if let Ok(s) = hv.to_str() {
                let part = s.split(';').next().unwrap_or("").to_string();
                if part.starts_with("session=") {
                    session_cookie = Some(part.clone());
                }
                cookies.push(part);
            }
        }

        // Check if redirect
        if let Some(location) = res.headers().get("location").and_then(|v| v.to_str().ok()) {
            if location == "https://courses.zju.edu.cn/user/index" && session_cookie.is_some() {
                break;
            }
            url = location.to_string();
        } else {
            break;
        }
    }

    let session = session_cookie.ok_or("无法获取session cookie")?;
    *state.courses_session.lock().await = Some(session);
    Ok(())
}

/// Fetch homework/todo list from 学在浙大.
pub async fn get_todos(state: &AppState) -> Result<Value, String> {
    let client = &state.client;
    let session = state.courses_session.lock().await;
    let session_cookie = session.as_ref().ok_or("学在浙大未登录")?;

    let res = client
        .get("https://courses.zju.edu.cn/api/todos")
        .header("Cookie", session_cookie)
        .send()
        .await
        .map_err(|e| format!("作业查询失败: {}", e))?;

    let body: Value = res
        .json()
        .await
        .map_err(|e| format!("作业JSON解析失败: {}", e))?;
    Ok(body)
}
