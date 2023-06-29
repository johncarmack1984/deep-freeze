use crate::aws;
use crate::dropbox;
use crate::util;

use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
use sedregex::find_and_replace;
use std::env;

pub async fn migrate_to_s3(
    aws_client: &AWSClient,
    migrated: &mut i64,
    dropbox_path: &str,
    size: &i64,
    sqlite_connection: &sqlite::ConnectionWithFullMutex,
) -> Result<(), std::io::Error> {
    if migrated.is_positive() {
        println!("✅ File already migrated");
        return Ok(());
    }
    // let base_name = Path::new(&dropbox_path)
    //     .file_name()
    //     .unwrap()
    //     .to_str()
    //     .unwrap();
    let base_folder = env::var("BASE_FOLDER").unwrap();
    let mut base_path = find_and_replace(
        &dropbox_path.clone().to_owned(),
        &[format!("s/\\{}\\///g", base_folder)],
    )
    .unwrap()
    .to_string();
    base_path = util::standardize_path(base_path);
    let s3_bucket = env::var("S3_BUCKET").unwrap();
    if migrated.is_negative() {
        println!("📂  Checking migration status for {}", dropbox_path);
        let db_size = size.clone();
        match aws::get_s3_attrs(&base_path, &aws_client, &s3_bucket).await {
            Ok(s3_attrs) => {
                if s3_attrs.object_size() == db_size {
                    println!("✅ File already migrated");
                    let statement = format!(
                        "UPDATE paths SET migrated = 1 WHERE path = '{}';",
                        dropbox_path.clone()
                    );
                    match sqlite_connection.execute(statement.clone()) {
                        Ok(_) => {
                            println!("📁 File list updated");
                            *migrated = 1;
                        }
                        Err(err) => {
                            println!("❌  Error in statement: {}", statement);
                            println!("❌  Database Could not be Updated: {}", statement);
                            panic!("{}", err);
                        }
                    }
                    return Ok(());
                } else {
                    println!("❌ File not the same size on S3 as DB");
                    let statement = format!(
                        "UPDATE paths SET migrated = 0 WHERE path = '{}';",
                        dropbox_path.clone()
                    );
                    match sqlite_connection.execute(statement.clone()) {
                        Ok(_) => {
                            *migrated = 0;
                            println!("📁 File list updated");
                        }
                        Err(err) => {
                            println!("❌  Error in statement: {}", statement);
                            panic!("{}", err);
                        }
                    }
                    // TODO download_from_db();
                }
            }
            Err(err) => match err {
                AWSError::NoSuchKey(_) => {
                    println!("❌  File not found in S3");

                    let statement = format!(
                        "UPDATE paths SET migrated = 0 WHERE path = '{}';",
                        dropbox_path.clone()
                    );
                    match sqlite_connection.execute(statement.clone()) {
                        Ok(_) => {
                            *migrated = 0;
                            println!("📁 File list updated");
                        }
                        Err(err) => {
                            println!("❌  Error in statement: {}", statement);
                            panic!("{}", err);
                        }
                    }
                    return Ok(());
                }
                _ => {
                    panic!("❌  Error in S3 request: {}", err);
                }
            },
        }
    }
    match migrated.abs() == 0 {
        true => {
            let local_path = format!("./temp/{base_path}");
            // let local_dir = find_and_replace(&local_path, &[format!("s/{}//g", base_name)])
            //     .unwrap()
            //     .to_string();
            // if !std::path::Path::new(&local_dir).exists() {
            //     let _dir = fs::create_dir_all(&local_dir)?;
            // }
            println!("📂  Migrating {base_path}");
            let _file = dropbox::download_from_db(&dropbox_path, &local_path)
                .await
                .unwrap();
            // verify file size (refactor from below)
            // TODO verify checksum from DB
            // TODO create checksum from file for AWS
            // TODO upload to S3
            match aws::upload_to_s3(&aws_client, &base_path, &local_path, &s3_bucket)
                .await
                .unwrap()
            {
                () => {
                    println!("✅ File uploaded to S3");
                    // std::fs::remove_file(&local_path).unwrap();
                    let statement = format!(
                        "UPDATE paths SET migrated = 1 WHERE path = '{}';",
                        dropbox_path.clone()
                    );
                    match sqlite_connection.execute(statement.clone()) {
                        Ok(_) => {
                            *migrated = 1;
                            println!("📁 File list updated");
                        }
                        Err(err) => {
                            println!("❌  Error in statement: {}", statement);
                            panic!("{}", err);
                        }
                    }
                } // TODO verify checksum from S3
                  // update migration status
                  // update file list
            }
            Ok(())
        }
        false => Ok(()),
    }
}

pub async fn perform_migration(
    sqlite_connection: &sqlite::ConnectionWithFullMutex,
    aws_client: &AWSClient,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    println!("🗃️  Performing migration...");
    let query = "SELECT * FROM paths WHERE migrated < 1";
    let rows = sqlite_connection
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>();
    // let semaphore = Arc::new(Semaphore::new(1)); // Limit to 10 concurrent downloads
    // let mut tasks = Vec::new();
    for row in rows {
        let mut migrated = row.try_read::<i64, &str>("migrated").unwrap();
        let dropbox_path = row.try_read::<&str, &str>("path").unwrap().to_string();
        let size = row.try_read::<i64, &str>("size").unwrap();
        let aws_client = aws_client.clone();
        // let sem_clone = Arc::clone(&semaphore);
        // let task = tokio::spawn(async move {
        // let permit = sem_clone.acquire().await.unwrap();
        // sqlite_connection.clone();
        match migrate_to_s3(
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
        // drop(permit); // Release the semaphore
        // });
        // tasks.push(task);
    }
    // for task in tasks {
    //     task.await.unwrap();
    // }
    println!("✅✅✅  Migration complete");
    Ok(())
}
