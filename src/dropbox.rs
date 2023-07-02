use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::cmp::min;
use std::io::{Seek, Write};
use std::{env, error::Error};

use crate::db::{self, DBConnection};
use crate::http::{self, HTTPClient, HeaderMap};
use crate::json::{self, JSON};
use crate::localfs::{
    self, create_download_folder, create_local_file, delete_local_dir, get_local_size,
    local_file_exists, local_folder_exists, local_path_is_dir,
};

pub async fn add_files_to_list(
    json: &JSON,
    connection: &DBConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(json.get("error"), None, "🛑 DropBox returned an error");
    let count: usize = json::count_files(&json);
    println!("🗄️  {count} files found");
    if count > 0 {
        db::insert_dropbox_paths(&connection, json::get_entries(&json));
    }
    Ok(())
}

async fn list_folder(http: &HTTPClient) -> String {
    let base_folder = env::var("BASE_FOLDER").unwrap();
    let mut headers = HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_select_admin_header(&mut headers);
    headers = http::dropbox_content_type_json_header(&mut headers);
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

async fn list_folder_continue(http: &HTTPClient, cursor: &String) -> String {
    let mut headers = HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_select_admin_header(&mut headers);
    headers = http::dropbox_content_type_json_header(&mut headers);
    let body = format!("{{\"cursor\": {cursor}}}");
    http.post("https://api.dropboxapi.com/2/files/list_folder/continue")
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
pub async fn get_paths(http: &HTTPClient, sqlite: &DBConnection) {
    print!("\n\n🗄️  Getting file list...\n");
    let count = db::count_rows(&sqlite);
    if count == 0 {
        println!("🗄️  File list empty");
        println!("🗄️  Populating file list...");
        let mut res = list_folder(&http).await;
        let mut json: JSON = json::from_res(&res);
        add_files_to_list(&json, &sqlite).await.unwrap();
        let mut has_more = json::get_has_more(&json);
        let mut cursor: String;
        while has_more == true {
            println!("🗄️  has_more is {}", has_more);
            cursor = json::get_cursor(&json);
            println!("🗄️  Getting next page of results...");
            res = list_folder_continue(&http, &cursor).await;
            json = json::from_res(&res);
            println!("🗄️  Adding results to database...");
            add_files_to_list(&json, &sqlite).await.unwrap();
            has_more = json::get_has_more(&json);
        }
    }
    db::report_status(&sqlite).try_into().unwrap()
}

pub async fn get_file_metadata(http: &HTTPClient, dropbox_path: &str) -> String {
    let mut headers = HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_select_admin_header(&mut headers);
    headers = http::dropbox_content_type_json_header(&mut headers);
    let body = format!("{{\"path\": \"{}\"}}", dropbox_path);
    http.post("https://api.dropboxapi.com/2/files/get_metadata")
        .headers(headers)
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

pub async fn get_dropbox_size(http: &HTTPClient, dropbox_path: &str) -> i64 {
    let res = get_file_metadata(&http, dropbox_path).await;
    let json = json::from_res(&res);
    json::get_size(&json)
}

pub async fn download_from_dropbox(
    http: &reqwest::Client,
    dropbox_path: &str,
    local_path: &str,
) -> Result<(), Box<dyn Error>> {
    let local_size = get_local_size(&local_path);
    let dropbox_size = get_dropbox_size(http, dropbox_path).await;
    let mut headers = HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_select_admin_header(&mut headers);
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
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn it_gets_file_metadata_from_dropbox() {
        dotenv::dotenv().ok();
        let http = crate::http::new_client();
        let dropbox_path = "/deep-freeze-test/test-dropbox-download.txt";
        let res = crate::dropbox::get_file_metadata(&http, dropbox_path).await;
        let json = crate::json::from_res(&res);
        let size = crate::json::get_size(&json);
        assert_eq!(size, 22)
    }

    #[tokio::test]
    async fn it_downloads_from_dropbox() {
        dotenv::dotenv().ok();
        let base_folder: &str = "/deep-freeze-test";
        let file_name: &str = "test-dropbox-download.txt";
        let http = crate::http::new_client();
        let dropbox_path = format!("{base_folder}/{}", &file_name);
        let local_path: &str = &format!("test/{}", file_name.to_string());
        if crate::localfs::local_file_exists(&local_path.to_string()) {
            crate::localfs::delete_local_file(&local_path);
        }
        crate::dropbox::download_from_dropbox(&http, &dropbox_path, &local_path)
            .await
            .unwrap();
        let local_size = crate::localfs::get_local_size(&local_path);
        let dropbox_size = crate::dropbox::get_dropbox_size(&http, &dropbox_path).await;
        assert_eq!(local_size, dropbox_size);
        std::fs::remove_file(&local_path).unwrap();
    }
}
