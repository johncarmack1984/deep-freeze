extern crate reqwest;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::operation::get_object_attributes::GetObjectAttributesOutput;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, ObjectAttributes, StorageClass};
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
use aws_smithy_http::byte_stream::Length;
use core::panic;
use dotenv::dotenv;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use open;
use reqwest::header;
use sedregex::find_and_replace;
use serde_json;
use sqlite;
use std::cmp::min;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::path::Path;
use std::sync::Arc;
use std::{env, fs};
use tokio::sync::Semaphore;

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
        let connection: sqlite::ConnectionWithFullMutex =
            sqlite::Connection::open_with_full_mutex("db.sqlite").unwrap();
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
    let connection = sqlite::Connection::open_with_full_mutex("db.sqlite").unwrap();
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
    client: &AWSClient,
    bucket: &str,
) -> Result<GetObjectAttributesOutput, AWSError> {
    let res = client
        .get_object_attributes()
        .bucket(bucket)
        .key(base_path)
        .object_attributes(ObjectAttributes::ObjectSize)
        .send()
        .await?;

    Ok::<GetObjectAttributesOutput, AWSError>(res)
}

async fn download_from_db(dropbox_path: &str, local_path: &str) -> Result<(), Box<dyn Error>> {
    // // Reqwest setup
    let access_token = env::var("ACCESS_TOKEN")?;
    let team_member_id = env::var("TEAM_MEMBER_ID")?;
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {}", access_token).parse().unwrap(),
    );
    headers.insert("Dropbox-API-Select-Admin", team_member_id.parse().unwrap());
    headers.insert(
        "Dropbox-API-Arg",
        format!("{{\"path\":\"{}\"}}", dropbox_path)
            .parse()
            .unwrap(),
    );
    let client = reqwest::Client::new();
    let res = client
        .post("https://content.dropboxapi.com/2/files/download")
        .headers(headers)
        .send()
        .await?;

    let total_size = res.content_length().ok_or(format!(
        "Failed to get content length from '{}'",
        &dropbox_path
    ))?;

    // // Indicatif setup
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green}  [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    let msg = format!("📁 Downloading {}", dropbox_path);
    pb.set_message(msg);

    let mut file;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    println!("🔍  Seeking in file.");
    if std::path::Path::new(local_path).exists()
        && std::fs::metadata(local_path).unwrap().is_dir() == false
    {
        println!("🕵️‍♂️  File exists. Resuming.");
        file = std::fs::OpenOptions::new()
            .read(true)
            .append(true)
            .open(local_path)
            .unwrap();

        let file_size = std::fs::metadata(local_path).unwrap().len();
        file.seek(std::io::SeekFrom::Start(file_size)).unwrap();
        downloaded = file_size;
    } else if std::path::Path::new(local_path).exists()
        && std::fs::metadata(local_path).unwrap().is_dir() == true
    {
        println!("🕵️‍♂️  Key exists as directory. Erasing.");
        std::fs::remove_dir(local_path).unwrap();
        println!("🕵️‍♂️  Fresh file..");
        file = File::create(local_path)
            .or(Err(format!("❌  Failed to create file '{}'", local_path)))?;
    } else {
        println!("🕵️‍♂️  Fresh file..");
        file = File::create(local_path)
            .or(Err(format!("❌  Failed to create file '{}'", local_path)))?;
    }

    println!("Commencing transfer");
    while let Some(item) = stream.next().await {
        let chunk = item.or(Err(format!("Error while downloading file")))?;
        file.write(&chunk)
            .or(Err(format!("Error while writing to file")))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }
    let finished_msg = format!("✅  Finished downloading {}", dropbox_path);
    pb.finish_with_message(finished_msg);
    Ok(())
}

const CHUNK_SIZE: u64 = 1024 * 1024 * 5;
const MAX_CHUNKS: u64 = 10000;

async fn upload_to_s3(
    aws_client: &AWSClient,
    s3_path: &str,
    local_path: &str,
    s3_bucket: &str,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    println!("📂  Uploading to S3 {}", s3_path);
    println!("📂  Uploading from {}", local_path);
    let res = aws_client
        .create_multipart_upload()
        .bucket(s3_bucket)
        .key(s3_path)
        .storage_class(StorageClass::DeepArchive)
        .send()
        .await
        .unwrap();
    let upload_id = res.upload_id().unwrap();

    let path = Path::new(local_path);
    let file_size = tokio::fs::metadata(path)
        .await
        .expect("it exists I swear")
        .len();

    let mut chunk_count = (file_size / CHUNK_SIZE) + 1;
    let mut size_of_last_chunk = file_size % CHUNK_SIZE;
    if size_of_last_chunk == 0 {
        size_of_last_chunk = CHUNK_SIZE;
        chunk_count -= 1;
    }

    if file_size == 0 {
        panic!("Bad file size.");
    }
    if chunk_count > MAX_CHUNKS {
        panic!("Too many chunks! Try increasing your chunk size.")
    }

    let mut upload_parts: Vec<CompletedPart> = Vec::new();

    println!("⬆️  Uploading {} chunks.", chunk_count);

    let pb = ProgressBar::new(file_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    let msg = format!("⬆️  Uploading {} to {}", s3_path, s3_bucket);
    pb.set_message(msg);

    for chunk_index in 0..chunk_count {
        let this_chunk = if chunk_count - 1 == chunk_index {
            size_of_last_chunk
        } else {
            CHUNK_SIZE
        };
        let uploaded = chunk_index * CHUNK_SIZE;
        pb.set_message(format!(
            "⬆️  Uploading chunk {} of {}.",
            chunk_index + 1,
            chunk_count
        ));
        let stream = ByteStream::read_from()
            .path(Path::new(local_path))
            .offset(uploaded)
            .length(Length::Exact(this_chunk))
            .build()
            .await
            .unwrap();
        //Chunk index needs to start at 0, but part numbers start at 1.
        let part_number = (chunk_index as i32) + 1;
        // snippet-start:[rust.example_code.s3.upload_part]
        let upload_part_res = aws_client
            .upload_part()
            .key(s3_path)
            .bucket(s3_bucket)
            .upload_id(upload_id)
            .body(stream)
            // .body(stream.to_multipart_s3_stream())
            .part_number(part_number)
            .send()
            .await?;
        upload_parts.push(
            CompletedPart::builder()
                .e_tag(upload_part_res.e_tag.unwrap_or_default())
                .part_number(part_number)
                .build(),
        );
        pb.set_position(uploaded + this_chunk);
        // snippet-end:[rust.example_code.s3.upload_part]
    }
    pb.finish_with_message("✅  All chunks uploaded.");
    // snippet-start:[rust.example_code.s3.upload_part.CompletedMultipartUpload]
    let completed_multipart_upload: CompletedMultipartUpload = CompletedMultipartUpload::builder()
        .set_parts(Some(upload_parts))
        .build();
    // snippet-end:[rust.example_code.s3.upload_part.CompletedMultipartUpload]
    println!("⏳  Completing upload.");
    // snippet-start:[rust.example_code.s3.complete_multipart_upload]
    let _complete_multipart_upload_res = aws_client
        .complete_multipart_upload()
        .bucket(s3_bucket)
        .key(s3_path)
        .multipart_upload(completed_multipart_upload)
        .upload_id(upload_id)
        .send()
        .await
        .unwrap();
    // // snippet-end:[rust.example_code.s3.complete_multipart_upload]
    println!("✅ Done uploading file.");

    Ok(())
}

async fn migrate_to_s3(
    aws_client: &AWSClient,
    migrated: &mut i64,
    dropbox_path: &str,
    size: &i64,
) -> Result<(), std::io::Error> {
    if migrated.is_positive() {
        println!("✅ File already migrated");
        return Ok(());
    }

    let base_name = Path::new(&dropbox_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let base_folder = env::var("BASE_FOLDER").unwrap();
    let mut base_path = find_and_replace(
        &dropbox_path.clone().to_owned(),
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
        base_path = find_and_replace(&base_path, &["s/_/_/g"])
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
    if migrated.is_negative() {
        println!("📂  Checking migration status for {}", dropbox_path);
        let db_size = size.clone();
        match get_s3_attrs(&base_path, &aws_client, &s3_bucket).await {
            Ok(s3_attrs) => {
                if s3_attrs.object_size() == db_size {
                    println!("✅ File already migrated");
                    let connection = sqlite::Connection::open_with_full_mutex("db.sqlite").unwrap();
                    let statement = format!(
                        "UPDATE paths SET migrated = 1 WHERE path = '{}';",
                        dropbox_path.clone()
                    );
                    match connection.execute(statement.clone()) {
                        Ok(_) => {
                            println!("✅ File list updated");
                            *migrated = 1;
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
                    let connection = sqlite::Connection::open_with_full_mutex("db.sqlite").unwrap();
                    let statement = format!(
                        "UPDATE paths SET migrated = 0 WHERE path = '{}';",
                        dropbox_path.clone()
                    );
                    match connection.execute(statement.clone()) {
                        Ok(_) => {
                            println!("✅ File list updated");
                            *migrated = 0;
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
                AWSError::NoSuchKey(_) => {
                    println!("❌  File not found in S3");
                    let connection = sqlite::Connection::open_with_full_mutex("db.sqlite").unwrap();
                    let statement = format!(
                        "UPDATE paths SET migrated = 0 WHERE path = '{}';",
                        dropbox_path.clone()
                    );
                    match connection.execute(statement.clone()) {
                        Ok(_) => {
                            println!("✅ File list updated");
                            *migrated = 0;
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
    match migrated.abs() == 0 {
        true => {
            let local_path = format!("./temp/{}", base_path);
            let local_dir = find_and_replace(&local_path, &[format!("s/{}//g", base_name)])
                .unwrap()
                .to_string();
            if !std::path::Path::new(&local_dir).exists() {
                let _dir = fs::create_dir_all(&local_dir)?;
            }
            // println!("📂  Migrating {}", path);
            let _file = download_from_db(&dropbox_path, &local_path).await.unwrap();
            // verify file size (refactor from below)
            // TODO verify checksum from DB
            // TODO create checksum from file for AWS
            // TODO upload to S3
            match upload_to_s3(&aws_client, &base_path, &local_path, &s3_bucket)
                .await
                .unwrap()
            {
                () => {
                    println!("✅ File uploaded to S3");
                    std::fs::remove_file(&local_path).unwrap();
                    let connection = sqlite::Connection::open_with_full_mutex("db.sqlite").unwrap();
                    let statement = format!(
                        "UPDATE paths SET migrated = 1 WHERE path = '{}';",
                        dropbox_path.clone()
                    );
                    match connection.execute(statement.clone()) {
                        Ok(_) => {
                            println!("✅ File list updated");
                            *migrated = 1;
                        }
                        Err(err) => {
                            println!("❌  Error in statement: {}", statement);
                            panic!("{}", err);
                        }
                    }
                } // TODO verify checksum from S3
                  // update migration status
                  // update file list
            }
            Ok(())
        }
        false => Ok(()),
    }
}

async fn perform_migration() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    println!("🗃️ Performing migration...");
    let region_provider = RegionProviderChain::first_try(Region::new("us-east-1"))
        .or_default_provider()
        .or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    let aws_client = AWSClient::new(&config);
    let query = "SELECT * FROM paths WHERE migrated < 1";
    let sqlite_connection = sqlite::Connection::open_with_full_mutex("db.sqlite").unwrap();
    let rows = sqlite_connection
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>();
    let semaphore = Arc::new(Semaphore::new(1)); // Limit to 10 concurrent downloads
    let mut tasks = Vec::new();
    for row in rows {
        let mut migrated = row.try_read::<i64, &str>("migrated").unwrap();
        let dropbox_path = row.try_read::<&str, &str>("path").unwrap().to_string();
        let size = row.try_read::<i64, &str>("size").unwrap();
        let aws_client = aws_client.clone();
        let sem_clone = Arc::clone(&semaphore);
        let task = tokio::spawn(async move {
            let permit = sem_clone.acquire().await.unwrap();
            // sqlite_connection.clone();
            match migrate_to_s3(&aws_client, &mut migrated, &dropbox_path, &size).await {
                Ok(_) => {}
                Err(err) => {
                    println!("{}", err);
                }
            };
            drop(permit); // Release the semaphore
        });
        tasks.push(task);
    }
    for task in tasks {
        task.await.unwrap();
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
    Ok(())
}
