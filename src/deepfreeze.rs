use crate::auth;
use crate::aws;
use crate::db::{self, DBConnection, Row};
use crate::dropbox;
use crate::http::HTTPClient;
use crate::localfs;
use crate::progress;
use crate::util;
use crate::util::getenv;
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
use futures_util::FutureExt;
use futures_util::__private::async_await;
use std::borrow::Borrow;
use std::env;
use std::ops::Deref;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::progress::{MultiProgress, ProgressStyle};
use console::{style, Emoji};
use indicatif::HumanDuration;
use rand::seq::SliceRandom;
use rand::Rng;

static MIGRATION_STEPS: &[&str] = &[
    "check migration status",
    "download from dropbox",
    "upload to s3",
];

pub async fn perform_migration(
    http: HTTPClient,
    database: DBConnection,
    aws: AWSClient,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    print!("\n🧊  Performing migration...\n\n\n\n");
    let m = progress::new_multi_progress();
    let started = Instant::now();
    let rows: Vec<_> = database
        .prepare("SELECT * FROM paths WHERE migrated < 1 AND skip < 1 ORDER BY dropbox_path ASC")
        .unwrap()
        .into_iter()
        .map(|row| async move {
            {
                let row = row.borrow();
                let dropbox_id = row
                    .as_ref()
                    .unwrap()
                    // .borrow()
                    .try_read::<&str, &str>("dropbox_id")
                    .unwrap()
                    .to_string();
                let filter = |i| i == dropbox_id;
                if env::var("SKIP")
                    .to_owned()
                    .unwrap_or("".to_string())
                    .split(',')
                    .collect::<Vec<&str>>()
                    .into_iter()
                    .any(filter)
                {
                    println!("✅ Skipping {dropbox_id}");
                    // db::set_skip(&database, &dropbox_id);
                } else {
                    if getenv("CHECK_ONLY") != "true" {
                        // auth::refresh_token(&http).await;
                    }
                    tokio::task::spawn(async move {
                        println!("📂  Migrating {dropbox_id}");
                        // dbg!(&http.to_owned());
                        // dbg!(&aws);
                        // dbg!(database);
                        // dbg!(database.change_count());
                        // dbg!(row);
                        // migrate_file_to_s3(&dropbox_id, &http, &aws, database, &m)
                        //     .await
                        //     .unwrap();
                        // row
                        // migrate_file_to_s3(row, &http, &aws, &sqlite, &m)
                        //     .await
                        //     .unwrap();
                    });
                }
            }
        })
        .collect();
    for row in rows {
        row.await;
    }
    m.clear().unwrap();
    db::report_status(&database);

    println!("✨ Done in {}", HumanDuration(started.elapsed()));
    if getenv("CHECK_ONLY") == "true" {
        println!("✅  Exiting");
        std::process::exit(0);
    }
    println!("");
    Ok(())
}

async fn migrate_file_to_s3(
    dropbox_id: &str,
    http: &HTTPClient,
    aws: &AWSClient,
    database: DBConnection,
    m: &MultiProgress,
) -> Result<(), Box<dyn std::error::Error>> {
    let row: db::Row = db::get_row(&database, &dropbox_id);
    let dropbox_id = row
        .try_read::<&str, &str>("dropbox_id")
        .unwrap()
        .to_string();

    match check_migration_status(&aws, &database, &row).await {
        0 => match getenv("CHECK_ONLY").as_str() {
            "true" => {
                print!("\n\n");
                return Ok(());
            }
            _ => (),
        },
        1 => return Ok(()),
        err => {
            dbg!("err");
            println!("❌  Unknown migration status {err}");
            db::set_skip(&database, &dropbox_id);
            return Ok(());
        }
    };

    let dropbox_path = row
        .try_read::<&str, &str>("dropbox_path")
        .unwrap()
        .to_string();
    let key = util::standardize_path(&dropbox_path);
    let bucket = env::var("AWS_S3_BUCKET").unwrap();

    // println!("📂  Migrating {key}");

    let local_path = format!("./temp/{key}");

    dropbox::download_from_dropbox(&http, &dropbox_id, &dropbox_path, &local_path, &m)
        .await
        .unwrap();

    // TODO verify checksum from DB

    match aws::upload_to_s3(&aws, &key, &local_path, &bucket, &m).await {
        Ok(_) => (),
        Err(err) => {
            println!("🚫  {err}");
            db::set_unmigrated(&database, &dropbox_id);
            // localfs::delete_local_file(&local_path);
            db::set_skip(&database, &dropbox_id);
        }
    }

    // TODO create checksum from file for AWS

    match aws::confirm_upload_size(&database, &aws, &bucket, &dropbox_id, &key).await {
        Ok(_) => (),
        Err(err) => {
            println!("🚫  {err}");
            db::set_unmigrated(&database, &dropbox_id);
            localfs::delete_local_file(&local_path);
            match aws::delete_from_s3(&aws, &bucket, &key).await {
                Ok(_) => println!("🗑️  Deleted s3://{bucket}/{key}"),
                Err(err) => println!("🚫  {err}"),
            };
        }
    }

    // // TODO verify checksum from S3

    db::set_migrated(&database, &dropbox_id);
    localfs::delete_local_file(&local_path);
    print!("\n\n");
    return Ok(());
}

async fn check_migration_status(aws: &AWSClient, database: &DBConnection, row: &Row) -> i64 {
    let dropbox_path = row
        .try_read::<&str, &str>("dropbox_path")
        .unwrap()
        .to_string();
    let bucket = env::var("AWS_S3_BUCKET").unwrap();
    let key = util::standardize_path(&dropbox_path);
    let dropbox_size = row.try_read::<i64, &str>("dropbox_size").unwrap();
    let dropbox_id = row
        .try_read::<&str, &str>("dropbox_id")
        .unwrap()
        .to_string();
    let local_path = format!("./temp/{key}");
    println!("🔍  Checking migration status for {}", dropbox_path);
    match aws::get_s3_attrs(&aws, &bucket, &key).await {
        Err(err) => match err {
            AWSError::NoSuchKey(_) => {
                println!("❌  Not found: s3:://{}/{}", bucket, key);
                db::set_unmigrated(&database, &dropbox_id);
                0
            }
            _ => panic!("❌  {}", err),
        },
        Ok(s3_attrs) => match s3_attrs.object_size() == dropbox_size {
            true => {
                println!("✅  Files the same size on DB & S3");
                db::set_migrated(&database, &dropbox_id);
                localfs::delete_local_file(&local_path);
                1
            }
            false => {
                println!("❌  File exists on S3, but is not the correct size");
                println!("🗳️  DB size: {dropbox_size}");
                println!("🗂️  S3 size: {}", s3_attrs.object_size());
                aws::delete_from_s3(&aws, &bucket, &key).await.unwrap();
                db::set_unmigrated(&database, &dropbox_id);
                0
            }
        },
    }
}
