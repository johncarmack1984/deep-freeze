use std::env;

use indicatif::HumanBytes;
use sedregex::find_and_replace;
use sqlite::{self, ConnectionWithFullMutex};

use crate::{json::JSON, util::setenv};

pub type DBConnection = ConnectionWithFullMutex;
pub type DBRow = sqlite::Row;

pub fn connect(dbpath: &str) -> ConnectionWithFullMutex {
    let sqlite = sqlite::Connection::open_with_full_mutex(dbpath).unwrap();
    init(&sqlite);
    sqlite
}

pub fn reset(dbpath: &str) {
    if crate::localfs::local_file_exists(dbpath) {
        std::fs::remove_file(dbpath).unwrap();
    }
    connect(dbpath);
}

pub fn report_status(sqlite: &ConnectionWithFullMutex) {
    let total_rows = count_rows(&sqlite);
    let total_size = get_pretty_total_size(&sqlite);
    println!("🗃️   Total: {total_rows} files ({total_size})");
    let migrated_rows = count_migrated(&sqlite);
    let migrated_size = get_pretty_migrated_size(&sqlite);
    if migrated_rows > 0 {
        println!("🪺  Migrated: {migrated_rows} files ({migrated_size})");
    }
    let unmigrated_rows = total_rows - migrated_rows;
    let unmigrated_size: String = get_pretty_unmigrated_size(sqlite);
    println!("🪹  Remaining: {unmigrated_rows} files ({unmigrated_size})");
    let percent = if migrated_rows > 0 {
        (100 * migrated_rows / total_rows).abs()
    } else {
        0
    };
    match percent {
        0 => println!("🤷 {percent}% done"),
        100 => {
            println!("🎉 All files migrated");
            std::process::exit(0);
        }
        _ => println!("🎉  {percent}% done!"),
    }
}

pub fn init(connection: &ConnectionWithFullMutex) {
    match connection.execute(
        "
            CREATE TABLE IF NOT EXISTS paths (
                dropbox_id TEXT PRIMARY KEY,
                dropbox_path TEXT NOT NULL,
                dropbox_size INTEGER NOT NULL,
                dropbox_hash TEXT NOT NULL,
                migrated INTEGER NOT NULL DEFAULT -1,
                local_path TEXT UNIQUE DEFAULT NULL,
                local_size INTEGER DEFAULT NULL,
                s3_key TEXT UNIQUE DEFAULT NULL,
                s3_size INTEGER DEFAULT NULL,
                s3_hash TEXT UNIQUE DEFAULT NULL,
                skip INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS user (
                dropbox_user_id TEXT UNIQUE NOT NULL,
                dropbox_team_member_id TEXT NOT NULL,
                dropbox_email TEXT NOT NULL,
                dropbox_refresh_token TEXT NOT NULL,
                dropbox_access_token TEXT NOT NULL,
                dropbox_authorization_code TEXT NOT NULL,
                dropbox_root_namespace_id STRING NOT NULL,
                dropbox_home_namespace_id STRING NOT NULL,
                aws_access_key_id TEXT NOT NULL,
                aws_secret_access_key TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS config (
                dropbox_base_folder,
                s3_bucket TEXT,
                aws_region TEXT
            );
            ",
    ) {
        Ok(_) => println!("📁  Database initialized"),
        Err(err) => panic!("❌  {err}"),
    }
}

pub fn insert_dropbox_paths(connection: &DBConnection, entries: &Vec<serde_json::Value>) {
    let statement = build_insert_statement(&entries);
    match connection.execute(&statement) {
        Ok(_) => println!("🎉 File list updated"),
        Err(err) => {
            println!("❌  Error in statement: {statement}");
            panic!("{}", err);
        }
    }
}

fn build_insert_statement(entries: &Vec<serde_json::Value>) -> String {
    let mut statement = entries
        .iter()
        .filter(|row| row.get(".tag").unwrap().as_str().unwrap() == "file")
        .map(|row| {
            let mut dropbox_path = row
                .clone()
                .get("path_display")
                .unwrap()
                .to_string()
                .to_owned();
            dropbox_path = find_and_replace(&dropbox_path, &["s/\'/\'\'/g"])
                .unwrap()
                .to_string();
            let dropbox_id = row.get("id").unwrap().to_string().to_owned();
            let dropbox_hash = row.get("content_hash").unwrap().to_string().to_owned();
            let dropbox_size = row.get("size").unwrap().to_string().to_owned();
            return format!(
                "('{}', '{}', {}, '{}', -1), ",
                dropbox_id, dropbox_path, dropbox_size, dropbox_hash
            );
        })
        .collect::<Vec<_>>()
        .join("");
    statement = format!(
        "INSERT OR IGNORE INTO paths (dropbox_id, dropbox_path, dropbox_size, dropbox_hash, migrated) VALUES {};",
        statement
    );
    find_and_replace(&statement, &["s/, ;/;/g", "s/\"//g"])
        .unwrap()
        .to_string()
        .to_owned()
}

pub fn insert_config(sqlite: &ConnectionWithFullMutex) {
    let dropbox_base_folder = env::var("DROPBOX_BASE_FOLDER").unwrap_or(String::new());
    let s3_bucket = env::var("S3_BUCKET").unwrap_or(String::new());
    let aws_region = env::var("AWS_REGION").unwrap_or(String::new());
    let statement = format!(
        "INSERT OR REPLACE INTO config (dropbox_base_folder, s3_bucket, aws_region) VALUES ('{}', '{}', '{}');",
        dropbox_base_folder, s3_bucket, aws_region
    );
    match sqlite.execute(&statement) {
        Ok(_) => print!("\n📁  Configuration updated\n\n"),
        Err(err) => {
            println!("❌  Error in statement: {statement}");
            panic!("{}", err);
        }
    }
}

pub fn insert_user(connection: &ConnectionWithFullMutex, member: &JSON) {
    let dropbox_user_id = member.get("account_id").unwrap().as_str().unwrap();
    let dropbox_team_member_id = member.get("team_member_id").unwrap().as_str().unwrap();
    setenv("DROPBOX_TEAM_MEMBER_ID", dropbox_team_member_id.to_string());
    let dropbox_email = member.get("email").unwrap().as_str().unwrap();
    let default = JSON::String("0".to_string());
    let dropbox_root_namespace_id = member
        .get("root_info")
        .unwrap_or(&default)
        .get("root_namespace_id")
        .unwrap_or(&default)
        .as_str()
        .unwrap_or("0");
    setenv(
        "DROPBOX_ROOT_NAMESPACE_ID",
        dropbox_root_namespace_id.to_string(),
    );
    let dropbox_home_namespace_id = member
        .get("root_info")
        .unwrap_or(&default)
        .get("home_namespace_id")
        .unwrap_or(&default)
        .as_str()
        .unwrap_or("0");
    setenv(
        "DROPBOX_HOME_NAMESPACE_ID",
        dropbox_home_namespace_id.to_string(),
    );
    let dropbox_refresh_token = env::var("DROPBOX_REFRESH_TOKEN").unwrap_or(String::new());
    let dropbox_access_token = env::var("DROPBOX_ACCESS_TOKEN").unwrap_or(String::new());
    let dropbox_authorization_code =
        env::var("DROPBOX_AUTHORIZATION_CODE").unwrap_or(String::new());
    let aws_access_key_id = env::var("AWS_ACCESS_KEY_ID").unwrap_or(String::new());
    let aws_secret_access_key = env::var("AWS_SECRET_ACCESS_KEY").unwrap_or(String::new());
    let statement = format!(
            "INSERT OR REPLACE INTO user (dropbox_user_id, dropbox_team_member_id, dropbox_email, dropbox_root_namespace_id, dropbox_home_namespace_id, dropbox_refresh_token, dropbox_access_token, dropbox_authorization_code, aws_access_key_id, aws_secret_access_key) VALUES ('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}','{}','{}');",
            dropbox_user_id, dropbox_team_member_id, dropbox_email, dropbox_root_namespace_id, dropbox_home_namespace_id, dropbox_refresh_token, dropbox_access_token, dropbox_authorization_code, aws_access_key_id, aws_secret_access_key
        );
    match connection.execute(&statement) {
        Ok(_) => {
            println!("👤  User {dropbox_email} updated");
            ()
        }
        Err(err) => {
            print!("\n\n❌  Error in statement:\n\n{:?}\n\n", statement);
            panic!("{}", err);
        }
    }
}

pub fn count_rows(connection: &ConnectionWithFullMutex) -> i64 {
    let query = "SELECT COUNT(*) FROM paths";
    connection
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .map(|row| row.read::<i64, _>(0))
        .next()
        .unwrap()
}

pub fn count_migrated(connection: &ConnectionWithFullMutex) -> i64 {
    let query = "SELECT COUNT(*) FROM paths WHERE migrated = 1";
    connection
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .map(|row| row.read::<i64, _>(0))
        .next()
        .unwrap()
}

pub fn count_unmigrated(connection: &ConnectionWithFullMutex) -> i64 {
    let query = "SELECT COUNT(*) FROM paths WHERE migrated < 1";
    connection
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .map(|row| row.read::<i64, _>(0))
        .next()
        .unwrap()
}

pub fn set_migrated(connection: &ConnectionWithFullMutex, dropbox_id: &str) {
    match connection.execute(format!(
        "UPDATE paths SET migrated = 1 WHERE dropbox_id = '{dropbox_id}';",
    )) {
        Ok(_) => println!("🪺  Migrated: {dropbox_id}"),
        Err(err) => panic!("❌  {err}"),
    }
}

pub fn set_unmigrated(connection: &ConnectionWithFullMutex, dropbox_id: &str) {
    match connection.execute(format!(
        "UPDATE paths SET migrated = 0 WHERE dropbox_id = '{dropbox_id}';",
    )) {
        Ok(_) => println!("🪹  Not migrated: {dropbox_id}"),
        Err(err) => panic!("❌  {err}"),
    }
}

pub fn set_skip(connection: &ConnectionWithFullMutex, dropbox_id: &str) {
    match connection.execute(format!(
        "UPDATE paths SET skip = 1 WHERE dropbox_id = '{dropbox_id}';",
    )) {
        Ok(_) => println!("🪹   Skipping: {dropbox_id}"),
        Err(err) => panic!("❌  {err}"),
    }
}

pub fn get_pretty_total_size(connection: &ConnectionWithFullMutex) -> String {
    let size: f64;
    if count_rows(connection) == 0 {
        size = 0.0;
    } else {
        let query = "SELECT SUM(dropbox_size) FROM paths";
        size = connection
            .prepare(query)
            .unwrap()
            .into_iter()
            .map(|row| row.unwrap())
            .map(|row| row.read::<i64, _>(0))
            .next()
            .unwrap() as f64;
    }
    match size {
        size if size == 0.0 => "0 bytes".to_string(),
        size if size > 0.0 => HumanBytes(size as u64).to_string(),
        _ => panic!("❌  Negative size"),
    }
}

pub fn get_pretty_migrated_size(connection: &ConnectionWithFullMutex) -> String {
    let size: f64;
    if count_migrated(connection) == 0 {
        size = 0.0;
    } else {
        let query = "SELECT SUM(dropbox_size) FROM paths WHERE migrated = 1";
        size = connection
            .prepare(query)
            .unwrap()
            .into_iter()
            .map(|row| row.unwrap())
            .map(|row| row.read::<i64, _>(0))
            .next()
            .unwrap() as f64;
    }
    match size {
        size if size == 0.0 => "0 bytes".to_string(),
        size if size > 0.0 => HumanBytes(size as u64).to_string(),
        _ => panic!("❌  Negative size"),
    }
}

pub fn get_pretty_unmigrated_size(connection: &ConnectionWithFullMutex) -> String {
    let size: f64;
    if count_unmigrated(connection) == 0 {
        size = 0.0;
    } else {
        let query = "SELECT SUM(dropbox_size) FROM paths WHERE migrated < 1";
        size = connection
            .prepare(query)
            .unwrap()
            .into_iter()
            .map(|row| row.unwrap())
            .map(|row| row.read::<i64, _>(0))
            .next()
            .unwrap() as f64;
    }
    match size {
        size if size == 0.0 => "0 bytes".to_string(),
        size if size > 0.0 => HumanBytes(size as u64).to_string(),
        _ => panic!("❌  Negative size"),
    }
}

pub fn get_dropbox_size(connection: &ConnectionWithFullMutex, dropbox_id: &str) -> i64 {
    let query = format!("SELECT dropbox_size FROM paths WHERE dropbox_id = '{dropbox_id}';");
    connection
        .prepare(&query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .map(|row| row.read::<i64, _>(0))
        .next()
        .unwrap()
}

// pub fn get_unmigrated_rows(connection: &ConnectionWithFullMutex) -> Vec<sqlite::Row> {
//     let query = "SELECT * FROM paths WHERE migrated < 1";
//     connection
//         .prepare(query)
//         .unwrap()
//         .into_iter()
//         .map(|row| row.unwrap())
//         .collect::<Vec<_>>()
// }
