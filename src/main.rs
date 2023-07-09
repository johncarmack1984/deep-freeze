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

use aws::AWSClient;
use clap::Parser;
use db::DBConnection;
use http::HTTPClient;
use std::process;
use tokio::signal;
use util::{getenv, setenv, setenv_for_e2e};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Dropbox access token
    #[arg(long, default_value = "")]
    access_token: String,
    /// Refresh the Dropbox access token, then exit (useful for CI)
    #[arg(long, default_value = "false")]
    auth_only: bool,
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
    deepfreeze::perform_migration(http, database, aws).await;

    cleanup().await;

    match signal::ctrl_c().await {
        Ok(_) => {
            cleanup().await;
            println!("✅  Exiting");
            process::exit(0)
        }
        Err(err) => {
            cleanup().await;
            eprintln!("🚨  Exiting with error {}", err);
            process::exit(1)
        }
    }
}

async fn init(args: Args) -> (DBConnection, HTTPClient, AWSClient) {
    setenv("ENV_FILE", args.env_file).await;
    setenv("SILENT", args.silent.to_string()).await;
    if getenv("SILENT").unwrap() == "true" {
        println!("🔇 Running in silent mode...");
    }
    setenv("CHECK_ONLY", args.check_only.to_string()).await;
    setenv("STATUS_ONLY", args.status_only.to_string()).await;
    setenv("RESET", args.reset.to_string()).await;
    setenv("RESET_ONLY", args.reset_only.to_string()).await;
    if args.e2e {
        setenv_for_e2e().await;
    }
    if getenv("DBFILE").is_err() || args.dbfile != "db.sqlite" {
        setenv("DBFILE", args.dbfile).await;
    }
    setenv("TEMP_DIR", args.temp_dir).await;
    if getenv("TEMP_DIR").unwrap() != "temp" {
        println!("📁 Using temp directory: {}", getenv("TEMP_DIR").unwrap());
    }
    if getenv("RESET").unwrap() == "true" || getenv("RESET_ONLY").unwrap() == "true" {
        reset().await;
        if getenv("RESET_ONLY").unwrap() == "true" {
            println!("👌  Reset only");
            println!("✅  Exiting");
            process::exit(0)
        }
    }
    if getenv("SKIP").is_err() {
        setenv("SKIP", args.skip.join(",")).await;
    }
    if getenv("SKIP").unwrap() == "" {
        println!("⏭️   Skipping no paths\n");
    } else {
        print!("⏭️   Skipping paths: {}\n", getenv("SKIP").unwrap());
    }
    println!("🗄️  Using database file: {}\n", getenv("DBFILE").unwrap());

    if !args.access_token.is_empty() {
        setenv("DROPBOX_ACCESS_TOKEN", args.access_token).await;
    }
    if args.aws_access_key_id != "" {
        setenv("AWS_ACCESS_KEY_ID", args.aws_access_key_id).await;
    }
    if getenv("AWS_ACCESS_KEY_ID").is_err() {
        let aws_access_key_id = util::prompt("📦  AWS access key ID").await;
        setenv("AWS_ACCESS_KEY_ID", aws_access_key_id).await;
    }
    if args.aws_secret_access_key != "" {
        setenv("AWS_SECRET_ACCESS_KEY", args.aws_secret_access_key).await;
    }
    if getenv("AWS_SECRET_ACCESS_KEY").is_err() {
        let aws_secret_access_key = util::prompt("📦  AWS secret access key").await;
        setenv("AWS_SECRET_ACCESS_KEY", aws_secret_access_key).await;
    }
    if args.s3_bucket != "" {
        setenv("AWS_S3_BUCKET", args.s3_bucket).await;
    }

    let database: DBConnection = db::connect(getenv("DBFILE").unwrap().as_str());

    if getenv("STATUS_ONLY").unwrap() == "true" {
        db::report_status(&database);
        println!("✅  Exiting");
        process::exit(0)
    }

    let http: HTTPClient = http::new_client();

    if args.auth_only {
        auth::refresh_token(&http).await;
        println!("✅  Exiting");
        process::exit(0)
    }

    let aws: AWSClient = aws::new_client().await;

    if getenv("AWS_S3_BUCKET").is_err() {
        aws::choose_bucket(&aws, &database).await;
    }
    if args.aws_region != "" {
        setenv("AWS_REGION", args.aws_region).await;
    }
    if getenv("AWS_REGION").is_err() {
        // let aws_region = util::prompt("📦  AWS region");
        setenv("AWS_REGION", "us-east-1".to_string()).await;
    }
    (database, http, aws)
}

async fn reset() {
    println!("🗑️  Resetting database and temp files");
    db::reset(getenv("DBFILE").unwrap().as_str()).await;
    println!("🚮  Database reset");
    if dotenv::var("E2E").is_ok() && getenv("E2E").unwrap() == "true" {
        println!("🗑️  Resetting test bucket");
        crate::aws::_empty_test_bucket().await;
        println!("🚮  Test bucket reset");
    }
    localfs::reset().await;
    println!("🚮  Temp & env files deleted");
    print!("🎉 Reset complete\n\n");
}

async fn cleanup() {
    if getenv("E2E").unwrap() == "true" {
        localfs::delete_local_file(getenv("DBFILE").unwrap().as_str()).await;
        localfs::delete_local_file(getenv("ENV_FILE").unwrap().as_str()).await;
        println!("🚮  Test database and env file deleted");
    }
}
