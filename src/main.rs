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
    #[arg(long, default_value = "")]
    access_token: String,
    /// AWS access key ID
    #[arg(long, default_value = "")]
    aws_access_key_id: String,
    /// AWS secret access key
    #[arg(long, default_value = "")]
    aws_secret_access_key: String,
    /// AWS region
    #[arg(long, default_value = "")]
    aws_region: String,
    /// Check the migration status of files
    #[arg(short, long, default_value = "false")]
    check_only: bool,
    /// Path to the sqlite database file
    #[arg(long, default_value = "db.sqlite")]
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
    /// Define the S3 folder to use
    #[arg(long, default_value = "")]
    s3_bucket: String,
    /// Run in silent mode
    #[arg(short, long, default_value = "false")]
    silent: bool,
    /// Skip these paths (e.g. --skip "path1,path2")
    #[arg(long)]
    skip: Vec<String>,
    /// Display the migration status of files, then exit
    #[arg(long, default_value = "false")]
    status_only: bool,
    /// Path to the temp directory
    #[arg(long, default_value = "temp")]
    temp_dir: String,
}

#[tokio::main]
async fn main() {
    print!("\n🧊🧊🧊 Deep Freeze - Migrate Files to S3 Deep Archive 🧊🧊🧊\n\n");
    let (database, http, aws) = init(Args::parse()).await;

    auth::check_account(&http, &database).await;
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
    setenv("CHECK_ONLY", args.check_only.to_string());
    setenv("STATUS_ONLY", args.status_only.to_string());
    setenv("RESET", args.reset.to_string());
    setenv("RESET_ONLY", args.reset_only.to_string());
    if args.e2e {
        println!("🧪 Running end-to-end test");
        env::set_var("E2E", args.e2e.to_string());
        env::set_var("DBFILE", "test/db.sqlite");
        env::set_var("TEMP_DIR", "test");
        env::set_var("SILENT", "false");
        env::set_var("RESET", "true");
        env::set_var("DROPBOX_BASE_FOLDER", "/deep-freeze-test");
        env::set_var("AWS_S3_BUCKET", "deep-freeze-test");
        env::set_var("RUST_BACKTRACE", "1");
    }
    if dotenv::var("DBFILE").is_err() || args.dbfile != "db.sqlite" {
        setenv("DBFILE", args.dbfile);
    }
    if env::var("RESET").unwrap() == "true" || env::var("RESET_ONLY").unwrap() == "true" {
        reset().await;
        if env::var("RESET_ONLY").unwrap() == "true" {
            println!("👌  Reset only");
            println!("✅  Exiting");
            process::exit(0)
        }
    }
    if dotenv::var("SKIP").is_err() {
        setenv("SKIP", args.skip.join(","));
    }
    if env::var("SKIP").unwrap() == "" {
        print!("🚫 Skipping no paths\n\n");
    } else {
        print!("🚫 Skipping paths: {}\n\n", env::var("SKIP").unwrap());
    }
    setenv("TEMP_DIR", args.temp_dir);
    if env::var("TEMP_DIR").unwrap() != "temp" {
        println!("📁 Using temp directory: {}", env::var("TEMP_DIR").unwrap());
    }
    println!("🗄️  Using database file: {}", getenv("DBFILE"));

    if !args.access_token.is_empty() {
        setenv("DROPBOX_ACCESS_TOKEN", args.access_token);
    }
    if args.aws_access_key_id != "" {
        setenv("AWS_ACCESS_KEY_ID", args.aws_access_key_id);
    }
    if env::var("AWS_ACCESS_KEY_ID").is_err() {
        let aws_access_key_id = util::prompt("📦  AWS access key ID");
        setenv("AWS_ACCESS_KEY_ID", aws_access_key_id);
    }
    if args.aws_secret_access_key != "" {
        setenv("AWS_SECRET_ACCESS_KEY", args.aws_secret_access_key);
    }
    if env::var("AWS_SECRET_ACCESS_KEY").is_err() {
        let aws_secret_access_key = util::prompt("📦  AWS secret access key");
        setenv("AWS_SECRET_ACCESS_KEY", aws_secret_access_key);
    }
    if args.s3_bucket != "" {
        setenv("AWS_S3_BUCKET", args.s3_bucket);
    }

    let database: DBConnection = db::connect(getenv("DBFILE").as_str());

    if getenv("STATUS_ONLY") == "true" {
        db::report_status(&database);
        println!("✅  Exiting");
        process::exit(0)
    }

    let http: HTTPClient = http::new_client();
    let aws: AWSClient = aws::new_client().await;

    if env::var("AWS_S3_BUCKET").is_err() {
        aws::choose_bucket(&aws, &database).await;
    }
    if args.aws_region != "" {
        setenv("AWS_REGION", args.aws_region);
    }
    if env::var("AWS_REGION").is_err() {
        // let aws_region = util::prompt("📦  AWS region");
        setenv("AWS_REGION", "us-east-1".to_string());
    }
    (database, http, aws)
}

async fn reset() {
    println!("🗑️  Resetting database and temp files");
    db::reset(env::var("DBFILE").unwrap().as_str());
    println!("🚮  Database reset");
    localfs::reset();
    println!("🚮  Temp & env files deleted");
    if dotenv::var("E2E").is_ok() && env::var("E2E").unwrap() == "true" {
        println!("🗑️  Resetting test bucket");
        crate::aws::_empty_test_bucket().await;
        println!("🚮  Test bucket reset");
    }
    print!("🎉 Reset complete\n\n");
}
