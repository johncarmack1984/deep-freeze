use sedregex::find_and_replace;
use sqlite::{self, ConnectionWithFullMutex};

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
    println!("🗃️  {total_rows} files in database");
    let migrated_rows = count_migrated(&sqlite);
    if migrated_rows > 0 {
        println!("🎉 {migrated_rows} already migrated");
    }
    let unmigrated_rows = total_rows - migrated_rows;
    let unmigrated_size: String = get_pretty_unmigrated_size(sqlite);
    println!("🗃️  {unmigrated_rows} files ({unmigrated_size}) left to migrate");
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
        _ => println!("🎉 {percent}% done!"),
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
                skip INTEGER NOT NULL DEFAULT 0,
                local_path TEXT UNIQUE DEFAULT NULL,
                local_size INTEGER DEFAULT NULL,
                s3_key TEXT UNIQUE DEFAULT NULL,
                s3_size INTEGER DEFAULT NULL,
                s3_hash TEXT UNIQUE DEFAULT NULL
            );
            ",
    ) {
        Ok(_) => println!("📁 Database initialized"),
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
        Ok(_) => println!("🪹   Not migrated: {dropbox_id}"),
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
        size if size > 0.0 => pretty_bytes::converter::convert(size),
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

// CREATE TABLE IF NOT EXISTS user (
//     dropbox_user_id TEXT NOT NULL,
//     dropbox_refresh_token TEXT NOT NULL,
//     dropbox_access_token TEXT NOT NULL,
//     dropbox_authorization_code TEXT NOT NULL,
//     aws_access_key_id TEXT NOT NULL,
//     aws_secret_key TEXT NOT NULL
// );
// CREATE TABLE IF NOT EXISTS config (
//     dropbox_app_key TEXT NOT NULL,
//     dropbox_app_secret TEXT NOT NULL,
//     dropbox_base_folder TEXT NOT NULL,
//     s3_bucket TEXT NOT NULL,
//     aws_region TEXT NOT NULL
// );
