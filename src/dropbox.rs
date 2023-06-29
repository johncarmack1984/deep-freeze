use reqwest::header::HeaderMap;
use sedregex::find_and_replace;
use std::{env, error::Error};
// use indicatif::{ProgressBar, ProgressStyle};

#[async_recursion::async_recursion(?Send)]
pub async fn add_files_to_list(
    res: String,
    connection: &sqlite::ConnectionWithFullMutex,
) -> Result<(), Box<dyn std::error::Error>> {
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
    println!("🗄️  {count} files found");
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
            return add_files_to_list(res, &connection).await;
        }
        Some(false) | None => {
            println!("✅  File list populated");
        }
    })
}

#[async_recursion::async_recursion(?Send)]
pub async fn get_paths(connection: &sqlite::ConnectionWithFullMutex) {
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
        match add_files_to_list(res, &connection).await {
            Ok(_) => get_paths(&connection).await,
            Err(err) => panic!("{err}"),
        }
    }
    println!("🗃️  {} files in database", count);
    let migrated_query = "SELECT COUNT(*) FROM paths WHERE migrated = 1";
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
        _ => {
            println!("🗃️  {} files left to migrate", diff);
            println!("🎉 {}% done!", 100 * migrated / count)
        }
    }
}

pub async fn download_from_db(dropbox_path: &str, local_path: &str) -> Result<(), Box<dyn Error>> {
    // // Reqwest setup
    // let access_token = env::var("ACCESS_TOKEN")?;
    // let team_member_id = env::var("TEAM_MEMBER_ID")?;
    // let mut headers = HeaderMap::new();
    // headers.insert(
    //     "Authorization",
    //     format!("Bearer {}", access_token).parse().unwrap(),
    // );
    // headers.insert("Dropbox-API-Select-Admin", team_member_id.parse().unwrap());
    // headers.insert(
    //     "Dropbox-API-Arg",
    //     format!("{{\"path\":\"{}\"}}", dropbox_path)
    //         .parse()
    //         .unwrap(),
    // );
    // let client = reqwest::Client::new();
    // let res = client
    //     .post("https://content.dropboxapi.com/2/files/download")
    //     .headers(headers)
    //     .send()
    //     .await?;

    // let total_size = res.content_length().ok_or(format!(
    //     "Failed to get content length from '{}'",
    //     &dropbox_path
    // ))?;

    println!("⬇️  Checking for saved spot in download {dropbox_path}");
    println!("⬇️  Checking for saved spot in download {local_path}");

    // // Indicatif setup
    // let pb = ProgressBar::new(total_size);
    // pb.set_style(ProgressStyle::default_bar()
    //     .template("{msg}\n{spinner:.green}  [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
    //     .unwrap()
    //     .progress_chars("█  "));
    // let msg = format!("⬇️ Downloading {}", dropbox_path);
    // pb.set_message(msg);

    // let mut file;
    // let mut downloaded: u64 = 0;
    // let mut stream = res.bytes_stream();

    // if std::path::Path::new(local_path).exists()
    //     && std::fs::metadata(local_path).unwrap().is_dir() == false
    // {
    //     println!("⬇️  File exists. Resuming.");
    //     file = std::fs::OpenOptions::new()
    //         .read(true)
    //         .append(true)
    //         .open(local_path)
    //         .unwrap();

    //     let file_size = std::fs::metadata(local_path).unwrap().len();
    //     file.seek(std::io::SeekFrom::Start(file_size)).unwrap();
    //     downloaded = file_size;
    // } else if std::path::Path::new(local_path).exists()
    //     && std::fs::metadata(local_path).unwrap().is_dir() == true
    // {
    //     println!("⌫  Key exists as directory. Erasing.");
    //     std::fs::remove_dir(local_path).unwrap();
    //     println!("⬇️  Fresh file.");
    //     file = File::create(local_path)
    //         .or(Err(format!("❌  Failed to create file '{}'", local_path)))?;
    // } else {
    //     println!("⬇️  Fresh file.");
    //     file = File::create(local_path)
    //         .or(Err(format!("❌  Failed to create file '{}'", local_path)))?;
    // }

    // println!("Commencing transfer");
    // while let Some(item) = stream.next().await {
    //     let chunk = item.or(Err(format!("❌  Error while downloading file")))?;
    //     file.write(&chunk)
    //         .or(Err(format!("❌  Error while writing to file")))?;
    //     let new = min(downloaded + (chunk.len() as u64), total_size);
    //     downloaded = new;
    //     pb.set_position(new);
    // }
    // let finished_msg = format!("🎉  Finished downloading {}", dropbox_path);
    // pb.finish_with_message(finished_msg);
    Ok(())
}
