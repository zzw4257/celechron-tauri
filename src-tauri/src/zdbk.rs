use crate::zjuam::AppState;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Login to zdbk.zju.edu.cn using CAS SSO (iPlanetDirectoryPro cookie).
/// Captures JSESSIONID and route cookies for subsequent requests.
pub async fn login_zdbk(state: &AppState) -> Result<(), String> {
    let client = &state.client;
    let iplanet = state.iplanet_cookie.lock().await;
    let cookie_val = iplanet.as_ref().ok_or("未登录：缺少iPlanetDirectoryPro")?;

    // Step 1: Hit CAS with service URL
    let cas_url = "https://zjuam.zju.edu.cn/cas/login?service=https%3A%2F%2Fzdbk.zju.edu.cn%2Fjwglxt%2Fxtgl%2Flogin_ssologin.html";
    let res = client
        .get(cas_url)
        .header("Cookie", format!("iPlanetDirectoryPro={}", cookie_val))
        .send()
        .await
        .map_err(|e| format!("CAS请求失败: {}", e))?;

    let location = res
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.replace("http://", "https://"))
        .ok_or("CAS未返回重定向")?;

    // Step 2: Follow redirect to zdbk to get JSESSIONID + route
    let res2 = client
        .get(&location)
        .send()
        .await
        .map_err(|e| format!("教务网重定向失败: {}", e))?;

    let mut jsessionid: Option<String> = None;
    let mut route: Option<String> = None;

    for hv in res2.headers().get_all("set-cookie").iter() {
        if let Ok(s) = hv.to_str() {
            println!("DEBUG ZDBK Set-Cookie: {}", s);
            let part = s.split(';').next().unwrap_or("");
            if part.starts_with("JSESSIONID=") && s.contains("/jwglxt") {
                jsessionid = Some(part.to_string());
            } else if part.starts_with("route=") {
                route = Some(part.to_string());
            }
        }
    }

    let jsid = jsessionid.ok_or("无法获取JSESSIONID")?;
    let rt = route.ok_or("无法获取route")?;

    *state.zdbk_jsessionid.lock().await = Some(jsid);
    *state.zdbk_route.lock().await = Some(rt);
    Ok(())
}

fn zdbk_cookies(state_jsid: &str, state_route: &str) -> String {
    format!("{}; {}", state_jsid, state_route)
}

/// Fetch full transcript (all grades).
pub async fn get_transcript(state: &AppState) -> Result<Vec<Value>, String> {
    let client = &state.client;
    let jsid = state.zdbk_jsessionid.lock().await;
    let rt = state.zdbk_route.lock().await;
    let cookies = zdbk_cookies(
        jsid.as_ref().ok_or("教务网未登录")?,
        rt.as_ref().ok_or("教务网未登录")?,
    );

    let url = "https://zdbk.zju.edu.cn/jwglxt/cxdy/xscjcx_cxXscjIndex.html?doType=query&queryModel.showCount=5000";
    let res = client
        .post(url)
        .header("Cookie", &cookies)
        .send()
        .await
        .map_err(|e| format!("成绩查询失败: {}", e))?;

    let text = res
        .text()
        .await
        .map_err(|e| format!("读取成绩响应失败: {}", e))?;

    // Attempt standard JSON parsing first
    if let Ok(json) = serde_json::from_str::<Value>(&text) {
        if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
            return Ok(items.clone());
        }
    }

    // Fallback manual slice if it's wrapped
    let start = text.find(r#""items":["#).map(|i| i + 8);
    let end = text.find(r#"],"limit""#).map(|i| i + 1);

    if let (Some(s), Some(e)) = (start, end) {
        if s < e {
            let json_str = &text[s..e];
            let grades: Vec<Value> =
                serde_json::from_str(json_str).map_err(|e| format!("JSON解析失败: {}", e))?;
            return Ok(grades);
        }
    }

    Err(format!(
        "无法解析成绩数据，响应截断: {}",
        &text[..std::cmp::min(200, text.len())]
    ))
}

/// Fetch major grades and compute major GPA.
pub async fn get_major_grades(state: &AppState) -> Result<Vec<Value>, String> {
    let client = &state.client;
    let jsid = state.zdbk_jsessionid.lock().await;
    let rt = state.zdbk_route.lock().await;
    let cookies = zdbk_cookies(
        jsid.as_ref().ok_or("教务网未登录")?,
        rt.as_ref().ok_or("教务网未登录")?,
    );

    let url = "https://zdbk.zju.edu.cn/jwglxt/zycjtj/xszgkc_cxXsZgkcIndex.html?doType=query&queryModel.showCount=5000";
    let res = client
        .post(url)
        .header("Cookie", &cookies)
        .send()
        .await
        .map_err(|e| format!("主修成绩查询失败: {}", e))?;

    let text = res
        .text()
        .await
        .map_err(|e| format!("读取主修成绩响应失败: {}", e))?;

    if let Ok(json) = serde_json::from_str::<Value>(&text) {
        if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
            return Ok(items.clone());
        }
    }

    let start = text.find(r#""items":["#).map(|i| i + 8);
    let end = text.find(r#"],"limit""#).map(|i| i + 1);

    if let (Some(s), Some(e)) = (start, end) {
        if s < e {
            let json_str = &text[s..e];
            let grades: Vec<Value> =
                serde_json::from_str(json_str).map_err(|e| format!("JSON解析失败: {}", e))?;
            return Ok(grades);
        }
    }

    Err(format!(
        "无法解析主修成绩数据: {}",
        &text[..std::cmp::min(200, text.len())]
    ))
}

/// Fetch timetable for a given academic year and semester.
/// year: e.g. "2024" semester uses xqm value ("3" for 秋冬, "12" for 春夏)
pub async fn get_timetable(
    state: &AppState,
    year: &str,
    semester: &str,
) -> Result<Vec<Value>, String> {
    let client = &state.client;
    let jsid = state.zdbk_jsessionid.lock().await;
    let rt = state.zdbk_route.lock().await;
    let cookies = zdbk_cookies(
        jsid.as_ref().ok_or("教务网未登录")?,
        rt.as_ref().ok_or("教务网未登录")?,
    );

    let url = "https://zdbk.zju.edu.cn/jwglxt/kbcx/xskbcx_cxXsKb.html";
    let form_body = format!("xnm={}&xqm={}", year, semester);

    let res = client
        .post(url)
        .header("Cookie", &cookies)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("X-Requested-With", "XMLHttpRequest")
        .body(form_body)
        .send()
        .await
        .map_err(|e| format!("课表查询失败: {}", e))?;

    let text = res
        .text()
        .await
        .map_err(|e| format!("读取课表响应失败: {}", e))?;

    if text == "null" {
        return Ok(vec![]);
    }

    if let Ok(json) = serde_json::from_str::<Value>(&text) {
        if let Some(sessions) = json.get("kbList").and_then(|v| v.as_array()) {
            return Ok(sessions.clone());
        }
    }

    let start = text.find(r#""kbList":["#).map(|i| i + 9);
    let end = text.find(r#"],"xh""#).map(|i| i + 1);

    if let (Some(s), Some(e)) = (start, end) {
        if s < e {
            let json_str = &text[s..e];
            let sessions: Vec<Value> =
                serde_json::from_str(json_str).map_err(|e| format!("JSON解析失败: {}", e))?;
            return Ok(sessions);
        }
    }

    Err(format!(
        "无法解析课表数据: {}",
        &text[..std::cmp::min(200, text.len())]
    ))
}

/// Fetch exam info.
pub async fn get_exams(state: &AppState) -> Result<Vec<Value>, String> {
    let client = &state.client;
    let jsid = state.zdbk_jsessionid.lock().await;
    let rt = state.zdbk_route.lock().await;
    let cookies = zdbk_cookies(
        jsid.as_ref().ok_or("教务网未登录")?,
        rt.as_ref().ok_or("教务网未登录")?,
    );

    let url = "https://zdbk.zju.edu.cn/jwglxt/xskscx/kscx_cxXsgrksIndex.html?doType=query&queryModel.showCount=5000";
    let res = client
        .post(url)
        .header("Cookie", &cookies)
        .send()
        .await
        .map_err(|e| format!("考试查询失败: {}", e))?;

    let text = res
        .text()
        .await
        .map_err(|e| format!("读取考试响应失败: {}", e))?;

    if let Ok(json) = serde_json::from_str::<Value>(&text) {
        if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
            return Ok(items.clone());
        }
    }

    let start = text.find(r#""items":["#).map(|i| i + 8);
    let end = text.find(r#"],"limit""#).map(|i| i + 1);

    if let (Some(s), Some(e)) = (start, end) {
        if s < e {
            let json_str = &text[s..e];
            let exams: Vec<Value> =
                serde_json::from_str(json_str).map_err(|e| format!("JSON解析失败: {}", e))?;
            return Ok(exams);
        }
    }

    Err(format!(
        "无法解析考试数据: {}",
        &text[..std::cmp::min(200, text.len())]
    ))
}

/// Fetch 二三四课堂 practice scores from HTML page.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PracticeScores {
    pub pt2: f64,
    pub pt3: f64,
    pub pt4: f64,
}

pub async fn get_practice_scores(state: &AppState) -> Result<PracticeScores, String> {
    let client = &state.client;
    let username = state.username.lock().await;
    let student_id = username.as_ref().ok_or("未登录")?;
    let jsid = state.zdbk_jsessionid.lock().await;
    let rt = state.zdbk_route.lock().await;
    let cookies = zdbk_cookies(
        jsid.as_ref().ok_or("教务网未登录")?,
        rt.as_ref().ok_or("教务网未登录")?,
    );

    let url = format!(
        "https://zdbk.zju.edu.cn/jwglxt/dessktgl/dessktcx_cxDessktcxIndex.html?gnmkdm=N108001&layout=default&su={}",
        student_id
    );
    let res = client
        .get(&url)
        .header("Cookie", &cookies)
        .send()
        .await
        .map_err(|e| format!("实践分查询失败: {}", e))?;

    let html = res
        .text()
        .await
        .map_err(|e| format!("读取实践分响应失败: {}", e))?;

    let mut scores = PracticeScores {
        pt2: 0.0,
        pt3: 0.0,
        pt4: 0.0,
    };

    // Try to parse practice scores from HTML
    if let Some(val) = extract_practice_score(&html, "第二课堂") {
        scores.pt2 = val;
    }
    if let Some(val) = extract_practice_score(&html, "第三课堂") {
        scores.pt3 = val;
    }
    if let Some(val) = extract_practice_score(&html, "第四课堂") {
        scores.pt4 = val;
    }

    Ok(scores)
}

fn extract_practice_score(html: &str, class_name: &str) -> Option<f64> {
    let pattern = format!(
        r#"<td[^>]*>{}</td>.*?<td[^>]*>([0-9.]+)</td>"#,
        regex::escape(class_name)
    );
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(html)
        .and_then(|caps| caps[1].parse::<f64>().ok())
}
