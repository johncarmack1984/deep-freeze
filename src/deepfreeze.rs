use crate::aws;
use crate::db;
use crate::db::DBConnection;
use crate::db::DBRow;
use crate::dropbox;
use crate::http::HTTPClient;
use crate::localfs;
use crate::util;
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
use std::env;

async fn check_migration_status(
    _http: &HTTPClient,
    aws: &AWSClient,
    sqlite: &DBConnection,
    row: &DBRow,
) -> Result<(), Box<dyn std::error::Error>> {
    let dropbox_path = row
        .try_read::<&str, &str>("dropbox_path")
        .unwrap()
        .to_string();
    println!("📂  Checking migration status for {}", dropbox_path);

    let bucket = env::var("S3_BUCKET").unwrap();
    let key = util::standardize_path(&dropbox_path);
    let dropbox_size = row.try_read::<i64, &str>("dropbox_size").unwrap();
    let dropbox_id = row
        .try_read::<&str, &str>("dropbox_id")
        .unwrap()
        .to_string();
    match aws::get_s3_attrs(&aws, &bucket, &key).await {
        Err(err) => match err {
            AWSError::NoSuchKey(_) => {
                println!("❌  Not found: s3:://{}/{}", bucket, key);
                db::set_unmigrated(&sqlite, &dropbox_id);
            }
            _ => panic!("❌  {}", err),
        },
        Ok(s3_attrs) => match s3_attrs.object_size() == dropbox_size.to_owned() {
            true => {
                println!("✅ Files the same size on DB & S3");
                db::set_migrated(&sqlite, &dropbox_id);
            }
            false => {
                println!("❌ File exists on S3, but is not the correct size");
                println!("🗳️  DB size: {dropbox_size}");
                println!("🗂️  S3 size: {}", s3_attrs.object_size());
                aws::delete_from_s3(&aws, &bucket, &key).await?;
                db::set_unmigrated(&sqlite, &dropbox_id);
            }
        },
    }
    Ok(())
}

#[async_recursion::async_recursion(?Send)]
async fn migrate_file_to_s3(
    row: sqlite::Row,
    http: &reqwest::Client,
    aws: &AWSClient,
    sqlite: &sqlite::ConnectionWithFullMutex,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("");
    let migrated = row.try_read::<i64, &str>("migrated").unwrap();
    let dropbox_id = row
        .try_read::<&str, &str>("dropbox_id")
        .unwrap()
        .to_string();
    let dropbox_path = row
        .try_read::<&str, &str>("dropbox_path")
        .unwrap()
        .to_string();

    if migrated.is_positive() {
        println!("✅ Already migrated: {dropbox_path}");
        return Ok(());
    }

    let key = util::standardize_path(&dropbox_path);
    let bucket = env::var("S3_BUCKET").unwrap();
    if migrated.is_negative() {
        check_migration_status(&http, &aws, &sqlite, &row).await?;
    }
    match migrated.abs() == 0 {
        true => {
            println!("📂  Migrating {key}");
            let local_path = format!("./temp/{key}");
            dropbox::download_from_dropbox(&http, &dropbox_id, &dropbox_path, &local_path).await?;
            aws::upload_to_s3(&aws, &key, &local_path, &bucket).await?;
            // TODO verify checksum from DB
            // TODO create checksum from file for AWS
            match aws::confirm_upload_size(&sqlite, &aws, &bucket, &dropbox_path, &key).await {
                Ok(_) => println!("✅ File uploaded to S3"),
                Err(err) => {
                    println!("🚫  {err}");
                    db::set_unmigrated(&sqlite, &dropbox_path);
                    localfs::delete_local_file(&local_path);
                    match aws::delete_from_s3(&aws, &bucket, &key).await {
                        Ok(_) => println!("🗑️  Deleted s3://{bucket}/{key}"),
                        Err(err) => println!("🚫  {err}"),
                    };
                }
            }
            // TODO verify checksum from S3
            db::set_migrated(&sqlite, &dropbox_path);
            Ok(())
        }
        false => Ok(()),
    }
}

pub async fn perform_migration(
    http: reqwest::Client,
    sqlite: sqlite::ConnectionWithFullMutex,
    aws: AWSClient,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    print!("\n\n");
    for row in sqlite
        .prepare("SELECT * FROM paths WHERE migrated < 1")
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
    {
        migrate_file_to_s3(row, &http, &aws, &sqlite).await.unwrap();
    }
    println!("");
    println!("✅ Migration complete");
    Ok(())
}
