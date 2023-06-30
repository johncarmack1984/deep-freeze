use futures_util::StreamExt;
use inquire::Confirm;
use reqwest::header::HeaderMap;
use std::cmp::min;
use std::fs::File;
use std::io::{Seek, Write};
use std::{env, error::Error};

use crate::db;
use crate::json;
use indicatif::{ProgressBar, ProgressStyle};

#[async_recursion::async_recursion(?Send)]
pub async fn add_files_to_list(
    res: String,
    connection: &sqlite::ConnectionWithFullMutex,
) -> Result<(), Box<dyn std::error::Error>> {
    let json: serde_json::Value = json::from_res(&res);
    assert_eq!(json.get("error"), None, "🛑 DropBox returned an error");
    let count: usize = json::count_files(&json);
    println!("🗄️  {count} files found");
    if count > 0 {
        db::insert_dropbox_paths(&connection, json::get_entries(&json));
    }
    let has_more = json::get_has_more(&json);
    println!("🗄️  has_more is {}", has_more);
    match has_more {
        true => {
            let cursor = json::get_cursor(&json);
            drop(res);
            drop(json);
            let res = list_folder_continue(&cursor).await;
            println!("🗄️  Adding results to database...");
            add_files_to_list(res, &connection).await
        }
        false => {
            println!("✅  File list populated");
            Ok(())
        }
    }
}

async fn list_folder() -> String {
    let access_token = env::var("ACCESS_TOKEN").unwrap();
    let team_member_id = env::var("TEAM_MEMBER_ID").unwrap();
    let base_folder = env::var("BASE_FOLDER").unwrap();
    let mut headers = HeaderMap::new();
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
    client
        .post("https://api.dropboxapi.com/2/files/list_folder")
        .headers(headers)
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

async fn list_folder_continue(cursor: &String) -> String {
    let access_token = env::var("ACCESS_TOKEN").unwrap();
    let team_member_id = env::var("TEAM_MEMBER_ID").unwrap();
    let mut headers = HeaderMap::new();
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
    client
        .post("https://api.dropboxapi.com/2/files/list_folder/continue")
        .headers(headers)
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

#[async_recursion::async_recursion(?Send)]
pub async fn get_paths(connection: &sqlite::ConnectionWithFullMutex) {
    let count = db::count_rows(&connection);
    if count == 0 {
        println!("🗄️  File list empty");
        println!("🗄️  Populating file list...");
        let res = list_folder().await;
        match add_files_to_list(res, &connection).await {
            Ok(_) => return get_paths(&connection).await,
            Err(err) => panic!("{err}"),
        }
    }
    println!("🗃️  {} files in database", count);
    let migrated = db::count_migrated(&connection);
    match migrated {
        _ => println!("🎉 {} already migrated", migrated),
    }
    let diff = count - migrated;
    println!("🗃️  {diff} files left to migrate");
    let percent = if count > 0 {
        (100 * migrated / count).abs()
    } else {
        0
    };
    match percent {
        0 => println!("🗄️  No files migrated"),
        100 => println!("🎉 All files migrated"),
        _ => println!("🎉 {percent}% done!"),
    }
}

pub async fn download_from_db(dropbox_path: &str, local_path: &str) -> Result<(), Box<dyn Error>> {
    match Confirm::new(&format!(
        "Download DropBox/{} to {}?",
        dropbox_path, local_path
    ))
    .with_default(true)
    .prompt()
    {
        Ok(true) => println!("🚀  Starting download"),
        Ok(false) => {
            println!("🚫  Download cancelled");
            std::process::exit(0)
        }
        Err(err) => {
            println!("🚫  {err}");
            std::process::exit(0)
        }
    }
    // // Reqwest setup
    let access_token = env::var("ACCESS_TOKEN")?;
    let team_member_id = env::var("TEAM_MEMBER_ID")?;
    let mut headers = HeaderMap::new();
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

    // Indicatif setup
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green}  [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    let msg = format!("⬇️  Checking for resumable download {local_path}");
    pb.set_message(msg);
    pb.set_position(0);

    let mut file;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    if std::path::Path::new(local_path).exists()
        && std::fs::metadata(local_path).unwrap().is_dir() == false
    {
        pb.set_message("⬇️  File exists. Resuming.");
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
        pb.set_message("⌫  Key exists as directory. Erasing.");
        std::fs::remove_dir(local_path).unwrap();
        pb.set_message("⬇️  Fresh file.");
        file = File::create(local_path)
            .or(Err(format!("❌  Failed to create file '{}'", local_path)))?;
    } else {
        pb.set_message("⬇️  Fresh file.");
        file = File::create(local_path)
            .or(Err(format!("❌  Failed to create file '{}'", local_path)))?;
    }
    pb.set_message(format!("⬇️ Downloading {dropbox_path}"));

    while let Some(item) = stream.next().await {
        let chunk = item.or(Err(format!("❌  Error while downloading file")))?;
        file.write(&chunk)
            .or(Err(format!("❌  Error while writing to file")))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }
    let finished_msg = format!("⬇️  Finished downloading {dropbox_path}");
    pb.finish_with_message(finished_msg);
    Ok(())
}
