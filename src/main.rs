#![allow(unused)]

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
use std::{env, os::unix::process};
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

    /// Run in silent mode
    #[arg(short, long, default_value = "false")]
    silent: bool,

    /// Path to the temp directory
    #[arg(short, long, default_value = "temp")]
    temp_dir: String,
}

#[tokio::main]
async fn main() {
    init(Args::parse()).await;

    let http: HTTPClient = http::new_client();
    auth::check_account(&http).await;
    let sqlite: DBConnection = db::connect(std::env::var("DBFILE").unwrap().as_str());
    dropbox::get_paths(&http, &sqlite).await;
    let aws: AWSClient = aws::new_client().await;
    match deepfreeze::perform_migration(http, sqlite, aws).await {
        Ok(_) => {
            println!("🎉 Migration complete");
            ::std::process::exit(0)
        }
        Err(e) => {
            println!("🚨 Migration failed: {}", e);
            ::std::process::exit(1)
        }
    }
}

async fn init(args: Args) {
    dotenv().ok();
    setenv("SILENT", args.silent.to_string());
    if env::var("SILENT").unwrap() == "true" {
        println!("🔇 Running in silent mode...");
    }
    setenv("RESET", args.reset.to_string());
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
    if env::var("RESET").unwrap() == "true" {
        reset().await;
    }
}

async fn reset() {
    println!("🗑️  Resetting database and temp files");
    db::reset(std::env::var("DBFILE").unwrap().as_str());
    println!("🚮  Database reset");
    localfs::reset();
    println!("🚮  Temp files deleted");
    crate::aws::_empty_test_bucket().await;
    println!("🎉 Reset complete");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn it_runs_end_to_end() {
        ::std::env::set_var("E2E", "true");
        main();
        assert_eq!(true, true);
    }
}
