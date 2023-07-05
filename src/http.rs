use std::env;

use crate::util::getenv;

pub type HTTPClient = reqwest::Client;
pub type HeaderMap = reqwest::header::HeaderMap;

const APP_KEY: &str = "5mmsu1p6otobzgk";

pub fn new_client() -> HTTPClient {
    HTTPClient::builder().build().unwrap()
}

pub fn dropbox_authorization_header(headers: &mut HeaderMap) -> HeaderMap {
    let access_token = ::std::env::var("DROPBOX_ACCESS_TOKEN").unwrap();
    headers.insert(
        "Authorization",
        format!("Bearer {}", access_token).parse().unwrap(),
    );
    headers.to_owned()
}

pub fn dropbox_api_path_root_header(headers: &mut HeaderMap) -> HeaderMap {
    // val: format!("{{\".tag\": \"root\"}}").parse().unwrap()
    let root_namespace_id = getenv("DROPBOX_ROOT_NAMESPACE_ID");
    let home_namespace_id = getenv("DROPBOX_HOME_NAMESPACE_ID");
    headers.insert(
        "Dropbox-API-Path-Root",
        format!(
            "{{\".tag\": \"namespace_id\", \"namespace_id\": \"{}\"}}",
            home_namespace_id
        )
        .parse()
        .unwrap(),
    );
    headers.to_owned()
}

pub fn dropbox_select_user_header(headers: &mut HeaderMap) -> HeaderMap {
    let team_member_id = ::std::env::var("DROPBOX_TEAM_MEMBER_ID").unwrap();
    headers.insert(
        "Dropbox-API-Select-User",
        format!("{}", team_member_id).parse().unwrap(),
    );
    headers.to_owned()
}

pub fn dropbox_select_admin_header(headers: &mut HeaderMap) -> HeaderMap {
    let team_member_id = ::std::env::var("DROPBOX_TEAM_MEMBER_ID").unwrap();
    headers.insert(
        "Dropbox-API-Select-Admin",
        format!("{}", team_member_id).parse().unwrap(),
    );
    headers.to_owned()
}

pub fn dropbox_content_type_json_header(headers: &mut HeaderMap) -> HeaderMap {
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.to_owned()
}

pub fn dropbox_content_type_x_www_form_urlencoded_header(headers: &mut HeaderMap) -> HeaderMap {
    headers.insert(
        "Content-Type",
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    headers.to_owned()
}

pub fn dropbox_refresh_token_body() -> String {
    let refresh_token = env::var("DROPBOX_REFRESH_TOKEN").unwrap();
    let app_secret = env::var("APP_SECRET").unwrap();
    format!(
        "refresh_token={}&grant_type=refresh_token&client_id={}&client_secret={}",
        refresh_token, APP_KEY, app_secret
    )
}

pub async fn dropbox_oauth2_token_body() -> String {
    let authorization_code = env::var("DROPBOX_AUTHORIZATION_CODE").unwrap();
    let app_secret = crate::aws::get_app_secret().await;
    format!(
        "code={}&grant_type=authorization_code&client_id={}&client_secret={}",
        authorization_code, APP_KEY, app_secret
    )
}

pub fn dropbox_authorization_code_url() -> String {
    format!(
        "https://www.dropbox.com/oauth2/authorize?client_id={APP_KEY}&token_access_type=offline&response_type=code" 
    )
}
