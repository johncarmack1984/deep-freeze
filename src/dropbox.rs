use futures_util::StreamExt;
use std::cmp::min;
use std::io::Write;
use std::{env, error::Error};

use crate::db::{self, DBConnection};
use crate::http::{self, HTTPClient, HeaderMap};
use crate::json::{self, JSON};
use crate::localfs::{self, create_local_file};
use crate::progress;
use crate::util::setenv;

pub async fn add_files_to_list(
    json: &JSON,
    connection: &DBConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        json.get("error"),
        None,
        "🛑 DropBox returned an error {json}"
    );
    let count: usize = json::count_files(&json);
    println!("🗄️  {count} files found");
    if count > 0 {
        db::insert_dropbox_paths(&connection, json::get_entries(&json));
    }
    Ok(())
}

pub async fn get_team_members_list(http: &HTTPClient) -> String {
    let mut headers = HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_content_type_json_header(&mut headers);
    let body = format!("{{\"limit\": 1000}}");
    http.post("https://api.dropboxapi.com/2/team/members/list_v2")
        .headers(headers)
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

async fn list_folder(http: &HTTPClient, recursive: bool) -> String {
    // let base_folder = env::var("BASE_FOLDER").unwrap();
    // "{{\"path\": \"{}\", \"recursive\": true,  \"limit\": 2000, \"include_non_downloadable_files\": false}}",
    let mut headers = HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_content_type_json_header(&mut headers);
    headers = http::dropbox_select_admin_header(&mut headers);
    headers = http::dropbox_api_path_root_header(&mut headers);
    let body = format!(
        "{{\"path\": \"{}\", \"recursive\": {},  \"limit\": 2000, \"include_non_downloadable_files\": false}}",
        env::var("DROPBOX_BASE_FOLDER").unwrap_or("".to_string()), recursive
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
    headers = http::dropbox_api_path_root_header(&mut headers);
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

pub async fn choose_folder(http: &HTTPClient, database: &DBConnection) {
    let recursive = false;
    let res = list_folder(&http, recursive).await;
    let json: JSON = json::from_res(&res);
    let folders = json.get("entries").unwrap().as_array().unwrap();
    let options: Vec<String> = folders
        .into_iter()
        .map(|folder| {
            let path = folder.get("path_display").unwrap().as_str().unwrap();
            path.to_string()
        })
        .collect();
    match inquire::Select::new("🗄️  Choose a folder to scan", options)
        .with_page_size(20)
        .prompt()
    {
        Ok(choice) => {
            println!("🗄️  You chose {choice}");
            setenv("DROPBOX_BASE_FOLDER", choice);
            db::insert_config(&database);
        }
        Err(err) => panic!("❌  Error choosing folder {err}"),
    }
}

// #[async_recursion::async_recursion(?Send)]
pub async fn get_paths(http: &HTTPClient, sqlite: &DBConnection) {
    print!("🗄️   Getting file list...\n");
    let count = db::count_rows(&sqlite);
    if count == 0 {
        println!("🗄️  File list empty");
        if dotenv::var("DROPBOX_BASE_FOLDER").is_err() {
            choose_folder(http, sqlite).await;
        }
        println!("🗄️  Populating file list...");
        let recursive = true;
        let mut res = list_folder(&http, recursive).await;
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
        print!("\n");
    }
    db::report_status(&sqlite).try_into().unwrap()
}

pub async fn get_file_metadata(http: &HTTPClient, dropbox_path: &str) -> String {
    let mut headers = HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_select_admin_header(&mut headers);
    headers = http::dropbox_content_type_json_header(&mut headers);
    let body = format!("{{\"path\": \"{dropbox_path}\"}}");
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
    http: &HTTPClient,
    dropbox_id: &str,
    _dropbox_path: &str,
    local_path: &str,
    m: &&crate::progress::MultiProgress,
) -> Result<(), Box<dyn Error>> {
    let dropbox_size = get_dropbox_size(http, dropbox_id).await;
    let mut headers = HeaderMap::new();
    headers = http::dropbox_authorization_header(&mut headers);
    headers = http::dropbox_select_admin_header(&mut headers);
    headers.insert(
        "Dropbox-API-Arg",
        format!("{{\"path\":\"{dropbox_id}\"}}").parse().unwrap(),
    );
    let res = http
        .post("https://content.dropboxapi.com/2/files/download")
        .headers(headers)
        .send()
        .await?;
    let mut stream = res.bytes_stream();
    let mut file;
    let mut downloaded: u64 = 0;
    let pb = m.add(progress::new(dropbox_size as u64, "file_transfer"));
    pb.set_prefix("⬇️   Download  ");
    if localfs::get_local_size(&local_path) != dropbox_size {
        file = create_local_file(&local_path);
        while let Some(item) = stream.next().await {
            let chunk = item.or(Err(format!("❌  Error while downloading file")))?;
            let new = min(downloaded + (chunk.len() as u64), dropbox_size as u64);
            downloaded = new;
            pb.set_position(downloaded);
            file.write(&chunk)
                .or(Err(format!("❌  Error while writing to file")))?;
        }
    }
    pb.finish();
    pb.set_prefix("✅  Download ");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn it_gets_file_metadata_from_dropbox() {
        dotenv::dotenv().ok();
        ::std::env::set_var("SILENT", "true");
        let http = crate::http::new_client();
        let dropbox_path = "/deep-freeze-test/test-dropbox-download.txt";
        let res = crate::dropbox::get_file_metadata(&http, dropbox_path).await;
        let json = crate::json::from_res(&res);
        let size = crate::json::get_size(&json);
        assert_eq!(size, 22);
        let id = crate::json::_get_id(&json);
        assert_eq!(id, "id:FVFwt7Ga8wEAAAAAACwqDA")
    }

    #[tokio::test]
    async fn it_downloads_from_dropbox() {
        dotenv::dotenv().ok();
        ::std::env::set_var("SILENT", "true");
        let base_folder: &str = "/deep-freeze-test";
        let file_name: &str = "test-dropbox-download.txt";
        let http = crate::http::new_client();
        let dropbox_path = format!("{base_folder}/{}", &file_name);
        let local_path: &str = &format!("test/{}", file_name.to_string());
        if crate::localfs::local_file_exists(&local_path.to_string()) {
            crate::localfs::delete_local_file(&local_path);
        }
        let res = crate::dropbox::get_file_metadata(&http, &dropbox_path).await;
        let json = crate::json::from_res(&res);
        let dropbox_size = crate::json::get_size(&json);
        let dropbox_id = crate::json::_get_id(&json);
        crate::dropbox::download_from_dropbox(
            &http,
            &dropbox_id,
            &dropbox_path,
            &local_path,
            &&crate::progress::new_multi_progress(),
        )
        .await
        .unwrap();
        let local_size = crate::localfs::get_local_size(&local_path);
        assert_eq!(local_size, dropbox_size);
        crate::localfs::delete_local_file(&local_path);
    }
}
