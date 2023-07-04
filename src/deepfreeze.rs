use crate::aws;
use crate::db;
use crate::db::DBConnection;
use crate::db::DBRow;
use crate::dropbox;
use crate::localfs;
use crate::progress;
use crate::util;
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
use futures_util::FutureExt;
use futures_util::__private::async_await;
use std::env;
use std::ops::Deref;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use console::{style, Emoji};
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use rand::seq::SliceRandom;
use rand::Rng;

static MIGRATION_STEPS: &[&str] = &[
    "check migration status",
    "download from dropbox",
    "upload to s3",
];

pub async fn perform_migration(
    http: reqwest::Client,
    sqlite: sqlite::ConnectionWithFullMutex,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let aws = aws::new_client().await;
    let started = Instant::now();
    // print!("\n\n");
    let m = progress::new_multi_progress();
    let rows: Vec<_> = sqlite
        .prepare("SELECT * FROM paths WHERE migrated < 1 AND skip = 0 ORDER BY dropbox_path")
        // .as_deref_mut()
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .map(|row| {
            let aws = aws.clone();
            // let mut sqlite = sqlite;
            // let mut row = row;
            // dbg!(&row);
            let dropbox_id = row.read::<&str, &str>("dropbox_id");
            let size = row.read::<i64, &str>("dropbox_size") as u64;
            let mut pb = m.add(progress::new(size, "file_transfer"));
            let key = row
                .try_read::<&str, &str>("dropbox_path")
                .unwrap()
                .to_string();
            tokio::task::spawn(async move {
                let step = MIGRATION_STEPS[0];
                pb.inc(1);
                pb.set_message(format!("📂  {key}: {step}"));
                // pb.`
                // [pb] = migrate_file_to_s3(pb).await;
                let migrated: i64;
                // *sqlite;
                (pb, migrated) = check_migration_status(pb, &aws).await;
                // (pb, migrated) = check_migration_status(pb, &aws, sqlite).await;
                // [pb] = migrate_file_to_s3(pb).await;
                // migrate_file_to_s3(sqlite).await;
                // pb.set_position(size / 2);
                // migrate_file_to_s3(row, &http, &aws, &sqlite, &m)
                //     .await
                //     .unwrap();
                // pb.set_position(size);
                // pb.finish_with_message("Migration complete");
            })
        })
        .collect();
    for row in rows {
        row.await;
    }
    m.clear().unwrap();
    println!("✨ Done in {}", HumanDuration(started.elapsed()));
    println!("");
    Ok(())
}

// #[async_await::async_recursion(?Send)]
// #[async_recursion::async_recursion(?Send)]
async fn migrate_file_to_s3(
    pb: ProgressBar,
    // sqlite: sqlite::ConnectionWithFullMutex,
    // row: &DBRow,
    // http: &reqwest::Client,
    // aws: &AWSClient,
    // m: &crate::progress::MultiProgress,
    // ) -> Result<(), Box<dyn std::error::Error>> {
) -> [ProgressBar; 1] {
    // ) {
    println!("");
    // pb.set_message("📂  Checking migration status");
    // dbg!(&row);
    // let dropbox_id = row
    //     .try_read::<&str, &str>("dropbox_id")
    //     .unwrap()
    //     .to_string();

    // match check_migration_status(&aws, &sqlite, &row).await {
    //     0 => println!("❌  Not migrated"),
    //     1 => {
    //         println!("✅ Already migrated");
    //         return Ok(());
    //     }
    //     err => {
    //         dbg!("err");
    //         println!("❌  Unknown migration status {err}");
    //         db::set_skip(&sqlite, &dropbox_id);
    //         return Ok(());
    //     }
    // };

    // let dropbox_path = row
    //     .try_read::<&str, &str>("dropbox_path")
    //     .unwrap()
    //     .to_string();
    // let key = util::standardize_path(&dropbox_path);
    // let bucket = env::var("S3_BUCKET").unwrap();

    // println!("📂  Migrating {key}");

    // let local_path = format!("./temp/{key}");

    // dropbox::download_from_dropbox(&http, &dropbox_id, &dropbox_path, &local_path, &m)
    //     .await
    //     .unwrap();

    // aws::upload_to_s3(&aws, &key, &local_path, &bucket, &m)
    //     .await
    //     .unwrap();

    // // TODO verify checksum from DB
    // // TODO create checksum from file for AWS

    // match aws::confirm_upload_size(&sqlite, &aws, &bucket, &dropbox_id, &key).await {
    //     Ok(_) => println!("✅ File uploaded to S3"),
    //     Err(err) => {
    //         println!("🚫  {err}");
    //         db::set_unmigrated(&sqlite, &dropbox_id);
    //         localfs::delete_local_file(&local_path);
    //         match aws::delete_from_s3(&aws, &bucket, &key).await {
    //             Ok(_) => println!("🗑️  Deleted s3://{bucket}/{key}"),
    //             Err(err) => println!("🚫  {err}"),
    //         };
    //     }
    // }

    // // TODO verify checksum from S3

    // db::set_migrated(&sqlite, &dropbox_id);
    // return Ok(());
    // pb.finish_with_message("Migration status checked");
    [pb]
}

// async fn check_migration_status(aws: &AWSClient, sqlite: &DBConnection, row: &DBRow) -> i64 {
async fn check_migration_status(
    pb: ProgressBar,
    aws: &AWSClient,
    // sqlite: &DBConnection,
) -> (ProgressBar, i64) {
    // println!("📂  Checking migration status for file");
    // let dropbox_path = row
    //     .try_read::<&str, &str>("dropbox_path")
    //     .unwrap()
    //     .to_string();
    // let bucket = env::var("S3_BUCKET").unwrap();
    // let key = util::standardize_path(&dropbox_path);
    // let dropbox_size = row.try_read::<i64, &str>("dropbox_size").unwrap();
    // let dropbox_id = row
    //     .try_read::<&str, &str>("dropbox_id")
    //     .unwrap()
    //     .to_string();
    // match aws::get_s3_attrs(&aws, &bucket, &key).await {
    //     Err(err) => match err {
    //         AWSError::NoSuchKey(_) => {
    //             println!("❌  Not found: s3:://{}/{}", bucket, key);
    //             db::set_unmigrated(&sqlite, &dropbox_id);
    //             0
    //         }
    //         _ => panic!("❌  {}", err),
    //     },
    //     Ok(s3_attrs) => match s3_attrs.object_size() == dropbox_size {
    //         true => {
    //             println!("✅ Files the same size on DB & S3");
    //             db::set_migrated(&sqlite, &dropbox_id);
    //             1
    //         }
    //         false => {
    //             println!("❌ File exists on S3, but is not the correct size");
    //             println!("🗳️  DB size: {dropbox_size}");
    //             println!("🗂️  S3 size: {}", s3_attrs.object_size());
    //             aws::delete_from_s3(&aws, &bucket, &key).await.unwrap();
    //             db::set_unmigrated(&sqlite, &dropbox_id);
    //             0
    //         }
    //     },
    // }
    (pb, 0)
}

// .collect::<Vec<sqlite::Row>>();
// for row in sqlite
//     .prepare("SELECT * FROM paths WHERE migrated < 1")
//     .unwrap()
//     .into_iter()
//     .map(|row| row.unwrap())
// {
// filter for skip here
//     let filter = |&i| i == dropbox_id;
//     if env::var("SKIP_ARRAY")
//         .unwrap_or("".to_string())
//         .split(',')
//         .collect::<Vec<&str>>()
//         .iter()
//         .any(filter)
//     {
//         println!("✅ Skipping {dropbox_id}");
//         continue;
//     } else {
//         migrate_file_to_s3(row, &http, &aws, &sqlite, &m)
//             .await
//             .unwrap();
//     }
// }
