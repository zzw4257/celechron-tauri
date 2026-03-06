use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAnalysisInput {
    pub base_url: String,
    pub api_key: Option<String>,
    pub prompt: String,
    pub context: Value,
    pub request_body: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DingtalkTestInput {
    pub webhook_url: String,
    pub secret: Option<String>,
    pub title: Option<String>,
    pub text: Option<String>,
}

type HmacSha256 = Hmac<Sha256>;

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn extract_markdown(value: &Value) -> Option<String> {
    value
        .get("markdown")
        .and_then(Value::as_str)
        .map(|text| text.to_string())
        .or_else(|| {
            value
                .get("content")
                .and_then(Value::as_str)
                .map(|text| text.to_string())
        })
        .or_else(|| {
            value
                .get("answer")
                .and_then(Value::as_str)
                .map(|text| text.to_string())
        })
        .or_else(|| value.get("data").and_then(extract_markdown))
        .or_else(|| {
            value
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("message"))
                .and_then(|message| message.get("content"))
                .and_then(Value::as_str)
                .map(|text| text.to_string())
        })
}

fn normalize_ai_url(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err("ZeroClaw endpoint 不能为空".to_string());
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err("ZeroClaw endpoint 必须以 http:// 或 https:// 开头".to_string());
    }
    Ok(trimmed.to_string())
}

pub async fn run_ai_analysis(input: AiAnalysisInput) -> Result<Value, String> {
    let url = normalize_ai_url(&input.base_url)?;
    let client = reqwest::Client::new();
    let request_body = input.request_body.unwrap_or_else(|| {
        json!({
            "prompt": input.prompt,
            "context": input.context,
            "stream": false,
        })
    });

    let mut request = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .json(&request_body);

    if let Some(api_key) = input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header(AUTHORIZATION, format!("Bearer {api_key}"));
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("调用 ZeroClaw 失败: {error}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取 ZeroClaw 响应失败: {error}"))?;

    if !status.is_success() {
        return Err(format!(
            "ZeroClaw 返回异常: HTTP {} - {}",
            status.as_u16(),
            text
        ));
    }

    let raw = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "text": text }));
    let markdown = extract_markdown(&raw)
        .or_else(|| {
            raw.get("text")
                .and_then(Value::as_str)
                .map(|value| value.to_string())
        })
        .unwrap_or_default();

    Ok(json!({
        "markdown": markdown,
        "raw": raw,
        "provider": "zeroclaw",
    }))
}

fn sign_dingtalk_url(webhook_url: &str, secret: Option<&str>) -> Result<String, String> {
    let trimmed = webhook_url.trim();
    if trimmed.is_empty() {
        return Err("DingTalk webhook 不能为空".to_string());
    }
    let Some(secret) = secret.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(trimmed.to_string());
    };

    let timestamp = now_millis();
    let string_to_sign = format!("{timestamp}\n{secret}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|error| format!("DingTalk secret 无效: {error}"))?;
    mac.update(string_to_sign.as_bytes());
    let sign = STANDARD.encode(mac.finalize().into_bytes());
    let separator = if trimmed.contains('?') { '&' } else { '?' };
    Ok(format!(
        "{trimmed}{separator}timestamp={timestamp}&sign={}",
        urlencoding::encode(&sign)
    ))
}

pub async fn send_dingtalk_test(input: DingtalkTestInput) -> Result<Value, String> {
    let signed_url = sign_dingtalk_url(&input.webhook_url, input.secret.as_deref())?;
    let title = input
        .title
        .unwrap_or_else(|| "Celechron 测试消息".to_string());
    let text = input.text.unwrap_or_else(|| {
        "### Celechron 连通性测试\n\n- 渠道：DingTalk Webhook\n- 状态：已成功从客户端发起测试消息".to_string()
    });

    let response = reqwest::Client::new()
        .post(signed_url)
        .json(&json!({
            "msgtype": "markdown",
            "markdown": {
                "title": title,
                "text": text,
            }
        }))
        .send()
        .await
        .map_err(|error| format!("发送 DingTalk 测试消息失败: {error}"))?;
    let status = response.status();
    let raw = response
        .text()
        .await
        .map_err(|error| format!("读取 DingTalk 响应失败: {error}"))?;

    if !status.is_success() {
        return Err(format!(
            "DingTalk 返回异常: HTTP {} - {}",
            status.as_u16(),
            raw
        ));
    }

    let parsed = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({ "text": raw }));
    Ok(json!({
        "ok": parsed.get("errcode").and_then(Value::as_i64).unwrap_or(0) == 0,
        "raw": parsed,
    }))
}
