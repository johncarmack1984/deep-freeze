extern crate reqwest;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::operation::get_object_attributes::GetObjectAttributesOutput;
use aws_sdk_s3::types::ObjectAttributes;
use aws_sdk_s3::{Client, Error};
use core::panic;
use dotenv::dotenv;
use open;
use reqwest::header;
use sedregex::find_and_replace;
use serde_json;
use sqlite::{self, Row};
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

fn setenv(key: &str, value: String) -> Result<(), Box<dyn std::error::Error>> {
    let envpath = Path::new(".env");
    let mut src = File::open(envpath).unwrap();
    let mut data = String::new();
    src.read_to_string(&mut data).unwrap();
    drop(src);
    let regex = format!("s/{}=.*/{}={}/g", key, key, value);
    let newenv = find_and_replace(&data, &[regex]).unwrap();
    let mut dst = File::create(envpath).unwrap();
    dst.write_all(newenv.as_bytes()).unwrap();
    env::set_var(key, value.clone());
    assert_eq!(env::var(key).unwrap(), value);
    Ok(())
}

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
    let mut headers = header::HeaderMap::new();
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
        Err(err) => println!("{}", err),
    }
    match setenv("REFRESH_TOKEN", refresh_token) {
        Ok(_) => println!("🔑 Refresh token set"),
        Err(err) => println!("{}", err),
    }
    match setenv("ACCESS_TOKEN", access_token) {
        Ok(_) => println!("🔑 Access token set"),
        Err(err) => println!("{}", err),
    }
    Ok(())
}

async fn refresh_token() -> Result<(), Box<dyn std::error::Error>> {
    let refresh_token = env::var("REFRESH_TOKEN").unwrap();
    let app_key = env::var("APP_KEY").unwrap();
    let app_secret = env::var("APP_SECRET").unwrap();
    let mut headers = header::HeaderMap::new();
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
    assert_eq!(json.get("error"), None, "🛑 Not logged in");
    let access_token = json.get("access_token").unwrap().to_string().to_owned();
    match setenv("ACCESS_TOKEN", access_token) {
        Ok(_) => println!("🔑 Access token set"),
        Err(err) => println!("{}", err),
    }
    Ok(())
}

#[async_recursion::async_recursion(?Send)]
async fn check_account() {
    dotenv().ok();
    println!("🪪  Checking account...");
    let access_token = env::var("ACCESS_TOKEN").unwrap();
    let team_member_id = env::var("TEAM_MEMBER_ID").unwrap();
    let mut headers = header::HeaderMap::new();
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
                Ok(_) => {
                    println!("🔑 Refreshed access token");
                    check_account().await;
                }
                Err(err) => {
                    println!("{}", err);
                }
            }
        }
        Some("invalid_access_token/") => {
            println!("🔑 Access token invalid");
            match login().await {
                Ok(_) => {
                    println!("🔑 Logged in");
                    check_account().await;
                }
                Err(err) => {
                    println!("{}", err);
                }
            }
        }
        Some(err) => {
            println!("{}", err);
        }
        None => {
            println!("👤 Logged in as {}", json.get("email").unwrap());
        }
    }
    assert_eq!(json.get("error"), None, "🛑 Not logged in");
}

#[async_recursion::async_recursion(?Send)]
async fn add_files_to_list(res: String) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::from_str::<serde_json::Value>(&res).unwrap();
    assert_eq!(
        json.get("error"),
        None,
        "🛑 DropBox returned an error when listing folder contents"
    );
    let count = json
        .get("entries")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row.get(".tag").unwrap().as_str().unwrap() == "file")
        .count();
    println!("🗄️  {} files found", count);
    if count > 0 {
        let entries = json.get("entries").unwrap().as_array().unwrap();
        let mut statement = entries
            .iter()
            .filter(|row| row.get(".tag").unwrap().as_str().unwrap() == "file")
            .map(|row| {
                let mut path = row
                    .clone()
                    .get("path_display")
                    .unwrap()
                    .to_string()
                    .to_owned();
                path = find_and_replace(&path, &["s/\'//g"]).unwrap().to_string();
                let hash = row.get("content_hash").unwrap().to_string().to_owned();
                let size = row.get("size").unwrap().to_string().to_owned();
                return format!("('{}', {}, '{}', -1), ", path, size, hash);
            })
            .collect::<Vec<_>>()
            .join("");
        let connection = sqlite::open("db.sqlite").unwrap();
        statement = format!("INSERT OR IGNORE INTO paths VALUES {};", statement);
        statement = find_and_replace(&statement, &["s/, ;/;/g"])
            .unwrap()
            .to_string();
        statement = find_and_replace(&statement, &["s/\"//g"])
            .unwrap()
            .to_string()
            .to_owned();
        match connection.execute(statement.clone()) {
            Ok(_) => {
                println!("✅ File list updated");
            }
            Err(err) => {
                println!("❌  Error in statement: {}", statement);
                panic!("{}", err);
            }
        }
    }
    let access_token = env::var("ACCESS_TOKEN").unwrap();
    let team_member_id = env::var("TEAM_MEMBER_ID").unwrap();
    let has_more = json.get("has_more").unwrap().as_bool();
    println!("🗄️  has_more is {}", has_more.unwrap());
    Ok(match has_more {
        Some(true) => {
            let cursor = json.get("cursor").unwrap().to_string().to_owned();
            let mut headers = header::HeaderMap::new();
            headers.insert(
                "Authorization",
                format!("Bearer {}", access_token).parse().unwrap(),
            );
            headers.insert("Content-Type", "application/json".parse().unwrap());
            headers.insert(
                "Dropbox-API-Select-Admin",
                format!("{}", team_member_id).parse().unwrap(),
            );
            println!("🗄️  Getting next page of results...");
            let body = format!("{{\"cursor\": {}}}", cursor);
            let client = reqwest::Client::new();
            let res = client
                .post("https://api.dropboxapi.com/2/files/list_folder/continue")
                .headers(headers)
                .body(body)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            println!("🗄️  Adding results to database...");
            return add_files_to_list(res).await;
        }
        Some(false) | None => {
            println!("✅  File list populated");
        }
    })
}

#[async_recursion::async_recursion(?Send)]
async fn get_paths() {
    let connection = sqlite::open("db.sqlite").unwrap();
    connection
        .execute(
            "
        CREATE TABLE IF NOT EXISTS paths (
            path TEXT NOT NULL UNIQUE,
            size INTEGER NOT NULL,
            hash TEXT NOT NULL,
            migrated INTEGER NOT NULL DEFAULT -1
        );
        ",
        )
        .unwrap();
    let count_query = "SELECT COUNT(*) FROM paths";
    let count = connection
        .prepare(count_query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .map(|row| row.read::<i64, _>(0))
        .next()
        .unwrap();
    if count == 0 {
        println!("🗄️  File list empty");
        let access_token = env::var("ACCESS_TOKEN").unwrap();
        let team_member_id = env::var("TEAM_MEMBER_ID").unwrap();
        let base_folder = env::var("BASE_FOLDER").unwrap();
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", access_token).parse().unwrap(),
        );
        headers.insert(
            "Dropbox-API-Select-Admin",
            format!("{}", team_member_id).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());
        let body = format!(
            "{{\"path\": \"{}\", \"recursive\": true,  \"limit\": 2000}}",
            base_folder
        );
        let client = reqwest::Client::new();
        let res = client
            .post("https://api.dropboxapi.com/2/files/list_folder")
            .headers(headers)
            .body(body)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        match add_files_to_list(res).await {
            Ok(_) => {
                get_paths().await;
            }
            Err(err) => panic!("{}", err),
        }
    }
    println!("🗃️  {} files in database", count);
    let migrated_query = "SELECT COUNT(*) FROM paths WHERE migrated < 1";
    let migrated = connection
        .prepare(migrated_query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .map(|row| row.read::<i64, _>(0))
        .next()
        .unwrap();
    match migrated {
        0 => println!("🗄️  No files migrated (or none confirmed migrated)"),
        _ => println!("🎉 {} already migrated", migrated),
    }
    let diff = count - migrated;
    match diff {
        0 => println!("🗄️  All files migrated"),
        _ => println!("🗃️  {} files left to migrate", diff),
    }
    match diff > 0 {
        true => println!("🎉 {}% done!", 100 * migrated / count),
        false => println!("🗄️  No files migrated (or none confirmed migrated)"),
    }
}

async fn get_s3_attrs(
    base_path: &String,
    client: &Client,
    bucket: &str,
) -> Result<GetObjectAttributesOutput, Error> {
    let res = client
        .get_object_attributes()
        .bucket(bucket)
        .key(base_path)
        .object_attributes(ObjectAttributes::ObjectSize)
        .send()
        .await?;

    Ok::<GetObjectAttributesOutput, Error>(res)
}

async fn migrate_to_s3(client: &Client, row: Row) -> Result<(), Error> {
    let migrated = row.try_read::<i64, &str>("migrated").unwrap();
    if migrated == 1 {
        println!("✅ File already migrated");
    }
    let path = row.try_read::<&str, &str>("path").unwrap();

    let base_folder = env::var("BASE_FOLDER").unwrap();
    let mut base_path = find_and_replace(
        &path.clone().to_owned(),
        &[format!("s/\\{}\\///g", base_folder)],
    )
    .unwrap()
    .to_string();
    if base_path.contains("channel") {
        base_path = find_and_replace(&base_path, &["s/channel/Channel/g"])
            .unwrap()
            .to_string();
    }
    if base_path.contains("_") {
        base_path = find_and_replace(&base_path, &["s/_/\\_/g"])
            .unwrap()
            .to_string();
    }
    if base_path.contains("|") {
        base_path = find_and_replace(&base_path, &["s/\\|/\\|/g"])
            .unwrap()
            .to_string();
    }
    if base_path.contains("•") {
        base_path = find_and_replace(&base_path, &["s/•/\\•/g"])
            .unwrap()
            .to_string();
    }
    let s3_bucket = env::var("S3_BUCKET").unwrap();
    if migrated == 0 {
        // println!("📂  Migrating {}", path);
        // TODO download_from_db();
        // verify file size (refactor from below)
        // TODO verify checksum from DB
        // TODO create checksum from file for AWS
        // TODO upload to S3
        // TODO verify checksum from S3
        // update migration status
        // update file list
    }
    if migrated == -1 {
        println!("📂  Checking migration status for {}", path);
        let db_size = row.try_read::<i64, &str>("size").unwrap();
        match get_s3_attrs(&base_path, &client, &s3_bucket).await {
            Ok(s3_attrs) => {
                if s3_attrs.object_size() == db_size {
                    println!("✅ File already migrated");
                    let connection = sqlite::open("db.sqlite").unwrap();
                    let statement = format!(
                        "UPDATE paths SET migrated = 1 WHERE path = '{}';",
                        path.clone()
                    );
                    match connection.execute(statement.clone()) {
                        Ok(_) => {
                            println!("✅ File list updated");
                        }
                        Err(err) => {
                            println!("❌  Error in statement: {}", statement);
                            println!("❌  Database Could not be Updated: {}", statement);
                            panic!("{}", err);
                        }
                    }
                    return Ok(());
                } else {
                    println!("❌ File not the same size on S3 as DB");
                    let connection = sqlite::open("db.sqlite").unwrap();
                    let statement = format!(
                        "UPDATE paths SET migrated = 0 WHERE path = '{}';",
                        path.clone()
                    );
                    match connection.execute(statement.clone()) {
                        Ok(_) => {
                            println!("✅ File list updated");
                        }
                        Err(err) => {
                            println!("❌  Error in statement: {}", statement);
                            panic!("{}", err);
                        }
                    }
                    // TODO download_from_db();
                }
            }
            Err(err) => match err {
                Error::NoSuchKey(_) => {
                    println!("❌  File not found in S3");
                    let connection = sqlite::open("db.sqlite").unwrap();
                    let statement = format!(
                        "UPDATE paths SET migrated = 0 WHERE path = '{}';",
                        path.clone()
                    );
                    match connection.execute(statement.clone()) {
                        Ok(_) => {
                            println!("✅ File list updated");
                        }
                        Err(err) => {
                            println!("❌  Error in statement: {}", statement);
                            panic!("{}", err);
                        }
                    }
                    return Ok(());
                }
                _ => {
                    panic!("❌  Error in S3 request: {}", err);
                }
            },
        }
    }
    Ok(())
}
async fn perform_migration() -> Result<(), Box<dyn std::error::Error>> {
    println!("🗃️ Performing migration...");
    let region_provider = RegionProviderChain::first_try(Region::new("us-east-1"))
        .or_default_provider()
        .or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    let client = Client::new(&config);
    let query = "SELECT * FROM paths WHERE migrated < 1";
    let connection = sqlite::open("db.sqlite").unwrap();
    let rows = connection
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>();
    for row in rows {
        migrate_to_s3(&client, row).await?;
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    check_account().await;
    get_paths().await;
    perform_migration().await?;
    println!("✅✅✅  Migration complete");
    return Ok(());
}
