use crate::json::{self, JSON};
use crate::util::setenv;
// use aws_smithy_http::http;
use crate::http::{self, HTTPClient, HeaderMap};
use open;
use std::env;
use std::io::{self, Write};

async fn login(http: &HTTPClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("🛑 No account found");
    println!("🔒 Initiating login...");
    let app_key = env::var("APP_KEY").unwrap();
    let app_secret = env::var("APP_SECRET").unwrap();
    let url = format!("https://www.dropbox.com/oauth2/authorize?client_id={}&token_access_type=offline&response_type=code", app_key);
    println!("🚦 Log in to DropBox (if you're not already)");
    println!("🌐 Open this URL in your browser:");
    println!("🌐 {}", url);
    open::that(url).unwrap();
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
    let res = http
        .post("https://api.dropbox.com/oauth2/token")
        .headers(headers)
        .body(body)
        .send()
        .await?
        .text()
        .await?;
    let json = json::from_res(&res);
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
        Ok(_) => println!("🔑 Login: Access token set"),
        Err(err) => println!("{err}"),
    }
    Ok(())
}

async fn refresh_token(http: &HTTPClient) -> Result<(), Box<dyn std::error::Error>> {
    let refresh_token = env::var("REFRESH_TOKEN").unwrap();
    let app_key = env::var("APP_KEY").unwrap();
    let app_secret = env::var("APP_SECRET").unwrap();
    let mut headers = HeaderMap::new();
    headers = http::dropbox_content_type_x_www_form_urlencoded_header(&mut headers);
    let body = format!(
        "refresh_token={}&grant_type=refresh_token&client_id={}&client_secret={}",
        refresh_token, app_key, app_secret
    );
    let res = http
        .post("https://api.dropbox.com/oauth2/token")
        .headers(headers)
        .body(body)
        .send()
        .await?
        .text()
        .await?;
    let json = json::from_res(&res);
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
                Ok(_) => Ok(println!("🔑 Refresh: Access token set")),
                Err(err) => panic!("{err}"),
            }
        }
    }
}

async fn get_current_account(http_client: &HTTPClient) -> JSON {
    let mut headers = HeaderMap::new();
    let access_token = env::var("ACCESS_TOKEN").unwrap();
    headers.insert(
        "Authorization",
        format!("Bearer {}", access_token).parse().unwrap(),
    );
    let team_member_id = env::var("TEAM_MEMBER_ID").unwrap();
    headers.insert(
        "Dropbox-API-Select-Admin",
        format!("{}", team_member_id).parse().unwrap(),
    );
    let res = http_client
        .post("https://api.dropboxapi.com/2/users/get_current_account")
        .headers(headers)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    json::from_res(&res)
}

#[async_recursion::async_recursion(?Send)]
pub async fn check_account(http: &reqwest::Client) {
    print!("\n🪪  Checking account...\n");
    let current_account = get_current_account(&http).await;
    match current_account.get("email") {
        Some(email) => return println!("👤 Logged in as {email}"),
        None => {
            println!("🚫  No account found");
            login(&http).await.unwrap()
        }
    }
    match current_account
        .get("error_summary")
        .map(|s| s.as_str().unwrap())
    {
        Some("expired_access_token/") => {
            println!("🚫  Access token expired");
            match refresh_token(&http).await {
                Ok(_) => println!("🔑  Refreshed access token"),
                Err(err) => panic!("❌  {err}"),
            }
        }
        Some("invalid_access_token/") => {
            println!("🚫  Access token invalid");
            match login(&http).await {
                Ok(_) => {
                    println!("🔑  Logged in");
                    check_account(&http).await.try_into().unwrap()
                }
                Err(err) => panic!("{err}"),
            }
        }
        Some(result) => panic!("❌  {result}"),
        None => (),
    }
}
