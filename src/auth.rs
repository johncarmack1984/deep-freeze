use crate::db::{self, DBConnection};
use crate::http::{self, HTTPClient, HeaderMap};
use crate::json::{self, JSON};
use crate::util::{prompt, setenv};
use open;
use std::env;
use std::io::{self, Write};

async fn login(http: &HTTPClient) -> String {
    println!("🔒 Initiating login...");
    get_authorization_code().await;
    println!("🔐 Requesting access token...");
    let mut headers = HeaderMap::new();
    headers = http::dropbox_content_type_x_www_form_urlencoded_header(&mut headers);
    let body = http::dropbox_oauth2_token_body();
    match http
        .post("https://api.dropbox.com/oauth2/token")
        .headers(headers)
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
    {
        Ok(res) => match res.contains("error") {
            true => handle_auth_error(&http, res).await,
            false => {
                let json = json::from_res(&res);
                let refresh_token = json.get("refresh_token").unwrap().to_string().to_owned();
                let access_token = json.get("access_token").unwrap().to_string().to_owned();
                setenv("REFRESH_TOKEN", refresh_token);
                println!("🔑 Refresh token set");
                setenv("ACCESS_TOKEN", access_token);
                println!("🔑 Login: Access token set");
                res
            }
        },
        Err(err) => panic!("❌ {err}"),
    }
}

async fn get_authorization_code() {
    let url = http::dropbox_authorization_code_url();
    print!("\n🚦 You need to be logged in to DropBox\n\n");
    open::that_detached(&url).unwrap();
    println!("🌐 Open this URL in your browser (one might have opened already):");
    print!("\n🌐 {}\n\n", url);
    println!("🔐 and authorize the app.");
    let authorization_code = prompt("🪪  Paste the authorization code you see here");
    setenv("AUTHORIZATION_CODE", format!("\"{}\"", authorization_code));
    println!("🔑 Authorization code set");
}

async fn refresh_token(http: &HTTPClient) -> String {
    dbg!("refresh_token");
    println!("🔑 Refreshing access token...");
    let mut headers = HeaderMap::new();
    headers = http::dropbox_content_type_x_www_form_urlencoded_header(&mut headers);
    let body = http::dropbox_refresh_token_body();
    match http
        .post("https://api.dropbox.com/oauth2/token")
        .headers(headers)
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
    {
        Ok(res) => match res.contains("error") {
            true => handle_auth_error(&http, res).await,
            false => {
                let json = json::from_res(&res);
                let access_token = json.get("access_token").unwrap().to_string().to_owned();
                setenv("ACCESS_TOKEN", access_token);
                get_current_account(&http).await
            }
        },
        Err(err) => panic!("❌ {err}"),
    }
}

async fn get_current_account(http: &HTTPClient) -> String {
    let mut headers = http::HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_select_admin_header(&mut headers);
    match http
        .post("https://api.dropboxapi.com/2/users/get_current_account")
        .headers(headers)
        .send()
        .await
        .unwrap()
        .text()
        .await
    {
        Ok(res) => match res.contains("error") {
            true => handle_auth_error(&http, res).await,
            false => res,
        },
        Err(err) => panic!("❌ {err}"),
    }
}

#[async_recursion::async_recursion(?Send)]
async fn handle_auth_error(http: &HTTPClient, res: String) -> String {
    println!("❌  Error in auth");
    let json = json::from_res(&res);
    match json
        .get("error")
        .unwrap()
        .get(".tag")
        .unwrap()
        .as_str()
        .unwrap()
    {
        "expired_access_token" => {
            println!("🚫  Access token expired");
            refresh_token(http).await
        }
        "invalid_access_token" => {
            println!("🚫  Access token invalid");
            "error".to_string()
        }
        result => panic!("❌  unhandled auth error {result}"),
    }
}

pub async fn check_account(http: &HTTPClient, sqlite: &DBConnection) {
    print!("\n🪪  Checking account...\n");
    let res = get_current_account(&http).await;
    let json = json::from_res(&res);
    db::insert_user(sqlite, &json);
    print!(
        "👤  Logged in as {}\n\n",
        &json.get("email").unwrap().as_str().unwrap()
    );
}
