use crate::db;
use crate::json;
use crate::localfs::{
    self, create_download_folder, create_local_file, delete_local_dir, get_local_size,
    local_file_exists, local_folder_exists, local_path_is_dir,
};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::header::HeaderMap;
use std::cmp::min;
use std::io::{Seek, Write};
use std::{env, error::Error};

pub async fn add_files_to_list(
    json: &serde_json::Value,
    db_connection: &sqlite::ConnectionWithFullMutex,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(json.get("error"), None, "🛑 DropBox returned an error");
    let count: usize = json::count_files(&json);
    println!("🗄️  {count} files found");
    if count > 0 {
        db::insert_dropbox_paths(&db_connection, json::get_entries(&json));
    }
    Ok(())
}

async fn list_folder(http: &reqwest::Client) -> String {
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
        "{{\"path\": \"{}\", \"recursive\": true,  \"limit\": 2000, \"include_non_downloadable_files\": false}}",
        base_folder
    );
    http.post("https://api.dropboxapi.com/2/files/list_folder")
        .headers(headers)
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

async fn list_folder_continue(http_client: &reqwest::Client, cursor: &String) -> String {
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
    let body = format!("{{\"cursor\": {cursor}}}");
    http_client
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
pub async fn get_paths(
    http_client: &reqwest::Client,
    db_connection: &sqlite::ConnectionWithFullMutex,
) {
    println!("");
    let count = db::count_rows(&db_connection);
    if count == 0 {
        println!("🗄️  File list empty");
        println!("🗄️  Populating file list...");
        let mut res = list_folder(&http_client).await;
        let mut json: serde_json::Value = json::from_res(&res);
        add_files_to_list(&json, &db_connection).await.unwrap();
        let mut has_more = json::get_has_more(&json);
        let mut cursor: String;
        while has_more == true {
            println!("🗄️  has_more is {}", has_more);
            cursor = json::get_cursor(&json);
            println!("🗄️  Getting next page of results...");
            res = list_folder_continue(&http_client, &cursor).await;
            json = json::from_res(&res);
            println!("🗄️  Adding results to database...");
            add_files_to_list(&json, &db_connection).await.unwrap();
            has_more = json::get_has_more(&json);
        }
    }
    db::report_status(&db_connection).try_into().unwrap()
}

pub async fn download_from_db(
    sqlite: &sqlite::ConnectionWithFullMutex,
    http: &reqwest::Client,
    dropbox_path: &str,
    local_path: &str,
) -> Result<(), Box<dyn Error>> {
    let local_size = get_local_size(&local_path);
    let dropbox_size = db::get_dropbox_size(&sqlite, &dropbox_path);
    let access_token = env::var("ACCESS_TOKEN")?;
    let team_member_id = env::var("TEAM_MEMBER_ID")?;
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {access_token}").parse().unwrap(),
    );
    headers.insert("Dropbox-API-Select-Admin", team_member_id.parse().unwrap());
    headers.insert(
        "Dropbox-API-Arg",
        format!("{{\"path\":\"{dropbox_path}\"}}").parse().unwrap(),
    );
    let res = http
        .post("https://content.dropboxapi.com/2/files/download")
        .headers(headers)
        .send()
        .await?;
    let pb = ProgressBar::new(dropbox_size as u64);
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
    match local_file_exists(&local_path) {
        false => {
            if !local_folder_exists(&local_path) {
                create_download_folder(&dropbox_path, &local_path);
            }
            file = create_local_file(&local_path);
        }
        true => {
            pb.set_message("🟢  Local file exists");
            match local_path_is_dir(&local_path) {
                true => {
                    delete_local_dir(&local_path);
                    pb.set_message("🚫  but is a directory. Erasing.");
                    pb.set_message("⌫  Key exists as directory. Erasing.");
                    std::fs::remove_dir(local_path).unwrap();
                    pb.set_message("⬇️  Fresh file.");
                    file = create_local_file(&local_path);
                }
                false => {
                    pb.set_message("🟢  and is not a directory.");
                    match local_size == dropbox_size {
                        true => {
                            pb.set_message("🟢  and matches DropBox size.");
                            let msg = format!("⬇️  Finished downloading {dropbox_path}");
                            pb.finish_with_message(msg);
                            return Ok(());
                        }
                        false => {
                            pb.set_message("⬇️  File exists. Resuming.");
                            file = std::fs::OpenOptions::new()
                                .read(true)
                                .append(true)
                                .open(local_path)
                                .unwrap();
                            downloaded = localfs::get_local_size(&local_path) as u64;
                            file.seek(std::io::SeekFrom::Start(downloaded)).unwrap();
                        }
                    }
                }
            }
        }
    }
    pb.set_message(format!("⬇️ Downloading {dropbox_path}"));
    while let Some(item) = stream.next().await {
        let chunk = item.or(Err(format!("❌  Error while downloading file")))?;
        file.write(&chunk)
            .or(Err(format!("❌  Error while writing to file")))?;
        let new = min(downloaded + (chunk.len() as u64), dropbox_size as u64);
        downloaded = new;
        pb.set_position(new);
    }
    let finished_msg = format!("⬇️  Finished downloading {dropbox_path}");
    pb.finish_with_message(finished_msg);
    assert_eq!(
        downloaded, dropbox_size as u64,
        "❌  Downloaded size does not match expected size"
    );
    Ok(())
}
