use crate::util::setenv;
use open;
use reqwest::header::HeaderMap;
use std::env;
use std::io::{self, Write};

async fn login() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛑 No account found");
    println!("🔒 Initiating login...");
    let app_key = env::var("APP_KEY").unwrap();
    let app_secret = env::var("APP_SECRET").unwrap();
    let url = format!("https://www.dropbox.com/oauth2/authorize?client_id={}&token_access_type=offline&response_type=code", app_key);
    println!("🚦 Log in to DropBox (if you're not already)");
    println!("🌐 Open this URL in your browser:");
    println!("🌐 {}", url);
    let _ = open::that(url);
    println!("🌐 (one might have opened already)");
    println!("🔐 and authorize the app.");

    fn prompt(msg: &str) -> String {
        eprint!("{}: ", msg);
        io::stderr().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        input.trim().to_owned()
    }

    let authorization_code = prompt("🪪  Paste the authorization code you see here");

    println!("🔐 Requesting access token...");
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    let body = format!(
        "code={}&grant_type=authorization_code&client_id={}&client_secret={}",
        authorization_code, app_key, app_secret
    );
    let client = reqwest::Client::new();
    let res = client
        .post("https://api.dropbox.com/oauth2/token")
        .headers(headers)
        .body(body)
        .send()
        .await?
        .text()
        .await?;
    let json = serde_json::from_str::<serde_json::Value>(&res).unwrap();
    assert_eq!(json.get("error"), None, "🛑 Not logged in");
    let refresh_token = json.get("refresh_token").unwrap().to_string().to_owned();
    let access_token = json.get("access_token").unwrap().to_string().to_owned();
    match setenv(
        "AUTHORIZATION_CODE",
        format!("\"{}\"", authorization_code.clone()),
    ) {
        Ok(_) => println!("🔑 Authorization code set"),
        Err(err) => println!("{err}"),
    }
    match setenv("REFRESH_TOKEN", refresh_token) {
        Ok(_) => println!("🔑 Refresh token set"),
        Err(err) => println!("{err}"),
    }
    match setenv("ACCESS_TOKEN", access_token) {
        Ok(_) => println!("🔑 Access token set"),
        Err(err) => println!("{err}"),
    }
    Ok(())
}

async fn refresh_token() -> Result<(), Box<dyn std::error::Error>> {
    let refresh_token = env::var("REFRESH_TOKEN").unwrap();
    let app_key = env::var("APP_KEY").unwrap();
    let app_secret = env::var("APP_SECRET").unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    let body = format!(
        "refresh_token={}&grant_type=refresh_token&client_id={}&client_secret={}",
        refresh_token, app_key, app_secret
    );
    let client = reqwest::Client::new();
    let res = client
        .post("https://api.dropbox.com/oauth2/token")
        .headers(headers)
        .body(body)
        .send()
        .await?
        .text()
        .await?;
    let json = serde_json::from_str::<serde_json::Value>(&res).unwrap();
    match json.get("error_summary").map(|s| s.as_str().unwrap()) {
        Some(result) => panic!("🛑 {result}"),
        None => {
            let access_token = json.get("access_token").unwrap().to_string().to_owned();
            assert_ne!(access_token, "null", "🛑  Access Token Null");
            assert_ne!(
                access_token,
                env::var("ACCESS_TOKEN").unwrap(),
                "🛑  Access Token Unchanged"
            );
            match setenv("ACCESS_TOKEN", access_token) {
                Ok(_) => Ok(println!("🔑 Access token set")),
                Err(err) => panic!("{err}"),
            }
        }
    }
}

#[async_recursion::async_recursion(?Send)]
pub async fn check_account() {
    println!("🪪  Checking account...");
    let access_token =
        env::var("ACCESS_TOKEN").unwrap_or("❌  Could not find access token.".to_string());
    let team_member_id = env::var("TEAM_MEMBER_ID").unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {}", access_token).parse().unwrap(),
    );
    headers.insert(
        "Dropbox-API-Select-Admin",
        format!("{}", team_member_id).parse().unwrap(),
    );
    let client = reqwest::Client::new();
    let res = client
        .post("https://api.dropboxapi.com/2/users/get_current_account")
        .headers(headers)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let json = serde_json::from_str::<serde_json::Value>(&res).unwrap();
    match json.get("error_summary").map(|s| s.as_str().unwrap()) {
        Some("expired_access_token/") => {
            println!("🔑 Access token expired");
            match refresh_token().await {
                Ok(_) => println!("🔑 Refreshed access token"),
                Err(err) => println!("{}", err),
            }
        }
        Some("invalid_access_token/") => {
            println!("🔑 Access token invalid");
            match login().await {
                Ok(_) => println!("🔑 Logged in"),
                Err(err) => println!("{}", err),
            }
        }
        Some(result) => println!("{result}"),
        None => {
            println!("👤 Logged in as {}", json.get("email").unwrap());
        }
    }
}
