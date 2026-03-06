use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TermDescriptor {
    pub year: String,
    pub academic_semester: String,
    pub timetable_semester: String,
    pub name: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTimeSlot {
    pub index: u8,
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TermTimeConfig {
    pub source: String,
    pub start_date: Option<String>,
    pub session_times: Vec<SessionTimeSlot>,
    pub holidays: BTreeMap<String, String>,
    pub exchanges: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedTimetableSession {
    pub id: String,
    pub xkkh: String,
    pub course_id: String,
    pub course_name: String,
    pub teacher: String,
    pub location: String,
    pub day_of_week: u8,
    pub start_period: u8,
    pub end_period: u8,
    pub week_numbers: Vec<u8>,
    pub odd_week: bool,
    pub even_week: bool,
    pub first_half: bool,
    pub second_half: bool,
}

pub fn normalize_academic_semester(semester: &str) -> Option<&'static str> {
    match semester.trim() {
        "1" | "3" => Some("1"),
        "2" | "12" => Some("2"),
        _ => None,
    }
}

pub fn to_timetable_semester(academic_semester: &str) -> &'static str {
    match academic_semester {
        "2" => "12",
        _ => "3",
    }
}

pub fn build_xkkh_prefix(year: &str, academic_semester: &str) -> String {
    let next_year = year
        .parse::<u32>()
        .ok()
        .map(|value| value + 1)
        .map(|value| value.to_string())
        .unwrap_or_else(|| year.to_string());
    format!("({year}-{next_year}-{academic_semester})")
}

pub fn descriptor_from_parts(year: impl Into<String>, academic_semester: &str) -> TermDescriptor {
    let year = year.into();
    let normalized = normalize_academic_semester(academic_semester).unwrap_or("1");
    let next_year = year
        .parse::<u32>()
        .ok()
        .map(|value| value + 1)
        .map(|value| value.to_string())
        .unwrap_or_else(|| year.clone());
    let name = format!("{}-{}-{}", year, next_year, normalized);
    let short_year = year
        .chars()
        .rev()
        .take(2)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    let short_next_year = next_year
        .chars()
        .rev()
        .take(2)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    let semester_name = if normalized == "2" {
        "春夏"
    } else {
        "秋冬"
    };

    TermDescriptor {
        year,
        academic_semester: normalized.to_string(),
        timetable_semester: to_timetable_semester(normalized).to_string(),
        name,
        display_name: format!("{}-{} {}", short_year, short_next_year, semester_name),
    }
}

pub fn descriptor_from_name(name: &str) -> Option<TermDescriptor> {
    let re = Regex::new(r"^(\d{4})-(\d{4})-(\d+)$").ok()?;
    let captures = re.captures(name.trim())?;
    let year = captures.get(1)?.as_str().to_string();
    let semester = captures.get(3)?.as_str();
    Some(descriptor_from_parts(year, semester))
}

pub fn extract_start_date(config: &Value) -> Option<String> {
    fn visit(value: &Value, output: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                if let Some(date) = normalize_date(text) {
                    output.push(date);
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, output);
                }
            }
            Value::Object(map) => {
                for value in map.values() {
                    visit(value, output);
                }
            }
            _ => {}
        }
    }

    let mut candidates = Vec::new();
    visit(config.get("dayOfWeekToDays")?, &mut candidates);
    candidates.sort();
    candidates.into_iter().next()
}

pub async fn load_term_time_config(app: &AppHandle, term: &TermDescriptor) -> TermTimeConfig {
    let cache_name = format!("cache_term_config_{}.json", term.name);

    if let Ok(remote) = fetch_remote_term_config(&term.name).await {
        let _ = write_term_config_cache(app, &cache_name, &remote);
        return parse_term_config(&remote, "remote");
    }

    if let Some(cached) = read_term_config_cache(app, &cache_name) {
        return parse_term_config(&cached, "cache");
    }

    default_term_time_config()
}

#[cfg(test)]
pub async fn load_remote_term_time_config(term: &TermDescriptor) -> TermTimeConfig {
    if let Ok(remote) = fetch_remote_term_config(&term.name).await {
        return parse_term_config(&remote, "remote");
    }

    default_term_time_config()
}

pub fn default_term_time_config() -> TermTimeConfig {
    parse_term_config(&Value::Null, "default")
}

pub fn normalize_timetable_sessions(
    term: &TermDescriptor,
    raw: &[Value],
) -> Vec<NormalizedTimetableSession> {
    let mut sessions = Vec::new();
    let target_prefix = build_xkkh_prefix(&term.year, &term.academic_semester);

    for row in raw {
        let xkkh = row
            .get("xkkh")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();

        if row.get("sfyjskc").and_then(Value::as_str) == Some("1") {
            continue;
        }
        if !xkkh.is_empty() && !xkkh.starts_with(&target_prefix) {
            continue;
        }

        let Some(day_of_week) = row
            .get("xqj")
            .and_then(Value::as_str)
            .or_else(|| row.get("xq").and_then(Value::as_str))
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|value| (1..=7).contains(value))
        else {
            continue;
        };

        let Some((start_period, end_period)) = parse_period_bounds(row) else {
            continue;
        };

        let course_name = read_course_name(row);
        if course_name.is_empty() {
            continue;
        }
        let (teacher, location) = read_teacher_location(row);
        let (first_half, second_half) = parse_half_flags(row);
        let week_numbers = parse_week_numbers(row, first_half, second_half);
        if week_numbers.is_empty() {
            continue;
        }

        let odd_week = row.get("dsz").and_then(Value::as_str) == Some("1");
        let even_week = row.get("dsz").and_then(Value::as_str) == Some("0");
        let course_id = row
            .get("kcdm")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();

        sessions.push(NormalizedTimetableSession {
            id: if xkkh.is_empty() {
                format!(
                    "{}-{}-{}-{}",
                    course_name, day_of_week, start_period, end_period
                )
            } else {
                format!("{}-{}-{}", xkkh, day_of_week, start_period)
            },
            xkkh,
            course_id,
            course_name,
            teacher,
            location,
            day_of_week,
            start_period,
            end_period,
            week_numbers,
            odd_week,
            even_week,
            first_half,
            second_half,
        });
    }

    sessions.sort_by(|left, right| {
        left.day_of_week
            .cmp(&right.day_of_week)
            .then_with(|| left.start_period.cmp(&right.start_period))
            .then_with(|| left.course_name.cmp(&right.course_name))
    });
    sessions
}

fn parse_term_config(raw: &Value, source: &str) -> TermTimeConfig {
    let mut holidays = BTreeMap::new();
    if let Some(map) = raw.get("holidays").and_then(Value::as_object) {
        for (key, value) in map {
            if let (Some(date), Some(label)) = (normalize_date(key), value.as_str()) {
                holidays.insert(date, label.to_string());
            }
        }
    }

    let mut exchanges = BTreeMap::new();
    if let Some(map) = raw.get("exchanges").and_then(Value::as_object) {
        for (key, value) in map {
            if let (Some(from), Some(to)) =
                (normalize_date(key), value.as_str().and_then(normalize_date))
            {
                exchanges.insert(from, to);
            }
        }
    }

    let mut session_times = default_session_times();
    if let Some(remote_slots) = raw.get("sessionToTime").and_then(Value::as_array) {
        let parsed = parse_session_times(remote_slots);
        if !parsed.is_empty() {
            session_times = parsed;
        }
    }

    TermTimeConfig {
        source: source.to_string(),
        start_date: extract_start_date(raw),
        session_times,
        holidays,
        exchanges,
    }
}

fn parse_period_bounds(row: &Value) -> Option<(u8, u8)> {
    if let Some(jcs) = row
        .get("jcs")
        .and_then(Value::as_str)
        .or_else(|| row.get("jc").and_then(Value::as_str))
    {
        let numbers = jcs
            .split('-')
            .filter_map(|value| value.trim().parse::<u8>().ok())
            .collect::<Vec<_>>();
        if let Some(first) = numbers.first().copied() {
            let last = numbers.last().copied().unwrap_or(first);
            return Some((first, last));
        }
    }

    let start_period = row
        .get("djj")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u8>().ok())?;
    let span = row
        .get("skcd")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(2);
    let end_period = start_period.saturating_add(span.saturating_sub(1));
    Some((start_period, end_period))
}

fn read_course_name(row: &Value) -> String {
    if let Some(name) = row.get("kcmc").and_then(Value::as_str) {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    row.get("kcb")
        .and_then(Value::as_str)
        .and_then(|html| html.split("<br>").next())
        .map(strip_html)
        .unwrap_or_default()
}

fn read_teacher_location(row: &Value) -> (String, String) {
    let mut teacher = row
        .get("jsxm")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut location = row
        .get("cdmc")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();

    if let Some(kcb) = row.get("kcb").and_then(Value::as_str) {
        let parts = kcb.split("<br>").collect::<Vec<_>>();
        if teacher.is_empty() && parts.len() > 2 {
            teacher = strip_html(parts[2]);
        }
        if location.is_empty() && parts.len() > 3 {
            location = strip_html(parts[3])
                .split("zwf")
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
        }
        if location.is_empty() && parts.len() > 2 {
            location = strip_html(parts[2]);
        }
    }

    (teacher, location)
}

fn parse_half_flags(row: &Value) -> (bool, bool) {
    let xxq = row
        .get("xxq")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let first_half = xxq.contains('秋') || xxq.contains('春');
    let second_half = xxq.contains('冬') || xxq.contains('夏');

    match (first_half, second_half) {
        (false, false) => (true, true),
        flags => flags,
    }
}

fn parse_week_numbers(row: &Value, first_half: bool, second_half: bool) -> Vec<u8> {
    let week_text = row
        .get("zcs")
        .and_then(Value::as_str)
        .or_else(|| row.get("zc").and_then(Value::as_str))
        .map(|value| value.to_string())
        .or_else(|| {
            row.get("kcb")
                .and_then(Value::as_str)
                .and_then(|html| html.split("<br>").nth(1))
                .map(|value| value.to_string())
        })
        .unwrap_or_default();

    let base_weeks = parse_relative_weeks(&week_text);
    let mut absolute = Vec::new();
    if first_half {
        absolute.extend(base_weeks.iter().copied());
    }
    if second_half {
        absolute.extend(base_weeks.iter().map(|week| week.saturating_add(8)));
    }
    if !first_half && !second_half {
        absolute.extend(base_weeks);
    }

    let dsz = row
        .get("dsz")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if dsz == "1" {
        absolute.retain(|week| week % 2 == 1);
    } else if dsz == "0" {
        absolute.retain(|week| week % 2 == 0);
    }

    absolute.sort_unstable();
    absolute.dedup();
    absolute
}

fn parse_relative_weeks(text: &str) -> Vec<u8> {
    let source = text.trim();
    let mut result = Vec::new();

    let raw = if let Some(captures) = Regex::new(r"第([0-9,\-]+)周")
        .ok()
        .and_then(|re| re.captures(source))
    {
        captures
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or(source)
    } else {
        source
    };

    for segment in raw.split(',') {
        let cleaned = segment
            .trim()
            .replace(|char: char| !char.is_ascii_digit() && char != '-', "");
        if cleaned.is_empty() {
            continue;
        }
        let range = cleaned.split('-').collect::<Vec<_>>();
        match range.as_slice() {
            [single] => {
                if let Ok(week) = single.parse::<u8>() {
                    result.push(week);
                }
            }
            [start, end] => {
                if let (Ok(start), Ok(end)) = (start.parse::<u8>(), end.parse::<u8>()) {
                    if start <= end {
                        for week in start..=end {
                            result.push(week);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if result.is_empty() {
        result.extend(1..=8);
    }

    result.sort_unstable();
    result.dedup();
    result
}

fn parse_session_times(slots: &[Value]) -> Vec<SessionTimeSlot> {
    let mut parsed = Vec::new();
    for (index, slot) in slots.iter().enumerate() {
        let Some(pair) = slot.as_array() else {
            continue;
        };
        if pair.len() < 2 {
            continue;
        }
        let Some(start) = pair[0].as_i64() else {
            continue;
        };
        let Some(end) = pair[1].as_i64() else {
            continue;
        };
        if index == 0 {
            continue;
        }
        parsed.push(SessionTimeSlot {
            index: index as u8,
            start: minutes_to_hhmm(start),
            end: minutes_to_hhmm(end),
        });
    }
    parsed
}

fn default_session_times() -> Vec<SessionTimeSlot> {
    let defaults = [
        (1, 480, 525),
        (2, 530, 575),
        (3, 600, 645),
        (4, 650, 695),
        (5, 700, 745),
        (6, 805, 850),
        (7, 855, 900),
        (8, 905, 950),
        (9, 975, 1020),
        (10, 1025, 1070),
        (11, 1130, 1175),
        (12, 1180, 1225),
        (13, 1230, 1275),
        (14, 1280, 1325),
    ];

    defaults
        .iter()
        .map(|(index, start, end)| SessionTimeSlot {
            index: *index,
            start: minutes_to_hhmm(*start),
            end: minutes_to_hhmm(*end),
        })
        .collect()
}

fn minutes_to_hhmm(total_minutes: i64) -> String {
    let hours = total_minutes.div_euclid(60);
    let minutes = total_minutes.rem_euclid(60);
    format!("{hours:02}:{minutes:02}")
}

fn strip_html(value: &str) -> String {
    Regex::new(r"<[^>]+>")
        .map(|re| re.replace_all(value, "").trim().to_string())
        .unwrap_or_else(|_| value.trim().to_string())
}

fn normalize_date(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() < 10 {
        return None;
    }
    let candidate = &trimmed[..10];
    let bytes = candidate.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if candidate
        .chars()
        .enumerate()
        .all(|(index, ch)| matches!(index, 4 | 7) || ch.is_ascii_digit())
    {
        Some(candidate.to_string())
    } else {
        None
    }
}

async fn fetch_remote_term_config(term_name: &str) -> Result<Value, String> {
    let urls = [
        format!("https://calendar.celechron.top/{term_name}.json"),
        format!("http://calendar.celechron.top/{term_name}.json"),
    ];

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|error| error.to_string())?;

    let mut last_error = String::from("term config unavailable");
    for url in urls {
        match client.get(&url).send().await {
            Ok(response) => match response.error_for_status() {
                Ok(ok) => {
                    let json = ok
                        .json::<Value>()
                        .await
                        .map_err(|error| error.to_string())?;
                    return Ok(json);
                }
                Err(error) => {
                    last_error = error.to_string();
                }
            },
            Err(error) => {
                last_error = error.to_string();
            }
        }
    }

    Err(last_error)
}

fn cache_path(app: &AppHandle, filename: &str) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|dir| {
            let _ = fs::create_dir_all(&dir);
            dir.join(filename)
        })
        .ok()
}

fn write_term_config_cache(app: &AppHandle, filename: &str, value: &Value) -> Result<(), String> {
    let Some(path) = cache_path(app, filename) else {
        return Err("cache path unavailable".to_string());
    };
    let content = serde_json::to_string(value).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn read_term_config_cache(app: &AppHandle, filename: &str) -> Option<Value> {
    let path = cache_path(app, filename)?;
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        build_xkkh_prefix, descriptor_from_name, descriptor_from_parts,
        normalize_academic_semester, normalize_timetable_sessions, parse_relative_weeks,
    };
    use serde_json::json;

    #[test]
    fn semester_aliases_are_normalized() {
        assert_eq!(normalize_academic_semester("1"), Some("1"));
        assert_eq!(normalize_academic_semester("3"), Some("1"));
        assert_eq!(normalize_academic_semester("2"), Some("2"));
        assert_eq!(normalize_academic_semester("12"), Some("2"));
        assert_eq!(normalize_academic_semester("short"), None);
    }

    #[test]
    fn display_name_is_stable() {
        let descriptor = descriptor_from_parts("2024", "2");
        assert_eq!(descriptor.name, "2024-2025-2");
        assert_eq!(descriptor.display_name, "24-25 春夏");
        assert_eq!(
            descriptor_from_name("2024-2025-12").unwrap().display_name,
            "24-25 春夏"
        );
    }

    #[test]
    fn xkkh_prefix_uses_normalized_term() {
        assert_eq!(build_xkkh_prefix("2024", "1"), "(2024-2025-1)");
    }

    #[test]
    fn week_parser_expands_ranges() {
        assert_eq!(parse_relative_weeks("第1-3,5周"), vec![1, 2, 3, 5]);
        assert_eq!(parse_relative_weeks(""), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn timetable_normalizer_filters_cross_term_rows() {
        let descriptor = descriptor_from_parts("2024", "1");
        let rows = vec![
            json!({
                "xkkh": "(2024-2025-1)-211G0001-01",
                "kcdm": "211G0001",
                "kcmc": "程序设计",
                "xqj": "1",
                "jcs": "1-2",
                "zcs": "1-8",
                "xxq": "秋",
            }),
            json!({
                "xkkh": "(2024-2025-2)-211G0001-01",
                "kcdm": "211G0001",
                "kcmc": "程序设计",
                "xqj": "1",
                "jcs": "1-2",
                "zcs": "1-8",
                "xxq": "春",
            }),
        ];

        let sessions = normalize_timetable_sessions(&descriptor, &rows);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].course_name, "程序设计");
    }
}
