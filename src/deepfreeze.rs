use crate::auth;
use crate::aws;
use crate::db;
use crate::db::DBConnection;
use crate::db::DBRow;
use crate::db::DBValue;
use crate::dropbox;
use crate::localfs;
use crate::progress;
use crate::util;
use crate::util::getenv;
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
use indicatif::HumanDuration;
use sqlite::ConnectionWithFullMutex;
use sqlite::Value;
use std::borrow::Borrow;
use std::env;
use std::time::Instant;

pub async fn perform_migration(
    http: reqwest::Client,
    sqlite: DBConnection,
    aws: AWSClient,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    print!("\n🧊  Performing migration...\n\n\n\n");
    let started = Instant::now();

    let total_count: u64 = db::count_rows(&sqlite).try_into().unwrap();
    let unmigrated_count: u64 = db::count_unmigrated(&sqlite).try_into().unwrap();
    let m = progress::new_multi_progress();

    let migration_progress = m.add(progress::new(
        total_count - unmigrated_count,
        "migration_progress",
    ));

    for row in sqlite
        .prepare("SELECT * FROM paths WHERE migrated < 1 AND skip < 1 ORDER BY dropbox_path ASC")
        .unwrap()
        .into_iter()
        .map(|row| async move {
            // let row = row.borrow().clone().to_owned();
            // dbg!(&row);
            // let _dropbox_id = row.read::<&str, _>("dropbox_id");
            // let _dropbox_path = row.read::<String, _>("dropbox_path").unwrap();
            // let _dropbox_size = row.read::<String, _>("dropbox_size").unwrap();
            // let _dropbox_hash = row.read::<String, _>("dropbox_hash").unwrap();
            // let _migrated = row.read::<String, _>("migrated").unwrap();
            tokio::task::spawn(async move {
                // print!("{:?}\n\n\n", row);
                // migrate_file_to_s3(row, &http, &aws, &sqlite, &m);
                //         // dbg!(&aws);
                //         // dbg!(http);
                //         // db::get_dropbox_size(&sqlite, &dropbox_id);
                //         println!("📂  Migrating {_dropbox_path}");
            });
        })
    {
        row.await;
    }

    // for row in rows {
    //     dbg!(&row);
    // }
    // let sqlite: &'static DBConnection = &sqlite;
    // // while let Ok(sqlite::State::Row) = statement.next() {
    //     // for row in db::get_rows
    //     // for row in rows.
    // {
    //     // let row = statement.read::<_, _>("row").unwrap();
    //     // dbg!(&statement);
    //     // dbg!(&http);
    //     // dbg!(&aws);
    //     // let http = http.clone();
    //     // let aws = aws.clone();

    //     migration_progress.inc(1);
    //     migrate_file_to_s3(statement, &http, &aws, &sqlite, &m).await?;
    //     // let row = statement.read::<DBRow>().unwrap();
    // }

    // .collect();

    // dbg!(statement);

    // for row in rows {
    //     migrate_file_to_s3(row.unwrap(), &http, &aws, &sqlite, &m).await?;
    // }
    // print!("{:?}", rows);

    // while let Some(row) = rows.next() {
    // dbg!(row);
    // migrate_file_to_s3(row, &http, &aws, &sqlite, &m).await?;
    // }

    // for row in rows {
    //     migrate_file_to_s3(row, &http, &aws, &sqlite, &m).await?;
    // }

    // {
    //     if getenv("CHECK_ONLY") == "false" {
    //         auth::refresh_token(&http).await;
    //     }
    //     migrate_file_to_s3(row, &http, &aws, &sqlite, &m)
    //         .await
    //         .unwrap();
    // }
    // db::report_status(&sqlite);

    println!("✨ Done in {}", HumanDuration(started.elapsed()));
    // if getenv("CHECK_ONLY") == "true" {
    //     println!("✅  Exiting");
    //     std::process::exit(0);
    // }
    print!("\n\n\n");
    Ok(())
}

async fn migrate_file_to_s3(
    row: sqlite::Row,
    http: &reqwest::Client,
    aws: &AWSClient,
    sqlite: &sqlite::ConnectionWithFullMutex,
    m: &crate::progress::MultiProgress,
) -> Result<(), Box<dyn std::error::Error>> {
    let dropbox_id = row
        .try_read::<&str, &str>("dropbox_id")
        .unwrap()
        .to_string();

    match check_migration_status(&aws, &sqlite, &row).await {
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
            db::set_skip(&sqlite, &dropbox_id);
            return Ok(());
        }
    };

    let dropbox_path = row
        .try_read::<&str, &str>("dropbox_path")
        .unwrap()
        .to_string();
    let key = util::standardize_path(&dropbox_path);
    let bucket = env::var("AWS_S3_BUCKET").unwrap();

    println!("📂  Migrating {key}");

    let local_path = format!("./temp/{key}");

    dropbox::download_from_dropbox(&http, &dropbox_id, &dropbox_path, &local_path, &m)
        .await
        .unwrap();

    // TODO verify checksum from DB

    match aws::upload_to_s3(&aws, &key, &local_path, &bucket, &m).await {
        Ok(_) => (),
        Err(err) => {
            println!("🚫  {err}");
            db::set_unmigrated(&sqlite, &dropbox_id);
            // localfs::delete_local_file(&local_path);
            db::set_skip(&sqlite, &dropbox_id);
        }
    }

    // TODO create checksum from file for AWS

    match aws::confirm_upload_size(&sqlite, &aws, &bucket, &dropbox_id, &key).await {
        Ok(_) => (),
        Err(err) => {
            println!("🚫  {err}");
            db::set_unmigrated(&sqlite, &dropbox_id);
            localfs::delete_local_file(&local_path);
            match aws::delete_from_s3(&aws, &bucket, &key).await {
                Ok(_) => println!("🗑️  Deleted s3://{bucket}/{key}"),
                Err(err) => println!("🚫  {err}"),
            };
        }
    }

    // TODO verify checksum from S3

    db::set_migrated(&sqlite, &dropbox_id);
    localfs::delete_local_file(&local_path);
    print!("\n\n");
    return Ok(());
}

async fn check_migration_status(aws: &AWSClient, sqlite: &DBConnection, row: &DBRow) -> i64 {
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
                db::set_unmigrated(&sqlite, &dropbox_id);
                0
            }
            _ => panic!("❌  {}", err),
        },
        Ok(s3_attrs) => match s3_attrs.object_size() == dropbox_size {
            true => {
                println!("✅  Files the same size on DB & S3");
                db::set_migrated(&sqlite, &dropbox_id);
                localfs::delete_local_file(&local_path);
                1
            }
            false => {
                println!("❌  File exists on S3, but is not the correct size");
                println!("🗳️  DB size: {dropbox_size}");
                println!("🗂️  S3 size: {}", s3_attrs.object_size());
                aws::delete_from_s3(&aws, &bucket, &key).await.unwrap();
                db::set_unmigrated(&sqlite, &dropbox_id);
                0
            }
        },
    }
}
