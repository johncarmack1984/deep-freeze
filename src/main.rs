extern crate reqwest;
use dotenv::dotenv;
use open;
use reqwest::header;
use sedregex::find_and_replace;
use serde_json;
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

fn setenv(key: &str, value: String) {
    let envpath = Path::new(".env");
    let mut src = File::open(envpath).unwrap();
    let mut data = String::new();
    src.read_to_string(&mut data).unwrap();
    drop(src);
    let regex = format!("s/{}=\".*/{}=\"{}\"/g", key, key, value);
    let newenv = find_and_replace(&data, &[regex]).unwrap();
    let mut dst = File::create(envpath).unwrap();
    dst.write_all(newenv.as_bytes()).unwrap();
    env::set_var(key, value.clone());
    println!("🔑 {} set", key);
}

fn login() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛑 No account found");
    println!("🔒 Initiating login...");
    let app_key = env::var("APP_KEY").unwrap();
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
    setenv("AUTHORIZATION_CODE", authorization_code.clone());
    let app_secret = env::var("APP_SECRET").unwrap();

    println!("🔐 Requesting access token...");
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "Content-Type",
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    let body = format!(
        "code={}&grant_type=authorization_code&client_id={}&client_secret={}",
        authorization_code, app_key, app_secret
    );
    let client = reqwest::blocking::Client::new();
    let res = client
        .post("https://api.dropbox.com/oauth2/token")
        .headers(headers)
        .body(body)
        .send()?
        .text()?;
    let json = serde_json::from_str::<serde_json::Value>(&res).unwrap();
    println!("{:#?}", json);
    let refresh_token = json.get("refresh_token").unwrap().to_string().to_owned();
    let access_token = json.get("access_token").unwrap().to_string().to_owned();
    setenv("REFRESH_TOKEN", refresh_token);
    setenv("ACCESS_TOKEN", access_token);
    Ok(())
}

fn refresh_token() -> Result<(), Box<dyn std::error::Error>> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "Content-Type",
        "application/x-www-form-urlencoded".parse().unwrap(),
    );

    let client = reqwest::blocking::Client::new();
    let body = format!(
        "refresh_token={}&grant_type=refresh_token&client_id={}&client_secret={}",
        env::var("REFRESH_TOKEN").unwrap(),
        env::var("APP_KEY").unwrap(),
        env::var("APP_SECRET").unwrap()
    );
    let res = client
        .post("https://api.dropbox.com/oauth2/token")
        .headers(headers)
        .body(body)
        .send()?
        .text()?;
    let json = serde_json::from_str::<serde_json::Value>(&res).unwrap();
    println!("{:?}", json);
    let access_token = json.get("access_token").unwrap().to_string().to_owned();
    setenv("ACCESS_TOKEN", access_token);
    Ok(())
}

fn check_account() -> Result<(), Box<dyn std::error::Error>> {
    println!("🪪 Checking account...");
    let mut headers = header::HeaderMap::new();
    let access_token = env::var("ACCESS_TOKEN").unwrap();
    let team_member_id = env::var("TEAM_MEMBER_ID").unwrap();
    headers.insert(
        "Authorization",
        format!("Bearer {}", access_token).parse().unwrap(),
    );
    headers.insert(
        "Dropbox-API-Select-Admin",
        format!("{}", team_member_id).parse().unwrap(),
    );

    let client = reqwest::blocking::Client::new();
    let res = client
        .post("https://api.dropboxapi.com/2/users/get_current_account")
        .headers(headers)
        .send()?
        .text()?;
    let json = serde_json::from_str::<serde_json::Value>(&res).unwrap();
    assert_eq!(json.get("error"), None);
    println!("👤 Logged in as {}", json.get("email").unwrap());
    if false {
        let _rt = refresh_token();
        let _l = login();
    }
    Ok(())
}

fn add_files_to_list() {
    println!("add_files_to_list");
}

fn get_paths() {
    println!("get_paths");
    add_files_to_list();
}
fn remove_from_list() {
    println!("remove-from-list");
}
fn migrate_to_s3() {
    println!("migrate_to_s3");
    remove_from_list();
}
fn perform_migration() {
    println!("perform_migration");
    migrate_to_s3();
}

fn main() {
    dotenv().ok();
    let _ca = check_account();
    get_paths();
    perform_migration();
}
