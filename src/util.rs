use sedregex::find_and_replace;
use std::{env, io::stdin};
use tokio::io::{self, AsyncWriteExt};

use crate::localfs;

pub async fn setenv(key: &str, value: String) {
    env::set_var(key, value.clone());
    localfs::update_env_file(key, value).await.unwrap();
}

pub fn getenv(key: &str) -> Result<String, env::VarError> {
    env::var(key)
}

pub async fn setenv_for_e2e() {
    println!("🧪 Running end-to-end test");
    setenv("ENV_FILE", ".env.test".to_string()).await;
    setenv("E2E", "true".to_string()).await;
    setenv("DBFILE", "test/db.sqlite".to_string()).await;
    setenv("TEMP_DIR", "test".to_string()).await;
    setenv("SILENT", "false".to_string()).await;
    setenv("RESET", "true".to_string()).await;
    setenv("DROPBOX_BASE_FOLDER", "/deep-freeze-test".to_string()).await;
    setenv("AWS_S3_BUCKET", "deep-freeze-test".to_string()).await;
    setenv("RUST_BACKTRACE", "1".to_string()).await;
}

pub async fn prompt(msg: &str) -> String {
    io::stderr().flush().await.unwrap();
    eprint!("{}: ", msg);
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    eprint!("\n\n");
    input.trim().to_owned()
}

pub fn standardize_path(old_path: &str) -> String {
    let base_folder = getenv("DROPBOX_BASE_FOLDER").unwrap();
    let mut path = find_and_replace(
        &old_path.clone().to_owned(),
        &[format!("s/\\{}\\///g", base_folder)],
    )
    .unwrap()
    .to_string();

    path = find_and_replace(
        &path,
        &["s/channel/Channel/g", "s/_/_/g", "s/\\|/\\|/g", "s/•/\\•/g"],
    )
    .unwrap()
    .to_string();

    path.to_string()
}

pub fn coerce_static_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}
