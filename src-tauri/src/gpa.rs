use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

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

#[derive(Clone, Debug, Serialize)]
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
    if let Some(id) = grade.get("xkkh").and_then(|v| v.as_str()) {
        let re = Regex::new(r"\(([^)]+)\)").ok()?;
        if let Some(caps) = re.captures(id) {
            return caps.get(1).map(|m| m.as_str().to_string());
        }
    }

    let xnm = grade
        .get("xnm")
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        })
        .unwrap_or_default();

    let xqm = grade
        .get("xqm")
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        })
        .unwrap_or_default();

    if !xnm.is_empty() && !xqm.is_empty() {
        return Some(format!("{}-{}-{}", xnm, xnm.parse::<u32>().unwrap_or(0) + 1, xqm));
    }

    None
}

pub fn enrich_grade(raw: &Value) -> Value {
    let mut enriched = raw.clone();
    let cj = raw
        .get("cj")
        .and_then(|v| v.as_str())
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

    let four_point = parse_f64(raw.get("fourPoint")).unwrap_or_else(|| to_four_point_43(five_point));
    let four_point_legacy =
        parse_f64(raw.get("fourPointLegacy")).unwrap_or_else(|| to_four_point_legacy(five_point));

    if let Some(obj) = enriched.as_object_mut() {
        obj.insert("credit".to_string(), json!(credit));
        obj.insert("hundredPoint".to_string(), json!(hundred_point));
        obj.insert("fivePoint".to_string(), json!(five_point));
        obj.insert("fourPoint".to_string(), json!(four_point));
        obj.insert("fourPointLegacy".to_string(), json!(four_point_legacy));
    }

    enriched
}

pub fn apply_simulated_score(grade: &mut Value, score: f64) {
    let hundred = score.max(0.0).min(100.0);
    let five = to_five_point(hundred);
    let four = to_four_point_43(five);
    let legacy = to_four_point_legacy(five);

    if let Some(obj) = grade.as_object_mut() {
        obj.insert("cj".to_string(), json!(format!("{:.2}", hundred)));
        obj.insert("hundredPoint".to_string(), json!(hundred));
        obj.insert("fivePoint".to_string(), json!(five));
        obj.insert("fourPoint".to_string(), json!(four));
        obj.insert("fourPointLegacy".to_string(), json!(legacy));
    }
}

pub fn compute_gpa_by_policy(
    grades: &[Value],
    major_course_ids: &HashSet<String>,
    policy: RetakePolicy,
) -> GpaSummary {
    let entries = grades
        .iter()
        .map(grade_to_entry)
        .collect::<Vec<_>>();

    let valid_entries = select_retake_entries(entries, policy);

    let mut total_earned_credits = 0.0_f64;
    let mut gpa_credits = 0.0_f64;
    let mut weighted_five = 0.0_f64;
    let mut weighted_four = 0.0_f64;
    let mut weighted_legacy = 0.0_f64;
    let mut weighted_hundred = 0.0_f64;

    let mut major_earned_credits = 0.0_f64;
    let mut major_gpa_credits = 0.0_f64;
    let mut major_weighted_four = 0.0_f64;
    let mut major_weighted_legacy = 0.0_f64;

    for entry in valid_entries {
        if entry.credit <= 0.0 {
            continue;
        }

        let (earns_credit, counts_for_gpa) = classify_grade(&entry);
        let is_major = major_course_ids.contains(&entry.xkkh) || major_course_ids.contains(&entry.kcdm);

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
    let mut groups: HashMap<String, Vec<GradeEntry>> = HashMap::new();
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
                .min_by(|a, b| a.sem_rank.cmp(&b.sem_rank))
                .unwrap(),
            RetakePolicy::Highest => group
                .into_iter()
                .max_by(|a, b| {
                    a.five_point
                        .partial_cmp(&b.five_point)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| {
                            a.hundred_point
                                .partial_cmp(&b.hundred_point)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .then_with(|| a.sem_rank.cmp(&b.sem_rank))
                })
                .unwrap(),
        };

        selected.push(picked);
    }

    selected
}

fn classify_grade(entry: &GradeEntry) -> (bool, bool) {
    let cj = entry.cj.trim();

    if entry.xkkh.contains("xtwkc") {
        return (entry.five_point > 0.0, false);
    }

    if ["待录", "缓考", "无效"].contains(&cj) {
        return (false, false);
    }

    if cj == "弃修" {
        return (false, false);
    }

    if ["合格", "免修", "免考"].contains(&cj) {
        return (true, false);
    }

    if cj == "不合格" {
        return (false, false);
    }

    if ["不及格", "F"].contains(&cj) {
        return (false, true);
    }

    let is_letter_grade = [
        "A+", "A", "A-", "B+", "B", "B-", "C+", "C", "C-", "D+", "D",
    ]
    .contains(&cj);

    if is_letter_grade {
        return (true, true);
    }

    if let Ok(score) = cj.parse::<f64>() {
        return (score >= 60.0, true);
    }

    let is_level_grade = ["及格", "中等", "良好", "优秀"].contains(&cj);
    if is_level_grade {
        return (true, true);
    }

    if entry.hundred_point > 0.0 || entry.five_point > 0.0 {
        return (entry.hundred_point >= 60.0, true);
    }

    (false, false)
}

fn grade_to_entry(grade: &Value) -> GradeEntry {
    let xkkh = grade
        .get("xkkh")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let kcdm = grade
        .get("kcdm")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let kcmc = grade
        .get("kcmc")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let key = if !kcdm.is_empty() {
        kcdm.clone()
    } else if !kcmc.is_empty() {
        kcmc.clone()
    } else if !xkkh.is_empty() {
        xkkh.clone()
    } else {
        "unknown".to_string()
    };

    let cj = grade
        .get("cj")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();

    let sem_name = extract_semester_name(grade).unwrap_or_else(|| "9999-9999-99".to_string());

    GradeEntry {
        xkkh,
        kcdm,
        key,
        cj,
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

fn parse_f64(v: Option<&Value>) -> Option<f64> {
    v.and_then(|v| {
        v.as_f64().or_else(|| {
            v.as_str()
                .and_then(|s| s.trim().parse::<f64>().ok())
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
    ];

    for (k, v) in mapping {
        if s == k {
            return v;
        }
    }

    if let Ok(v) = s.parse::<f64>() {
        return v;
    }

    let re = Regex::new(r"\d+(?:\.\d+)?").unwrap();
    if let Some(caps) = re.captures(s) {
        if let Ok(v) = caps[0].parse::<f64>() {
            return v;
        }
    }

    0.0
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
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or(9999);
        let sem = caps
            .get(3)
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or(99);
        return year * 100 + sem;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn g(cj: &str, xf: f64, xkkh: &str, kcdm: &str) -> Value {
        json!({
            "cj": cj,
            "xf": xf,
            "xkkh": xkkh,
            "kcdm": kcdm,
            "kcmc": kcdm,
            "credit": xf,
            "fivePoint": to_five_point(parse_score(cj)),
            "fourPoint": to_four_point_43(to_five_point(parse_score(cj))),
            "fourPointLegacy": to_four_point_legacy(to_five_point(parse_score(cj))),
            "hundredPoint": parse_score(cj)
        })
    }

    #[test]
    fn test_pending_and_pass_fail_rules() {
        let grades = vec![
            g("95", 3.0, "(2023-2024-1)-A", "A"),
            g("待录", 2.0, "(2023-2024-1)-B", "B"),
            g("合格", 1.0, "(2023-2024-1)-C", "C"),
            g("不及格", 2.0, "(2023-2024-1)-D", "D"),
        ];

        let summary = compute_gpa_by_policy(&grades, &HashSet::new(), RetakePolicy::First);
        assert!((summary.total_credits - 4.0).abs() < 1e-6);
        assert!(summary.four_point > 0.0);
    }

    #[test]
    fn test_retake_first_vs_highest() {
        let grades = vec![
            g("60", 3.0, "(2022-2023-1)-COURSE-1", "COURSE"),
            g("95", 3.0, "(2023-2024-1)-COURSE-2", "COURSE"),
        ];

        let first = compute_gpa_by_policy(&grades, &HashSet::new(), RetakePolicy::First);
        let highest = compute_gpa_by_policy(&grades, &HashSet::new(), RetakePolicy::Highest);

        assert!(highest.five_point > first.five_point);
    }

    #[test]
    fn test_major_calculation() {
        let grades = vec![
            g("90", 3.0, "(2023-2024-1)-M1", "M1"),
            g("85", 3.0, "(2023-2024-1)-N1", "N1"),
        ];

        let mut major = HashSet::new();
        major.insert("(2023-2024-1)-M1".to_string());
        let summary = compute_gpa_by_policy(&grades, &major, RetakePolicy::First);

        assert!(summary.major_credits > 0.0);
        assert!(summary.major_gpa > 0.0);
    }
}
