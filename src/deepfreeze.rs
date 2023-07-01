use crate::aws;
use crate::db;
use crate::dropbox;
use crate::localfs;
use crate::util;
use aws_sdk_s3::operation::get_object_attributes::GetObjectAttributesOutput as S3Attrs;
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
// use inquire::Confirm;
// use pretty_bytes;
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
                println!("❌  Not found: s3:://{}/{}", s3_bucket, base_path);
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

#[async_recursion::async_recursion(?Send)]
async fn migrate_file_to_s3(
    row: &sqlite::Row,
    http: &reqwest::Client,
    aws: &AWSClient,
    sqlite: &sqlite::ConnectionWithFullMutex,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("");
    let mut migrated = row.try_read::<i64, &str>("migrated").unwrap();
    let dropbox_path = row.try_read::<&str, &str>("path").unwrap().to_string();
    if migrated.is_positive() {
        println!("✅ Already migrated: {dropbox_path}");
        return Ok(());
    }
    let size = row.try_read::<i64, &str>("size").unwrap();
    let base_path = util::standardize_path(&dropbox_path);
    let s3_bucket = env::var("S3_BUCKET").unwrap();
    if migrated.is_negative() {
        check_migration_status(
            &dropbox_path,
            &size,
            &base_path,
            &aws,
            &s3_bucket,
            &mut migrated,
            &sqlite,
        )
        .await?;
    }
    // match Confirm::new(&format!(
    //     "Migrate {base_path} ({}) to S3?",
    //     pretty_bytes::converter::convert(size as f64)
    // ))
    // .with_default(true)
    // .prompt()
    // {
    //     Ok(true) => println!("🚀  Starting migration"),
    //     Ok(false) => {
    //         println!("🚫  Skipping {dropbox_path}");
    //         return Ok(());
    //     }
    //     Err(err) => {
    //         println!("🚫  {err}");
    //         std::process::exit(0)
    //     }
    // }
    match migrated.abs() == 0 {
        true => {
            println!("📂  Migrating {base_path}");
            let local_path = format!("./temp/{base_path}");
            dropbox::download_from_db(&sqlite, &http, &dropbox_path, &local_path).await?;
            aws::upload_to_s3(&aws, &base_path, &local_path, &s3_bucket).await?;
            // TODO verify checksum from DB
            // TODO create checksum from file for AWS
            match aws::confirm_upload_size(&sqlite, &aws, &s3_bucket, &dropbox_path, &base_path)
                .await
            {
                Ok(_) => println!("✅ File uploaded to S3"),
                Err(err) => {
                    println!("🚫  {err}");
                    aws::delete_from_s3(&aws, &base_path, &s3_bucket)
                        .await
                        .unwrap();
                    localfs::delete_local_file(&local_path);
                    return migrate_file_to_s3(&row, &http, &aws, &sqlite).await;
                }
            }
            println!("✅ File uploaded to S3");
            // TODO verify checksum from S3
            db::set_migrated(&dropbox_path, &sqlite);
            localfs::delete_local_file(&local_path);
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
    // match Confirm::new(&format!(
    //     "Migrate {} from DropBox{} to s3:://{}?",
    //     db::get_pretty_unmigrated_size(&sqlite),
    //     env::var("BASE_FOLDER").unwrap(),
    //     env::var("S3_BUCKET").unwrap()
    // ))
    // .with_default(true)
    // .prompt()
    // {
    //     Ok(true) => println!("🚀  Starting migration"),
    //     Ok(false) => {
    //         println!("🚫  Migration cancelled");
    //         std::process::exit(0)
    //     }
    //     Err(err) => {
    //         println!("🚫  {err}");
    //         std::process::exit(0)
    //     }
    // }
    for row in sqlite
        .prepare("SELECT * FROM paths WHERE migrated < 1")
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
    {
        migrate_file_to_s3(&row, &http, &aws, &sqlite)
            .await
            .unwrap();
        let dropbox_path = row.try_read::<&str, &str>("path").unwrap().to_string();
        db::set_migrated(&dropbox_path, &sqlite);
    }
    println!("");
    println!("✅ Migration complete");
    Ok(())
}
