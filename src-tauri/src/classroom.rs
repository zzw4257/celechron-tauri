use crate::zjuam::AppState;
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use percent_encoding::percent_decode_str;
use regex::Regex;
use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const CLASSROOM_SSO_URL: &str = "https://tgmedia.cmc.zju.edu.cn/index.php?r=auth/login&auType=cmc&tenant_code=112&forward=https%3A%2F%2Fclassroom.zju.edu.cn%2F";
const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const MAX_SSO_REDIRECTS: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassroomSubject {
    pub course_id: i64,
    pub sub_id: i64,
    pub course_name: String,
    pub sub_name: String,
    pub lecturer_name: String,
    pub ppt_image_urls: Vec<String>,
    pub week_bucket: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassroomFetchResult {
    pub items: Vec<ClassroomSubject>,
    pub warnings: Vec<String>,
    pub week_label: String,
}

#[derive(Clone)]
pub struct ClassroomSession {
    client: Client,
    token: String,
    account: String,
}

fn parse_i64(value: Option<&Value>) -> i64 {
    value
        .and_then(|item| {
            item.as_i64().or_else(|| {
                item.as_str()
                    .and_then(|text| text.trim().parse::<i64>().ok())
            })
        })
        .unwrap_or_default()
}

fn clean_label(input: &str) -> String {
    input.trim().replace('/', "_")
}

fn auth_headers(token: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    let bearer = format!("Bearer {token}");
    let auth = HeaderValue::from_str(&bearer)
        .map_err(|error| format!("Classroom 鉴权头构建失败: {error}"))?;
    headers.insert(AUTHORIZATION, auth);
    Ok(headers)
}

fn extract_token(jar: &Jar) -> Result<String, String> {
    let classroom_url =
        Url::parse("https://classroom.zju.edu.cn").map_err(|error| error.to_string())?;
    let cookies = jar
        .cookies(&classroom_url)
        .ok_or_else(|| "Classroom cookie 缺失，请重新登录".to_string())?;
    let cookie_text = percent_decode_str(cookies.to_str().map_err(|error| error.to_string())?)
        .decode_utf8_lossy()
        .to_string();
    let re = Regex::new(r#"\{i:\d+;s:\d+:"_token";i:\d+;s:\d+:"(.+?)";\}"#)
        .map_err(|error| format!("Classroom token 正则构建失败: {error}"))?;
    re.captures(&cookie_text)
        .and_then(|caps| caps.get(1).map(|item| item.as_str().to_string()))
        .ok_or_else(|| "Classroom token 缺失，请重新登录".to_string())
}

fn resolve_redirect_url(current: &Url, target: &str) -> Option<Url> {
    let trimmed = target.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return None;
    }
    current
        .join(trimmed)
        .ok()
        .or_else(|| Url::parse(trimmed).ok())
}

fn extract_html_redirect(current: &Url, body: &str) -> Option<Url> {
    let patterns = [
        r#"url=([^"'\s>]+)"#,
        r#"location\.href\s*=\s*["']([^"']+)["']"#,
        r#"window\.location\s*=\s*["']([^"']+)["']"#,
        r#"window\.location\.replace\(["']([^"']+)["']\)"#,
    ];

    for pattern in patterns {
        let re = Regex::new(pattern).ok()?;
        if let Some(target) = re
            .captures(body)
            .and_then(|caps| caps.get(1).map(|item| item.as_str()))
        {
            if let Some(url) = resolve_redirect_url(current, target) {
                return Some(url);
            }
        }
    }

    None
}

fn extract_refresh_redirect(current: &Url, refresh: &str) -> Option<Url> {
    let lowered = refresh.trim();
    let target = lowered
        .split(';')
        .find_map(|part| part.trim().strip_prefix("url="))
        .or_else(|| {
            lowered
                .split(';')
                .find_map(|part| part.trim().strip_prefix("URL="))
        });
    target.and_then(|value| resolve_redirect_url(current, value))
}

impl ClassroomSession {
    pub async fn login(state: &AppState) -> Result<Self, String> {
        let iplanet = state
            .iplanet_cookie
            .lock()
            .await
            .clone()
            .ok_or_else(|| "统一认证未登录，无法访问智云课堂".to_string())?;

        let jar = Arc::new(Jar::default());
        let cas_url = Url::parse("https://zjuam.zju.edu.cn").map_err(|error| error.to_string())?;
        let cookie_name = ["iPlanet", "Directory", "Pro"].join("");
        let cookie_value = format!("{cookie_name}={iplanet}");
        jar.add_cookie_str(&cookie_value, &cas_url);

        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_provider(jar.clone())
            .user_agent(USER_AGENT_VALUE)
            .build()
            .map_err(|error| format!("构建 Classroom 客户端失败: {error}"))?;

        let mut current_url = Url::parse(CLASSROOM_SSO_URL)
            .map_err(|error| format!("Classroom SSO 地址非法: {error}"))?;
        let mut final_url = current_url.clone();
        let mut reached_classroom = false;
        let mut final_body = String::new();
        let mut final_status = String::new();
        let mut final_refresh = String::new();
        let mut final_location = String::new();

        for _ in 0..MAX_SSO_REDIRECTS {
            let response = client
                .get(current_url.clone())
                .send()
                .await
                .map_err(|error| format!("Classroom SSO 登录失败: {error}"))?;

            for cookie in response.headers().get_all("set-cookie").iter() {
                if let Ok(raw) = cookie.to_str() {
                    jar.add_cookie_str(raw, &current_url);
                }
            }

            final_status = response.status().to_string();

            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            final_location = location.clone().unwrap_or_default();
            let refresh = response
                .headers()
                .get("refresh")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            final_refresh = refresh.clone().unwrap_or_default();
            let body = response.text().await.unwrap_or_default();
            final_body = body.clone();

            if let Some(target) = location
                .as_deref()
                .and_then(|value| resolve_redirect_url(&current_url, value))
                .or_else(|| {
                    refresh
                        .as_deref()
                        .and_then(|value| extract_refresh_redirect(&current_url, value))
                })
                .or_else(|| extract_html_redirect(&current_url, &body))
            {
                current_url = target.clone();
                final_url = target;
                if final_url.domain() == Some("classroom.zju.edu.cn") {
                    reached_classroom = true;
                }
                continue;
            }

            final_url = current_url.clone();
            reached_classroom = final_url.domain() == Some("classroom.zju.edu.cn")
                || body.contains("classroom.zju.edu.cn")
                || body.contains("_token");
            break;
        }

        if !reached_classroom {
            let snippet = final_body.chars().take(240).collect::<String>();
            return Err(format!(
                "Classroom SSO 未到达目标站点，最终停留在 {} | status={} | location={} | refresh={} | body={} ",
                final_url,
                final_status,
                final_location,
                final_refresh,
                snippet.replace('\n', " ")
            ));
        }

        client
            .get("https://classroom.zju.edu.cn/")
            .send()
            .await
            .map_err(|error| format!("Classroom 首页预热失败: {error}"))?;

        let token = extract_token(jar.as_ref())?;
        let headers = auth_headers(&token)?;
        let info: Value = client
            .get("https://classroom.zju.edu.cn/userapi/v1/infosimple")
            .headers(headers)
            .send()
            .await
            .map_err(|error| format!("获取 Classroom 用户信息失败: {error}"))?
            .json()
            .await
            .map_err(|error| format!("解析 Classroom 用户信息失败: {error}"))?;

        let account = info
            .get("params")
            .and_then(|params| params.get("account"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Classroom 用户账号缺失".to_string())?
            .to_string();

        Ok(Self {
            client,
            token,
            account,
        })
    }

    pub fn current_week_bounds() -> (NaiveDate, NaiveDate, String) {
        let today = Local::now().date_naive();
        let monday_offset = match today.weekday() {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        };
        let monday = today - Duration::days(monday_offset);
        let sunday = monday + Duration::days(6);
        let label = format!(
            "{:02}/{:02}-{:02}/{:02}",
            monday.month(),
            monday.day(),
            sunday.month(),
            sunday.day()
        );
        (monday, sunday, label)
    }

    pub async fn fetch_material_subjects(
        &self,
        course_ids: &[i64],
    ) -> Result<ClassroomFetchResult, String> {
        let (week_start, week_end, week_label) = Self::current_week_bounds();
        let mut warnings = Vec::new();
        let mut current_items = Vec::<ClassroomSubject>::new();
        let mut current_ids = HashSet::<(i64, i64)>::new();

        match self.fetch_range_subjects(week_start, week_end).await {
            Ok(subjects) => {
                for subject in subjects {
                    current_ids.insert((subject.course_id, subject.sub_id));
                    current_items.push(subject);
                }
            }
            Err(error) => warnings.push(format!("智云课堂本周资料同步失败: {error}")),
        }

        let mut all_subjects = Vec::<ClassroomSubject>::new();
        for course_id in course_ids.iter().copied().filter(|value| *value > 0) {
            match self.fetch_course_subjects(course_id).await {
                Ok(subjects) => all_subjects.extend(subjects),
                Err(error) => {
                    warnings.push(format!("智云课堂课程 {course_id} 资料同步失败: {error}"))
                }
            }
        }

        let mut merged = HashMap::<(i64, i64), ClassroomSubject>::new();
        for subject in all_subjects.into_iter().chain(current_items.into_iter()) {
            let key = (subject.course_id, subject.sub_id);
            let entry = merged.entry(key).or_insert_with(|| ClassroomSubject {
                week_bucket: if current_ids.contains(&key) {
                    "current".to_string()
                } else {
                    "unknown".to_string()
                },
                ..subject.clone()
            });
            if entry.ppt_image_urls.is_empty() && !subject.ppt_image_urls.is_empty() {
                entry.ppt_image_urls = subject.ppt_image_urls.clone();
            }
            if current_ids.contains(&key) {
                entry.week_bucket = "current".to_string();
            }
        }

        let mut items = Vec::new();
        for mut subject in merged.into_values() {
            match self.get_ppt_urls(subject.course_id, subject.sub_id).await {
                Ok(urls) => {
                    if urls.is_empty() {
                        continue;
                    }
                    subject.ppt_image_urls = urls;
                    items.push(subject);
                }
                Err(error) => warnings.push(format!(
                    "{} / {} PPT 解析失败: {}",
                    subject.course_name, subject.sub_name, error
                )),
            }
        }

        items.sort_by(|left, right| {
            left.week_bucket
                .cmp(&right.week_bucket)
                .then_with(|| left.course_name.cmp(&right.course_name))
                .then_with(|| left.sub_name.cmp(&right.sub_name))
        });

        Ok(ClassroomFetchResult {
            items,
            warnings,
            week_label,
        })
    }

    async fn fetch_range_subjects(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<ClassroomSubject>, String> {
        let headers = auth_headers(&self.token)?;
        let mut subjects = Vec::new();
        let mut date = start;
        while date <= end {
            let payload: Value = self
                .client
                .get(format!(
                    "https://classroom.zju.edu.cn/courseapi/v2/course-live/get-my-course-day?day={}",
                    date.format("%Y-%m-%d")
                ))
                .headers(headers.clone())
                .send()
                .await
                .map_err(|error| format!("拉取智云课堂日程失败: {error}"))?
                .json()
                .await
                .map_err(|error| format!("解析智云课堂日程失败: {error}"))?;
            if let Some(list) = payload.get("list").and_then(Value::as_object) {
                for data in list.values() {
                    if let Some(courses) = data.get("course").and_then(Value::as_array) {
                        for course in courses {
                            if let Some(subject) = self.subject_from_day(course) {
                                subjects.push(subject);
                            }
                        }
                    }
                }
            }
            date += Duration::days(1);
        }
        Ok(subjects)
    }

    async fn fetch_course_subjects(&self, course_id: i64) -> Result<Vec<ClassroomSubject>, String> {
        let headers = auth_headers(&self.token)?;
        let payload: Value = self
            .client
            .get(format!(
                "https://yjapi.cmc.zju.edu.cn/courseapi/v3/multi-search/get-course-detail?course_id={course_id}&student={}",
                urlencoding::encode(&self.account)
            ))
            .headers(headers)
            .send()
            .await
            .map_err(|error| format!("拉取智云课堂课程详情失败: {error}"))?
            .json()
            .await
            .map_err(|error| format!("解析智云课堂课程详情失败: {error}"))?;

        let data = payload
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| "智云课堂课程详情为空".to_string())?;
        let course_name = data
            .get("title")
            .and_then(Value::as_str)
            .map(clean_label)
            .unwrap_or_else(|| format!("课程 {course_id}"));
        let mut items = Vec::new();
        let Some(year_map) = data.get("sub_list").and_then(Value::as_object) else {
            return Ok(items);
        };

        for month_map in year_map.values().filter_map(Value::as_object) {
            for week_map in month_map.values().filter_map(Value::as_object) {
                for subject_list in week_map.values().filter_map(Value::as_array) {
                    for subject in subject_list {
                        let sub_id = parse_i64(subject.get("id"));
                        if sub_id <= 0 {
                            continue;
                        }
                        let sub_name = subject
                            .get("sub_title")
                            .and_then(Value::as_str)
                            .map(clean_label)
                            .unwrap_or_else(|| format!("课件 {sub_id}"));
                        let lecturer_name = subject
                            .get("lecturer_name")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .unwrap_or("")
                            .to_string();
                        items.push(ClassroomSubject {
                            course_id,
                            sub_id,
                            course_name: course_name.clone(),
                            sub_name,
                            lecturer_name,
                            ppt_image_urls: Vec::new(),
                            week_bucket: "unknown".to_string(),
                        });
                    }
                }
            }
        }

        Ok(items)
    }

    pub async fn get_ppt_urls(&self, course_id: i64, sub_id: i64) -> Result<Vec<String>, String> {
        let mut urls = Vec::new();
        let mut page = 1_i64;
        let mut total = None::<i64>;

        loop {
            let payload: Value = self
                .client
                .get(format!(
                    "https://classroom.zju.edu.cn/pptnote/v1/schedule/search-ppt?course_id={course_id}&sub_id={sub_id}&page={page}&per_page=100"
                ))
                .send()
                .await
                .map_err(|error| format!("拉取 Classroom PPT 列表失败: {error}"))?
                .json()
                .await
                .map_err(|error| format!("解析 Classroom PPT 列表失败: {error}"))?;

            let list = payload
                .get("list")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if total.is_none() {
                total = payload.get("total").and_then(Value::as_i64);
            }

            for ppt in &list {
                let content = ppt
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Classroom PPT 内容缺失".to_string())?;
                let parsed: Value = serde_json::from_str(content)
                    .map_err(|error| format!("解析 Classroom PPT 内容失败: {error}"))?;
                if let Some(url) = parsed.get("pptimgurl").and_then(Value::as_str) {
                    let trimmed = url.trim();
                    if !trimmed.is_empty() {
                        urls.push(trimmed.to_string());
                    }
                }
            }

            let expected = total.unwrap_or_default().max(0) as usize;
            if expected == 0 || urls.len() >= expected || list.len() < 100 {
                break;
            }
            page += 1;
        }

        Ok(urls)
    }

    pub async fn download_bytes(&self, url: &str) -> Result<(Vec<u8>, Option<String>), String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("下载 Classroom 资料失败: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "下载 Classroom 资料失败: HTTP {}",
                response.status().as_u16()
            ));
        }
        let mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("读取 Classroom 资料失败: {error}"))?;
        Ok((bytes.to_vec(), mime))
    }

    fn subject_from_day(&self, course: &Value) -> Option<ClassroomSubject> {
        let course_id = parse_i64(course.get("id"));
        let sub_id = parse_i64(course.get("sub_id"));
        if course_id <= 0 || sub_id <= 0 {
            return None;
        }
        Some(ClassroomSubject {
            course_id,
            sub_id,
            course_name: course
                .get("title")
                .and_then(Value::as_str)
                .map(clean_label)
                .unwrap_or_else(|| format!("课程 {course_id}")),
            sub_name: course
                .get("sub_title")
                .and_then(Value::as_str)
                .map(clean_label)
                .unwrap_or_else(|| format!("课件 {sub_id}")),
            lecturer_name: course
                .get("realname")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .to_string(),
            ppt_image_urls: Vec::new(),
            week_bucket: "current".to_string(),
        })
    }
}
