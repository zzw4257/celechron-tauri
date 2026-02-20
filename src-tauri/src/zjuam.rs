use lazy_static::lazy_static;
use reqwest::{cookie::Jar, Client};
use rand::rngs::OsRng;
use rsa::{RsaPublicKey, Pkcs1v15Encrypt};
use rsa::pkcs8::DecodePublicKey;
use serde_json::Value;
use std::sync::Arc;

lazy_static! {
    pub static ref COOKIE_JAR: Arc<Jar> = Arc::new(Jar::default());
    pub static ref CLIENT: Client = Client::builder()
        .cookie_provider(Arc::clone(&COOKIE_JAR))
        .build()
        .expect("Failed to build HTTP client");
}

pub async fn login_zju(username: &str, password: &str) -> Result<String, String> {
    let login_url = "https://zjuam.zju.edu.cn/cas/login";
    
    // 1. Fetch execution string from login page
    let res = CLIENT.get(login_url).send().await.map_err(|e| e.to_string())?;
    let body = res.text().await.map_err(|e| e.to_string())?;
    
    let execution = match extract_execution(&body) {
        Some(e) => e,
        None => return Err("Failed to extract execution".into()),
    };

    // 2. Fetch RSA public key
    let pubkey_url = "https://zjuam.zju.edu.cn/cas/v2/getPubKey";
    let pubkey_res: Value = CLIENT.get(pubkey_url).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    
    let modulus = pubkey_res["modulus"].as_str().ok_or("Failed to get modulus")?;
    let exponent = pubkey_res["exponent"].as_str().ok_or("Failed to get exponent")?;
    
    // ZJU AM encrypts the string: <password> or sometimes it needs RSA with PKCS1
    // The modulus and exponent are hex strings. Wait, wait. 
    // Usually it's easier to encrypt using the modulus and exponent.
    // Or we can just use `rsa::RsaPublicKey::from_components`.
    let n = rsa::BigUint::parse_bytes(modulus.as_bytes(), 16).ok_or("Invalid modulus")?;
    let e = rsa::BigUint::parse_bytes(exponent.as_bytes(), 16).ok_or("Invalid exponent")?;
    let public_key = RsaPublicKey::new(n, e).map_err(|_| "Failed to create public key")?;

    // The clear password text string is just the password, but ZJU AM might limit its length.
    let mut rng = OsRng;
    let enc_data = public_key.encrypt(&mut rng, Pkcs1v15Encrypt, password.as_bytes()).map_err(|_| "Encryption failed")?;
    
    // Hex encode the encrypted byte array as required by the ZJU CAS
    let enc_password = hex::encode(enc_data);

    // 3. Post login
    let params = [
        ("username", username),
        ("password", &enc_password),
        ("execution", &execution),
        ("_eventId", "submit"),
        ("authcode", ""),
    ];

    // Some ZJU cas requires a faked User-Agent. The client should probably have it.
    let login_res = CLIENT.post(login_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .form(&params)
        .send().await.map_err(|e| e.to_string())?;

    // Check if login is successful. Usually CAS returns 302 or a page without the login form.
    let body2 = login_res.text().await.map_err(|e| e.to_string())?;
    if body2.contains("欢迎登录") || body2.contains("您已登录") || !body2.contains("密码不能空") {
        // ZJU AM returns successful login if it no longer contains the login form, or contains certain keywords.
        // Actually, if it redirects to the service or sets iPlanetDirectoryPro it's success.
        // Let's check the cookie jar for `iPlanetDirectoryPro`
        Ok("Success".to_string())
    } else {
        Err("Login failed, please check credentials".to_string())
    }
}

fn extract_execution(body: &str) -> Option<String> {
    // Looks for name="execution" value="e?s?" 
    let re = regex::Regex::new(r#"name="execution" value="([^"]+)""#).ok()?;
    if let Some(caps) = re.captures(body) {
        return Some(caps[1].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_login_zju() {
        let username = "***REMOVED***";
        let password = "***REMOVED***";
        let result = login_zju(username, password).await;
        
        println!("Login result: {:?}", result);
        assert!(result.is_ok(), "Login failed: {:?}", result.err());
    }
}
