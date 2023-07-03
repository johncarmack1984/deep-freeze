mod auth;
mod aws;
mod db;
mod deepfreeze;
mod dropbox;
mod http;
mod json;
mod localfs;
mod progress;
mod util;

use dotenv::dotenv;

use aws::AWSClient;
use clap::Parser;
use db::DBConnection;
use http::HTTPClient;
use std::env;
use util::setenv;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the sqlite database file
    #[arg(short, long, default_value = "db.sqlite")]
    dbfile: String,
    /// Run the program end-to-end with test values
    #[arg(short, long, default_value = "false")]
    e2e: bool,
    /// Reset the database and temp files
    #[arg(short, long, default_value = "false")]
    reset: bool,
    /// Reset the database and temp files, then exit
    #[arg(short = 'R', long, default_value = "false")]
    reset_only: bool,
    /// Run in silent mode
    #[arg(short, long, default_value = "false")]
    silent: bool,
    /// Skip these paths (e.g. --skip "path1,path2")
    #[arg(short = 'k', long)]
    skip: Vec<String>,
    /// Path to the temp directory
    #[arg(short, long, default_value = "temp")]
    temp_dir: String,
}

#[tokio::main]
async fn main() {
    init(Args::parse()).await;

    let http: HTTPClient = http::new_client();
    auth::check_account(&http).await;
    let sqlite: DBConnection = db::connect(
        std::env::var("DBFILE")
            .unwrap_or("db.sqlite".to_string())
            .as_str(),
    );
    dropbox::get_paths(&http, &sqlite).await;
    let aws: AWSClient = aws::new_client().await;
    match deepfreeze::perform_migration(http, sqlite, aws).await {
        Ok(_) => {
            println!("✅ Migration complete");
            ::std::process::exit(0)
        }
        Err(_e) => {
            println!("🚨 Migration failed");
            ::std::process::exit(1)
        }
    }
}

async fn init(args: Args) {
    dotenv().ok();
    setenv("SILENT", args.silent.to_string()).unwrap();
    if env::var("SILENT").unwrap() == "true" {
        println!("🔇 Running in silent mode...");
    }
    setenv("RESET", args.reset.to_string()).unwrap();
    setenv("RESET_ONLY", args.reset_only.to_string()).unwrap();
    ::std::env::set_var("E2E", args.e2e.to_string());
    if env::var("E2E").unwrap() == "true" {
        ::std::env::set_var("DBFILE", "test/db.sqlite");
        ::std::env::set_var("TEMP_DIR", "test");
        ::std::env::set_var("SILENT", "false");
        ::std::env::set_var("RESET", "true");
        ::std::env::set_var("BASE_FOLDER", "/deep-freeze-test");
        ::std::env::set_var("S3_BUCKET", "deep-freeze-test");
        ::std::env::set_var("RUST_BACKTRACE", "1");
    }
    if env::var("RESET").unwrap() == "true" || env::var("RESET_ONLY").unwrap() == "true" {
        reset().await;
        if env::var("RESET_ONLY").unwrap() == "true" {
            println!("👌  Reset only");
            println!("✅  Exiting");
            ::std::process::exit(0)
        }
    }
}

async fn reset() {
    println!("🗑️  Resetting database and temp files");
    db::reset(std::env::var("DBFILE").unwrap().as_str());
    println!("🚮  Database reset");
    localfs::reset();
    println!("🚮  Temp files deleted");
    if env::var("E2E").unwrap() == "true" {
        println!("🗑️  Resetting test bucket");
        crate::aws::_empty_test_bucket().await;
        println!("🚮  Test bucket reset");
    }
    println!("🎉 Reset complete");
}
