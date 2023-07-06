use crate::db::{self, DBConnection};
use crate::dropbox;
use crate::http::{self, HTTPClient, HeaderMap};
use crate::json;
use crate::util::{prompt, setenv};

use inquire::{InquireError, Select};
use open;

async fn login(http: &HTTPClient) {
    println!("🔒 Initiating login...");
    get_authorization_code().await;
    get_access_token(http).await.unwrap();
}

async fn get_access_token(http: &HTTPClient) -> Result<(), String> {
    println!("🔐 Requesting access token...");
    let mut headers = HeaderMap::new();
    headers = http::dropbox_content_type_x_www_form_urlencoded_header(&mut headers);
    let body = http::dropbox_oauth2_token_body().await;
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
            true => Err(handle_auth_error(&http, res).await),
            false => Ok(handle_successful_login(res).await),
        },
        Err(err) => panic!("❌ {err}"),
    }
}

async fn handle_successful_login(res: String) {
    let json = json::from_res(&res);
    let team_id = json.get("team_id").unwrap().as_str().unwrap();
    let refresh_token = json.get("refresh_token").unwrap().as_str().unwrap();
    let access_token = json.get("access_token").unwrap().as_str().unwrap();
    setenv("DROPBOX_TEAM_ID", team_id.to_string());
    println!("🔑 Team ID set");
    setenv("DROPBOX_REFRESH_TOKEN", refresh_token.to_string());
    println!("🔑 Refresh token set");
    setenv("DROPBOX_ACCESS_TOKEN", access_token.to_string());
    println!("🔑 Login: Access token set");
}

async fn get_authorization_code() {
    let url = http::dropbox_authorization_code_url();
    print!("\n🚦 You need to be logged in to DropBox\n\n");
    open::that_detached(&url).unwrap();
    println!("🌐 Open this URL in your browser (one might have opened already):");
    print!("\n🌐 {}\n\n", url);
    println!("🔐 and authorize the app.");
    let authorization_code = prompt("🪪  Paste the authorization code you see here");
    setenv("DROPBOX_AUTHORIZATION_CODE", authorization_code);
    println!("🔑 Authorization code set");
}

async fn refresh_token(http: &HTTPClient) -> String {
    println!("🔑 Refreshing access token...");
    let mut headers = HeaderMap::new();
    headers = http::dropbox_content_type_x_www_form_urlencoded_header(&mut headers);
    let body = http::dropbox_refresh_token_body().await;
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
            true => {
                dbg!(&res);
                handle_auth_error(&http, res).await
            }
            false => {
                let json = json::from_res(&res);
                let access_token = json.get("access_token").unwrap().to_string().to_owned();
                setenv("DROPBOX_ACCESS_TOKEN", access_token);
                get_current_account(&http).await
            }
        },
        Err(err) => panic!("❌ {err}"),
    }
}

async fn get_current_account(http: &HTTPClient) -> String {
    let mut headers = http::HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    // headers = http::dropbox_select_admin_header(&mut headers);
    headers = http::dropbox_select_user_header(&mut headers);
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

async fn select_team_member(http: &HTTPClient, sqlite: &DBConnection) {
    let res = dropbox::get_team_members_list(&http).await;
    // println!("println res {:?}", res);
    let json = json::from_res(&res);
    let members = json.get("members").unwrap().as_array().unwrap();
    let options: Vec<String> = members
        .into_iter()
        .map(|member| {
            let email = member
                .get("profile")
                .unwrap()
                .get("email")
                .unwrap()
                .as_str()
                .unwrap();
            format!("{}", email)
        })
        .collect();
    let ans: Result<String, InquireError> =
        Select::new("Which team member are you?", options).prompt();
    match ans {
        Ok(choice) => {
            let member = members
                .into_iter()
                .find(|member| {
                    member
                        .get("profile")
                        .unwrap()
                        .get("email")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        == choice
                })
                .unwrap()
                .get("profile")
                .unwrap();
            db::insert_user(sqlite, member);
        }
        Err(_) => {
            println!("🚫  Error selecting team member");
            std::process::exit(1);
        }
    }
}

pub async fn check_account(http: &HTTPClient, sqlite: &DBConnection) {
    if dotenv::var("DROPBOX_REFRESH_TOKEN").is_err() {
        login(http).await;
    }
    if dotenv::var("DROPBOX_TEAM_MEMBER_ID").is_err() {
        select_team_member(http, sqlite).await;
    }
    print!("\n🪪  Checking account...\n");
    let res = get_current_account(&http).await;
    let json = json::from_res(&res);
    db::insert_user(sqlite, &json);
    print!(
        "👤  Logged in as {}\n\n",
        &json.get("email").unwrap().as_str().unwrap()
    );
}
