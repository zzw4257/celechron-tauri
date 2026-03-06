use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetakePolicy {
    First,
    Highest,
}

impl RetakePolicy {
    pub fn from_str(s: &str) -> Self {
        match s {
            "highest" | "best" => RetakePolicy::Highest,
            _ => RetakePolicy::First,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpaSummary {
    pub five_point: f64,
    pub four_point: f64,
    pub four_point_legacy: f64,
    pub hundred_point: f64,
    pub total_credits: f64,
    pub major_gpa: f64,
    pub major_gpa_legacy: f64,
    pub major_credits: f64,
}

impl Default for GpaSummary {
    fn default() -> Self {
        Self {
            five_point: 0.0,
            four_point: 0.0,
            four_point_legacy: 0.0,
            hundred_point: 0.0,
            total_credits: 0.0,
            major_gpa: 0.0,
            major_gpa_legacy: 0.0,
            major_credits: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
struct GradeEntry {
    xkkh: String,
    kcdm: String,
    key: String,
    cj: String,
    credit: f64,
    five_point: f64,
    four_point: f64,
    four_point_legacy: f64,
    hundred_point: f64,
    sem_rank: i64,
}

pub fn extract_semester_name(grade: &Value) -> Option<String> {
    if let Some(id) = grade.get("xkkh").and_then(Value::as_str) {
        let re = Regex::new(r"\((\d{4})-(\d{4})-(\d+)\)").ok()?;
        if let Some(caps) = re.captures(id) {
            let year = caps.get(1)?.as_str();
            let semester = normalize_semester_code(caps.get(3)?.as_str())?;
            return Some(format!(
                "{}-{}-{}",
                year,
                year.parse::<u32>().ok()?.saturating_add(1),
                semester
            ));
        }
    }

    let xnm = grade
        .get("xnm")
        .and_then(|value| {
            value
                .as_str()
                .map(|text| text.to_string())
                .or_else(|| value.as_u64().map(|n| n.to_string()))
        })
        .unwrap_or_default();
    let xqm = grade
        .get("xqm")
        .and_then(|value| {
            value
                .as_str()
                .map(|text| text.to_string())
                .or_else(|| value.as_u64().map(|n| n.to_string()))
        })
        .unwrap_or_default();

    if !xnm.is_empty() {
        let semester = normalize_semester_code(&xqm).unwrap_or("1");
        let next_year = xnm
            .parse::<u32>()
            .ok()
            .map(|value| value + 1)
            .unwrap_or_default();
        return Some(format!("{}-{}-{}", xnm, next_year, semester));
    }

    None
}

pub fn enrich_grade(raw: &Value) -> Value {
    let mut enriched = raw.clone();
    let cj = raw
        .get("cj")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let credit = parse_f64(raw.get("credit"))
        .or_else(|| parse_f64(raw.get("xf")))
        .unwrap_or(0.0);
    let hundred_point = parse_f64(raw.get("hundredPoint")).unwrap_or_else(|| parse_score(&cj));
    let five_point = parse_f64(raw.get("fivePoint"))
        .or_else(|| parse_f64(raw.get("jd")))
        .unwrap_or_else(|| to_five_point(hundred_point));
    let four_point =
        parse_f64(raw.get("fourPoint")).unwrap_or_else(|| to_four_point_43(five_point));
    let four_point_legacy =
        parse_f64(raw.get("fourPointLegacy")).unwrap_or_else(|| to_four_point_legacy(five_point));
    let xkkh = raw.get("xkkh").and_then(Value::as_str).unwrap_or_default();
    let (credit_included, gpa_included, earned_credit) =
        grade_flags(xkkh, &cj, hundred_point, five_point);
    let retake_key = canonical_course_key(
        xkkh,
        raw.get("kcdm").and_then(Value::as_str).unwrap_or_default(),
        raw.get("kcmc").and_then(Value::as_str).unwrap_or_default(),
    );
    let semester_name = extract_semester_name(raw).unwrap_or_else(|| "其他/认定".to_string());

    if let Some(obj) = enriched.as_object_mut() {
        obj.insert("credit".to_string(), json!(credit));
        obj.insert("hundredPoint".to_string(), json!(hundred_point));
        obj.insert("fivePoint".to_string(), json!(five_point));
        obj.insert("fourPoint".to_string(), json!(four_point));
        obj.insert("fourPointLegacy".to_string(), json!(four_point_legacy));
        obj.insert("creditIncluded".to_string(), json!(credit_included));
        obj.insert("gpaIncluded".to_string(), json!(gpa_included));
        obj.insert(
            "earnedCredit".to_string(),
            json!(if earned_credit { credit } else { 0.0 }),
        );
        obj.insert("retakeKey".to_string(), json!(retake_key));
        obj.insert("semesterName".to_string(), json!(semester_name));
    }

    enriched
}

pub fn apply_simulated_score(grade: &mut Value, score: f64) {
    let hundred = score.clamp(0.0, 100.0);
    let five = to_five_point(hundred);
    let four = to_four_point_43(five);
    let legacy = to_four_point_legacy(five);
    let cj = format!("{:.2}", hundred);
    let xkkh = grade
        .get("xkkh")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let (credit_included, gpa_included, earned_credit) = grade_flags(&xkkh, &cj, hundred, five);
    let credit = parse_f64(grade.get("credit"))
        .or_else(|| parse_f64(grade.get("xf")))
        .unwrap_or(0.0);

    if let Some(obj) = grade.as_object_mut() {
        obj.insert("cj".to_string(), json!(cj));
        obj.insert("hundredPoint".to_string(), json!(hundred));
        obj.insert("fivePoint".to_string(), json!(five));
        obj.insert("fourPoint".to_string(), json!(four));
        obj.insert("fourPointLegacy".to_string(), json!(legacy));
        obj.insert("creditIncluded".to_string(), json!(credit_included));
        obj.insert("gpaIncluded".to_string(), json!(gpa_included));
        obj.insert(
            "earnedCredit".to_string(),
            json!(if earned_credit { credit } else { 0.0 }),
        );
    }
}

pub fn compute_gpa_by_policy(
    grades: &[Value],
    major_course_ids: &HashSet<String>,
    policy: RetakePolicy,
) -> GpaSummary {
    let valid_entries = select_retake_entries(
        grades.iter().map(grade_to_entry).collect::<Vec<_>>(),
        policy,
    );

    let mut total_earned_credits = 0.0;
    let mut gpa_credits = 0.0;
    let mut weighted_five = 0.0;
    let mut weighted_four = 0.0;
    let mut weighted_legacy = 0.0;
    let mut weighted_hundred = 0.0;

    let mut major_earned_credits = 0.0;
    let mut major_gpa_credits = 0.0;
    let mut major_weighted_four = 0.0;
    let mut major_weighted_legacy = 0.0;

    for entry in valid_entries {
        if entry.credit <= 0.0 {
            continue;
        }

        let (earns_credit, counts_for_gpa) = classify_grade(&entry);
        let is_major =
            major_course_ids.contains(&entry.xkkh) || major_course_ids.contains(&entry.kcdm);

        if earns_credit {
            total_earned_credits += entry.credit;
            if is_major {
                major_earned_credits += entry.credit;
            }
        }

        if counts_for_gpa {
            gpa_credits += entry.credit;
            weighted_five += entry.credit * entry.five_point;
            weighted_four += entry.credit * entry.four_point;
            weighted_legacy += entry.credit * entry.four_point_legacy;
            weighted_hundred += entry.credit * entry.hundred_point;

            if is_major {
                major_gpa_credits += entry.credit;
                major_weighted_four += entry.credit * entry.four_point;
                major_weighted_legacy += entry.credit * entry.four_point_legacy;
            }
        }
    }

    GpaSummary {
        five_point: safe_div(weighted_five, gpa_credits),
        four_point: safe_div(weighted_four, gpa_credits),
        four_point_legacy: safe_div(weighted_legacy, gpa_credits),
        hundred_point: safe_div(weighted_hundred, gpa_credits),
        total_credits: total_earned_credits,
        major_gpa: safe_div(major_weighted_four, major_gpa_credits),
        major_gpa_legacy: safe_div(major_weighted_legacy, major_gpa_credits),
        major_credits: major_earned_credits,
    }
}

fn select_retake_entries(entries: Vec<GradeEntry>, policy: RetakePolicy) -> Vec<GradeEntry> {
    let mut groups = std::collections::HashMap::<String, Vec<GradeEntry>>::new();
    for entry in entries {
        groups.entry(entry.key.clone()).or_default().push(entry);
    }

    let mut selected = Vec::new();
    for (_, mut group) in groups {
        if group.len() == 1 {
            selected.push(group.remove(0));
            continue;
        }

        let picked = match policy {
            RetakePolicy::First => group
                .into_iter()
                .min_by(|left, right| left.sem_rank.cmp(&right.sem_rank))
                .unwrap(),
            RetakePolicy::Highest => group
                .into_iter()
                .max_by(|left, right| {
                    left.five_point
                        .partial_cmp(&right.five_point)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| {
                            left.hundred_point
                                .partial_cmp(&right.hundred_point)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .then_with(|| right.sem_rank.cmp(&left.sem_rank))
                })
                .unwrap(),
        };
        selected.push(picked);
    }

    selected
}

fn classify_grade(entry: &GradeEntry) -> (bool, bool) {
    grade_flags(
        &entry.xkkh,
        &entry.cj,
        entry.hundred_point,
        entry.five_point,
    )
    .pipe(|(credit_included, gpa_included, earned_credit)| {
        let earns_credit = credit_included && earned_credit;
        (earns_credit, gpa_included)
    })
}

fn grade_to_entry(grade: &Value) -> GradeEntry {
    let xkkh = grade
        .get("xkkh")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let kcdm = grade
        .get("kcdm")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let kcmc = grade
        .get("kcmc")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let sem_name = grade
        .get("semesterName")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .or_else(|| extract_semester_name(grade))
        .unwrap_or_else(|| "9999-9999-99".to_string());

    GradeEntry {
        xkkh: xkkh.clone(),
        kcdm: kcdm.clone(),
        key: grade
            .get("retakeKey")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .unwrap_or_else(|| canonical_course_key(&xkkh, &kcdm, &kcmc)),
        cj: grade
            .get("cj")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        credit: parse_f64(grade.get("credit"))
            .or_else(|| parse_f64(grade.get("xf")))
            .unwrap_or(0.0),
        five_point: parse_f64(grade.get("fivePoint")).unwrap_or(0.0),
        four_point: parse_f64(grade.get("fourPoint")).unwrap_or(0.0),
        four_point_legacy: parse_f64(grade.get("fourPointLegacy")).unwrap_or(0.0),
        hundred_point: parse_f64(grade.get("hundredPoint")).unwrap_or(0.0),
        sem_rank: semester_rank(&sem_name),
    }
}

fn normalize_semester_code(code: &str) -> Option<&'static str> {
    match code.trim() {
        "1" | "3" => Some("1"),
        "2" | "12" => Some("2"),
        _ => None,
    }
}

fn canonical_course_key(xkkh: &str, kcdm: &str, kcmc: &str) -> String {
    let re = Regex::new(r"(\(.*\)-(.*?))-.*").unwrap();
    let mut key = re
        .captures(xkkh)
        .and_then(|caps| caps.get(2).map(|value| value.as_str().to_string()))
        .or_else(|| {
            if xkkh.len() >= 22 {
                xkkh.get(14..22).map(|value| value.to_string())
            } else {
                None
            }
        })
        .or_else(|| (!kcdm.is_empty()).then(|| kcdm.to_string()))
        .or_else(|| (!kcmc.is_empty()).then(|| kcmc.to_string()))
        .unwrap_or_else(|| xkkh.to_string());

    if key.starts_with("PPAE") || key.starts_with("401") {
        key = re
            .captures(xkkh)
            .and_then(|caps| caps.get(1).map(|value| value.as_str().to_string()))
            .or_else(|| {
                xkkh.get(0..std::cmp::min(22, xkkh.len()))
                    .map(|value| value.to_string())
            })
            .unwrap_or(key);
    }

    key
}

fn grade_flags(xkkh: &str, cj: &str, hundred_point: f64, five_point: f64) -> (bool, bool, bool) {
    let grade = cj.trim();
    let is_sports = is_sports_course(xkkh);

    if ["弃修", "待录", "缓考", "无效"].contains(&grade) {
        return (false, false, false);
    }

    if ["合格", "免修", "免考"].contains(&grade) {
        return (true, false, true);
    }

    if grade == "不合格" {
        return (true, false, false);
    }

    if grade == "缺考" || grade == "F" || grade == "不及格" {
        return (true, !is_sports, false);
    }

    let is_letter_grade =
        ["A+", "A", "A-", "B+", "B", "B-", "C+", "C", "C-", "D+", "D"].contains(&grade);
    let is_level_grade = ["及格", "中等", "良好", "优秀"].contains(&grade);
    let numeric_score = grade.parse::<f64>().ok();

    let passed = if is_letter_grade || is_level_grade {
        true
    } else if let Some(score) = numeric_score {
        score >= 60.0
    } else if hundred_point > 0.0 || five_point > 0.0 {
        hundred_point >= 60.0 || five_point > 0.0
    } else {
        false
    };

    let gpa_included = !is_sports
        && (is_letter_grade || is_level_grade || numeric_score.is_some() || grade == "缺考");
    (true, gpa_included, passed)
}

fn is_sports_course(xkkh: &str) -> bool {
    let key = canonical_course_key(xkkh, "", "");
    key.contains("xtwkc") || key.contains("PPAE") || key.contains("401")
}

fn parse_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(|raw| {
        raw.as_f64().or_else(|| {
            raw.as_str()
                .and_then(|text| text.trim().parse::<f64>().ok())
        })
    })
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
        ("弃修", 0.0),
        ("缺考", 0.0),
        ("缓考", 0.0),
        ("待录", 0.0),
        ("无效", 0.0),
    ];

    for (key, value) in mapping {
        if s == key {
            return value;
        }
    }

    if let Ok(value) = s.parse::<f64>() {
        return value;
    }

    let re = Regex::new(r"\d+(?:\.\d+)?").unwrap();
    re.captures(s)
        .and_then(|caps| {
            caps.get(0)
                .and_then(|value| value.as_str().parse::<f64>().ok())
        })
        .unwrap_or(0.0)
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

fn semester_rank(name: &str) -> i64 {
    let re = Regex::new(r"^(\d{4})-(\d{4})-(\d+)$").unwrap();
    if let Some(caps) = re.captures(name) {
        let year = caps
            .get(1)
            .and_then(|value| value.as_str().parse::<i64>().ok())
            .unwrap_or(9999);
        let semester = caps
            .get(3)
            .and_then(|value| value.as_str().parse::<i64>().ok())
            .unwrap_or(99);
        return year * 100 + semester;
    }
    9_999_999
}

fn safe_div(numerator: f64, denominator: f64) -> f64 {
    if denominator <= 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::{
        canonical_course_key, compute_gpa_by_policy, enrich_grade, extract_semester_name,
        GpaSummary, RetakePolicy,
    };
    use serde_json::json;
    use std::collections::HashSet;

    fn g(cj: &str, xf: f64, xkkh: &str, kcdm: &str) -> serde_json::Value {
        enrich_grade(&json!({
            "cj": cj,
            "xf": xf,
            "xkkh": xkkh,
            "kcdm": kcdm,
            "kcmc": kcdm,
            "xnm": "2024",
            "xqm": "3",
        }))
    }

    fn summary(grades: Vec<serde_json::Value>) -> GpaSummary {
        compute_gpa_by_policy(&grades, &HashSet::new(), RetakePolicy::First)
    }

    #[test]
    fn term_extraction_normalizes_semester_alias() {
        let grade = json!({ "xnm": "2024", "xqm": "12" });
        assert_eq!(
            extract_semester_name(&grade).as_deref(),
            Some("2024-2025-2")
        );
    }

    #[test]
    fn retake_first_vs_highest_follow_flutter_key() {
        let first = g("70", 3.0, "(2024-2025-1)-211G0001-0001-1", "211G0001");
        let retake = enrich_grade(&json!({
            "cj": "90",
            "xf": 3.0,
            "xkkh": "(2024-2025-2)-211G0001-0001-1",
            "kcdm": "211G0001",
            "kcmc": "211G0001",
            "xnm": "2024",
            "xqm": "12",
        }));

        let grades = vec![first.clone(), retake.clone()];
        let first_summary = compute_gpa_by_policy(&grades, &HashSet::new(), RetakePolicy::First);
        let highest_summary =
            compute_gpa_by_policy(&grades, &HashSet::new(), RetakePolicy::Highest);

        assert!(first_summary.hundred_point < highest_summary.hundred_point);
        assert_eq!(
            canonical_course_key("(2024-2025-1)-211G0001-0001-1", "211G0001", ""),
            "211G0001"
        );
    }

    #[test]
    fn sports_courses_do_not_merge_across_terms() {
        let fall = g("90", 1.0, "(2024-2025-1)-PPAE0001-0001-1", "PPAE0001");
        let spring = enrich_grade(&json!({
            "cj": "95",
            "xf": 1.0,
            "xkkh": "(2024-2025-2)-PPAE0001-0001-1",
            "kcdm": "PPAE0001",
            "kcmc": "体育",
            "xnm": "2024",
            "xqm": "12",
        }));
        let sum =
            compute_gpa_by_policy(&vec![fall, spring], &HashSet::new(), RetakePolicy::Highest);
        assert_eq!(sum.total_credits, 2.0);
        assert_eq!(sum.five_point, 0.0);
    }

    #[test]
    fn pending_pass_fail_and_absent_rules_are_stable() {
        let grades = vec![
            g("待录", 2.0, "(2024-2025-1)-A-1", "A0000001"),
            g("合格", 1.0, "(2024-2025-1)-B-1", "B0000001"),
            g("不合格", 1.0, "(2024-2025-1)-C-1", "C0000001"),
            g("缺考", 3.0, "(2024-2025-1)-D-1", "D0000001"),
        ];
        let result = summary(grades);
        assert_eq!(result.total_credits, 1.0);
        assert_eq!(result.hundred_point, 0.0);
        assert_eq!(result.five_point, 0.0);
    }

    #[test]
    fn major_calculation_counts_major_only() {
        let major = g("95", 3.0, "(2024-2025-1)-M-1", "M0000001");
        let common = g("80", 2.0, "(2024-2025-1)-C-1", "C0000001");
        let mut major_ids = HashSet::new();
        major_ids.insert("M0000001".to_string());
        let result = compute_gpa_by_policy(&vec![major, common], &major_ids, RetakePolicy::First);
        assert!(result.major_gpa > result.four_point);
        assert_eq!(result.major_credits, 3.0);
    }
}
