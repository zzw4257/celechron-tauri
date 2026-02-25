use reqwest::Client;
use serde_json::Value;
use tokio::sync::Mutex;

/// Shared application state holding session cookies across all services.
pub struct AppState {
    pub client: Client,
    pub iplanet_cookie: Mutex<Option<String>>,
    pub zdbk_jsessionid: Mutex<Option<String>>,
    pub zdbk_route: Mutex<Option<String>>,
    pub courses_session: Mutex<Option<String>>,
    pub username: Mutex<Option<String>>,
}

impl AppState {
    pub fn new() -> Self {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            iplanet_cookie: Mutex::new(None),
            zdbk_jsessionid: Mutex::new(None),
            zdbk_route: Mutex::new(None),
            courses_session: Mutex::new(None),
            username: Mutex::new(None),
        }
    }
}

/// ZJU AM CAS login. Returns the iPlanetDirectoryPro cookie value on success.
pub async fn login_zju(state: &AppState, username: &str, password: &str) -> Result<String, String> {
    let client = &state.client;
    let login_url = "https://zjuam.zju.edu.cn/cas/login";

    // 1. Fetch login page to get execution token + cookies
    let res = client.get(login_url).send().await.map_err(|e| format!("网络错误: {}", e))?;
    
    // Collect Set-Cookie headers
    let initial_cookies: Vec<String> = res.headers().get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| {
            // Extract "NAME=VALUE" from "NAME=VALUE; Path=...; ..."
            s.split(';').next().unwrap_or("").to_string()
        })
        .collect();
    
    let body = res.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
    let execution = extract_execution(&body).ok_or("无法获取execution")?;

    // 2. Fetch RSA public key
    let pubkey_url = "https://zjuam.zju.edu.cn/cas/v2/getPubKey";
    let pubkey_res = client.get(pubkey_url)
        .header("Cookie", initial_cookies.join("; "))
        .send().await.map_err(|e| format!("获取公钥失败: {}", e))?;
    
    // Merge new cookies
    let mut all_cookies = initial_cookies.clone();
    for hv in pubkey_res.headers().get_all("set-cookie").iter() {
        if let Ok(s) = hv.to_str() {
            all_cookies.push(s.split(';').next().unwrap_or("").to_string());
        }
    }
    
    let pubkey_json: Value = pubkey_res.json().await.map_err(|e| format!("解析公钥失败: {}", e))?;
    let modulus = pubkey_json["modulus"].as_str().ok_or("无法获取modulus")?;
    let exponent = pubkey_json["exponent"].as_str().ok_or("无法获取exponent")?;

    // 3. Encrypt password using RSA (ZJU AM style: raw BigInt modPow)
    let mod_int = num_bigint::BigUint::parse_bytes(modulus.as_bytes(), 16).ok_or("modulus解析失败")?;
    let exp_int = num_bigint::BigUint::parse_bytes(exponent.as_bytes(), 16).ok_or("exponent解析失败")?;
    let pwd_hex: String = password.as_bytes().iter().map(|b| format!("{:02x}", b)).collect();
    let pwd_int = num_bigint::BigUint::parse_bytes(pwd_hex.as_bytes(), 16).ok_or("密码编码失败")?;
    let enc_int = pwd_int.modpow(&exp_int, &mod_int);
    let enc_password = format!("{:0>128}", format!("{:x}", enc_int));

    // 4. POST login
    let form_body = format!(
        "username={}&password={}&execution={}&_eventId=submit&rememberMe=true",
        username, enc_password, execution
    );
    
    let login_res = client.post(login_url)
        .header("Cookie", all_cookies.join("; "))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body)
        .send().await.map_err(|e| format!("登录请求失败: {}", e))?;

    // 5. Extract iPlanetDirectoryPro from Set-Cookie
    let mut iplanet_value: Option<String> = None;
    for hv in login_res.headers().get_all("set-cookie").iter() {
        if let Ok(s) = hv.to_str() {
            if s.starts_with("iPlanetDirectoryPro=") {
                let val = s.split(';').next().unwrap_or("")
                    .strip_prefix("iPlanetDirectoryPro=").unwrap_or("").to_string();
                if !val.is_empty() {
                    iplanet_value = Some(val);
                }
            }
        }
    }

    match iplanet_value {
        Some(cookie) => {
            *state.iplanet_cookie.lock().await = Some(cookie.clone());
            *state.username.lock().await = Some(username.to_string());
            Ok(cookie)
        }
        None => Err("学号或密码错误".to_string()),
    }
}

fn extract_execution(body: &str) -> Option<String> {
    let re = regex::Regex::new(r#"name="execution" value="([^"]+)""#).ok()?;
    re.captures(body).map(|caps| caps[1].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_login_zju() {
        let state = AppState::new();
        let result = login_zju(&state, "***REMOVED***", "***REMOVED***").await;
        println!("Login result: {:?}", result);
        assert!(result.is_ok(), "Login failed: {:?}", result.err());
        let cookie = state.iplanet_cookie.lock().await;
        assert!(cookie.is_some(), "iPlanetDirectoryPro cookie not set");
        println!("iPlanetDirectoryPro: {}", cookie.as_ref().unwrap());
    }
}
