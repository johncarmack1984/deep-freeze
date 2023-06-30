use crate::aws;
use crate::db;
use crate::dropbox;
use crate::localfs;
use crate::util;
use aws_sdk_s3::operation::get_object_attributes::GetObjectAttributesOutput as S3Attrs;

use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
use inquire::Confirm;
use std::env;

async fn check_migration_status(
    dropbox_path: &str,
    dropbox_size: &i64,
    base_path: &String,
    aws_client: &AWSClient,
    s3_bucket: &String,
    migrated: &mut i64,
    sqlite_connection: &sqlite::ConnectionWithFullMutex,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📂  Checking migration status for {}", dropbox_path);
    let s3_attrs: Result<S3Attrs, AWSError> =
        aws::get_s3_attrs(&base_path, &aws_client, &s3_bucket).await;
    match s3_attrs {
        Err(err) => match err {
            AWSError::NoSuchKey(_) => {
                println!("❌  File not found in S3");
                db::set_unmigrated(&dropbox_path, &sqlite_connection);
                *migrated = 0;
            }
            _ => panic!("❌  {}", err),
        },
        Ok(s3_attrs) => match s3_attrs.object_size() == dropbox_size.to_owned() {
            true => {
                println!("✅ Files the same size on DB & S3");
                db::set_migrated(&dropbox_path, &sqlite_connection);
                *migrated = 1;
            }
            false => {
                println!("❌ File not the same size on S3 as DB");
                println!("🗳️  DB size: {dropbox_size}");
                println!("🗂️  S3 size: {}", s3_attrs.object_size());
                db::set_unmigrated(&dropbox_path, &sqlite_connection);
                *migrated = 0;
            }
        },
    }
    Ok(())
}

async fn migrate_file_to_s3(
    aws_client: &AWSClient,
    migrated: &mut i64,
    dropbox_path: &str,
    size: &i64,
    sqlite_connection: &sqlite::ConnectionWithFullMutex,
) -> Result<(), Box<dyn std::error::Error>> {
    match Confirm::new(&format!(
        "This will migrate DropBox folder {} to s3:://{}. Proceed?",
        env::var("BASE_FOLDER").unwrap(),
        env::var("S3_BUCKET").unwrap()
    ))
    .with_default(true)
    .prompt()
    {
        Ok(true) => println!("🚀  Starting migration"),
        Ok(false) => {
            println!("🚫  Migration cancelled");
            std::process::exit(0)
        }
        Err(err) => {
            println!("🚫  {err}");
            std::process::exit(0)
        }
    }
    if migrated.is_positive() {
        println!("✅ File already migrated");
        return Ok(());
    }
    let base_path = util::standardize_path(&dropbox_path);
    let s3_bucket = env::var("S3_BUCKET").unwrap();
    if migrated.is_negative() {
        check_migration_status(
            &dropbox_path,
            &size,
            &base_path,
            &aws_client,
            &s3_bucket,
            migrated,
            &sqlite_connection,
        )
        .await?;
    }
    match migrated.abs() == 0 {
        true => {
            println!("📂  Migrating {base_path}");
            let local_path = format!("./temp/{base_path}");
            localfs::create_download_folder(&dropbox_path, &local_path);
            dropbox::download_from_db(&dropbox_path, &local_path).await?;
            // verify file size (refactor from below)
            // TODO verify checksum from DB
            // TODO create checksum from file for AWS
            aws::upload_to_s3(&aws_client, &base_path, &local_path, &s3_bucket).await?;
            println!("✅ File uploaded to S3");
            // TODO verify checksum from S3
            db::set_migrated(&dropbox_path, &sqlite_connection);
            *migrated = 1;
            // std::fs::remove_file(&local_path).unwrap();
            Ok(())
        }
        false => Ok(()),
    }
}

pub async fn perform_migration(
    sqlite_connection: &sqlite::ConnectionWithFullMutex,
    aws_client: &AWSClient,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    match Confirm::new(" Perform migration?")
        .with_default(true)
        .prompt()
    {
        Ok(true) => println!("🚀 Performing migration"),
        Ok(false) => {
            println!("🚫  Migration cancelled");
            std::process::exit(0);
        }
        Err(err) => panic!("🚫  {err}"),
    }
    let rows = db::get_unmigrated_rows(&sqlite_connection);
    for row in rows {
        let mut migrated = row.try_read::<i64, &str>("migrated").unwrap();
        let dropbox_path = row.try_read::<&str, &str>("path").unwrap().to_string();
        let size = row.try_read::<i64, &str>("size").unwrap();
        let aws_client = aws_client.clone();
        match migrate_file_to_s3(
            &aws_client,
            &mut migrated,
            &dropbox_path,
            &size,
            &sqlite_connection,
        )
        .await
        {
            Ok(_) => {}
            Err(err) => {
                println!("{}", err);
            }
        };
    }
    println!("✅ Migration complete");
    Ok(())
}
