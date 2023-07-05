#![allow(dead_code, unused)]

mod auth;
mod aws;
mod cli;
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
use std::{env, process};
use util::{getenv, setenv};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Dropbox access token
    #[arg(short = 'a', long, default_value = "")]
    access_token: String,
    /// Path to the sqlite database file
    #[arg(short, long, default_value = "db.sqlite")]
    dbfile: String,
    /// Path to the .env file
    #[arg(short = 'v', long, default_value = ".env")]
    env_file: String,
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
    #[arg(short = 't', long, default_value = "temp")]
    temp_dir: String,
}

#[tokio::main]
async fn main() {
    print!("\n🧊🧊🧊 Deep Freeze - Migrate Files to S3 Deep Archive 🧊🧊🧊\n\n");
    let (database, http, aws) = init(Args::parse()).await;

    auth::check_account(&http, &database).await;
    std::process::exit(0);
    dropbox::get_paths(&http, &database).await;

    match deepfreeze::perform_migration(http, database, aws).await {
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

async fn init(args: Args) -> (DBConnection, HTTPClient, AWSClient) {
    setenv("ENV_FILE", args.env_file);
    setenv("SILENT", args.silent.to_string());
    if env::var("SILENT").unwrap() == "true" {
        println!("🔇 Running in silent mode...");
    }
    if dotenv::var("SKIP").is_err() {
        setenv("SKIP", args.skip.join(","));
    }
    if env::var("SKIP").unwrap() == "" {
        println!("🚫 Skipping no paths");
    } else {
        println!("🚫 Skipping paths: {}", env::var("SKIP").unwrap());
    }
    setenv("TEMP_DIR", args.temp_dir);
    if env::var("TEMP_DIR").unwrap() != "temp" {
        println!("📁 Using temp directory: {}", env::var("TEMP_DIR").unwrap());
    }
    if dotenv::var("DBFILE").is_err() || args.dbfile != "db.sqlite" {
        setenv("DBFILE", args.dbfile);
    }
    println!("🗄️  Using database file: {}", getenv("DBFILE"));
    setenv("RESET", args.reset.to_string());
    setenv("RESET_ONLY", args.reset_only.to_string());
    if args.e2e {
        println!("🧪 Running end-to-end test");
        env::set_var("E2E", args.e2e.to_string());
        env::set_var("DBFILE", "test/db.sqlite");
        env::set_var("TEMP_DIR", "test");
        env::set_var("SILENT", "false");
        env::set_var("RESET", "true");
        env::set_var("BASE_FOLDER", "/deep-freeze-test");
        env::set_var("S3_BUCKET", "deep-freeze-test");
        env::set_var("RUST_BACKTRACE", "1");
    }
    if env::var("RESET").unwrap() == "true" || env::var("RESET_ONLY").unwrap() == "true" {
        reset().await;
        if env::var("RESET_ONLY").unwrap() == "true" {
            println!("👌  Reset only");
            println!("✅  Exiting");
            process::exit(0)
        }
    }
    if !args.access_token.is_empty() {
        setenv("DROPBOX_ACCESS_TOKEN", args.access_token);
    }
    let database: DBConnection = db::connect(getenv("DBFILE").as_str());
    let http: HTTPClient = http::new_client();
    let aws: AWSClient = aws::new_client().await;
    (database, http, aws)
}

async fn reset() {
    println!("🗑️  Resetting database and temp files");
    db::reset(env::var("DBFILE").unwrap().as_str());
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
