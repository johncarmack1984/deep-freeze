use sedregex::find_and_replace;
use sqlite::{Connection, ConnectionWithFullMutex};

pub fn connect() -> ConnectionWithFullMutex {
    Connection::open_with_full_mutex("db.sqlite").unwrap()
}

pub fn init(connection: &ConnectionWithFullMutex) {
    match connection.execute(
        "
            CREATE TABLE IF NOT EXISTS paths (
                path TEXT NOT NULL UNIQUE,
                size INTEGER NOT NULL,
                hash TEXT NOT NULL,
                migrated INTEGER NOT NULL DEFAULT -1
            );
            ",
    ) {
        Ok(_) => println!("📁 Database initialized"),
        Err(err) => panic!("❌  {err}"),
    }
}

pub fn insert_dropbox_paths(
    connection: &ConnectionWithFullMutex,
    entries: &Vec<serde_json::Value>,
) {
    let statement = build_insert_statement(&entries);
    match connection.execute(&statement) {
        Ok(_) => println!("🎉 File list updated"),
        Err(err) => {
            println!("❌  Error in statement: {}", statement);
            panic!("{}", err);
        }
    }
}

fn build_insert_statement(entries: &Vec<serde_json::Value>) -> String {
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

pub fn set_migrated(dropbox_path: &str, connection: &ConnectionWithFullMutex) {
    match connection.execute(format!(
        "UPDATE paths SET migrated = 1 WHERE path = '{dropbox_path}';",
    )) {
        Ok(_) => println!("☑️ Set migrated {dropbox_path}"),
        Err(err) => panic!("❌  {err}"),
    }
}

pub fn set_unmigrated(dropbox_path: &str, connection: &ConnectionWithFullMutex) {
    match connection.execute(format!(
        "UPDATE paths SET migrated = 0 WHERE path = '{dropbox_path}';",
    )) {
        Ok(_) => println!("🅇 Set unmigrated {dropbox_path}"),
        Err(err) => panic!("❌  {err}"),
    }
}

pub fn get_unmigrated_rows(connection: &ConnectionWithFullMutex) -> Vec<sqlite::Row> {
    let query = "SELECT * FROM paths WHERE migrated < 1";
    connection
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>()
}
