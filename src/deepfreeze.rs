use crate::aws;
use crate::db;
use crate::db::DBConnection;
use crate::db::DBRow;
use crate::dropbox;
use crate::localfs;
use crate::progress;
use crate::util;
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
// use indicatif::MultiProgress;
// use indicatif::ProgressStyle;
use std::env;
// use std::sync::Arc;
// use std::thread;
// use std::time::{Duration, Instant};
// use std::time::Instant;

// use console::{style, Emoji};
// use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
// use rand::seq::SliceRandom;
// use rand::Rng;

// static PACKAGES: &[&str] = &[
//     "fs-events",
//     "my-awesome-module",
//     "emoji-speaker",
//     "wrap-ansi",
//     "stream-browserify",
//     "acorn-dynamic-import",
// ];

// static COMMANDS: &[&str] = &[
//     "cmake .",
//     "make",
//     "make clean",
//     "gcc foo.c -o foo",
//     "gcc bar.c -o bar",
//     "./helper.sh rebuild-cache",
//     "make all-clean",
//     "make test",
// ];

// static LOOKING_GLASS: Emoji<'_, '_> = Emoji("🔍  ", "");
// static TRUCK: Emoji<'_, '_> = Emoji("🚚  ", "");
// static CLIP: Emoji<'_, '_> = Emoji("🔗  ", "");
// static PAPER: Emoji<'_, '_> = Emoji("📃  ", "");
// static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", ":-)");

pub async fn perform_migration(
    http: reqwest::Client,
    sqlite: sqlite::ConnectionWithFullMutex,
    aws: AWSClient,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    print!("\n\n");
    // let mut rng = rand::thread_rng();
    // let started = Instant::now();
    // let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
    //     .unwrap()
    //     .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
    // println!(
    //     "{} {}Resolving packages...",
    //     style("[1/4]").bold().dim(),
    //     LOOKING_GLASS
    // );
    // println!(
    //     "{} {}Fetching packages...",
    //     style("[2/4]").bold().dim(),
    //     TRUCK
    // );

    // println!(
    //     "{} {}Linking dependencies...",
    //     style("[3/4]").bold().dim(),
    //     CLIP
    // );
    // pb.tick();
    let m = progress::new_multi_progress();
    // let total_rows = db::count_rows(&sqlite);
    // let mut pb_macro = m.add(progress::new(total_rows as u64));
    // pb_macro = progress::set_style_migration_progress_units(pb_macro);
    // pb_macro.set_position(db::count_migrated(&sqlite) as u64);

    for row in sqlite
        .prepare("SELECT * FROM paths WHERE migrated < 1")
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
    {
        // let mut pb_local = m.add(progress::new(4));
        // pb_local = progress::set_style_migration_progress_units(pb_local);
        let dropbox_id = row
            .try_read::<&str, &str>("dropbox_id")
            .unwrap()
            .to_string();
        // pb_local.set_prefix(format!("[{}/{}]", count + 1, total_rows));
        let filter = |&i| i == dropbox_id;
        if env::var("SKIP_ARRAY")
            .unwrap_or("".to_string())
            .split(',')
            .collect::<Vec<&str>>()
            .iter()
            .any(filter)
        {
            // pb_macro.set_message(format!("✅ Skipping {dropbox_id}"));
            println!("✅ Skipping {dropbox_id}");
            continue;
        } else {
            // pb_macro.set_message(format!("📂  Migrating {dropbox_id}"));
            migrate_file_to_s3(row, &http, &aws, &sqlite, &m)
                .await
                .unwrap();
        }
        // pb_macro.inc(1);
    }
    // pb_macro.finish_with_message("done");
    println!("");
    Ok(())
}

// #[async_recursion::async_recursion(?Send)]
async fn migrate_file_to_s3(
    row: sqlite::Row,
    http: &reqwest::Client,
    aws: &AWSClient,
    sqlite: &sqlite::ConnectionWithFullMutex,
    m: &crate::progress::MultiProgress,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("");
    let dropbox_id = row
        .try_read::<&str, &str>("dropbox_id")
        .unwrap()
        .to_string();

    match check_migration_status(&aws, &sqlite, &row).await {
        0 => println!("❌  Not migrated"),
        1 => {
            println!("✅ Already migrated");
            return Ok(());
        }
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
    let bucket = env::var("S3_BUCKET").unwrap();

    println!("📂  Migrating {key}");

    let local_path = format!("./temp/{key}");

    dropbox::download_from_dropbox(&http, &dropbox_id, &dropbox_path, &local_path, &m)
        .await
        .unwrap();

    aws::upload_to_s3(&aws, &key, &local_path, &bucket, &m)
        .await
        .unwrap();

    // TODO verify checksum from DB
    // TODO create checksum from file for AWS

    match aws::confirm_upload_size(&sqlite, &aws, &bucket, &dropbox_id, &key).await {
        Ok(_) => println!("✅ File uploaded to S3"),
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
    return Ok(());
}

async fn check_migration_status(aws: &AWSClient, sqlite: &DBConnection, row: &DBRow) -> i64 {
    let dropbox_path = row
        .try_read::<&str, &str>("dropbox_path")
        .unwrap()
        .to_string();
    let bucket = env::var("S3_BUCKET").unwrap();
    let key = util::standardize_path(&dropbox_path);
    let dropbox_size = row.try_read::<i64, &str>("dropbox_size").unwrap();
    let dropbox_id = row
        .try_read::<&str, &str>("dropbox_id")
        .unwrap()
        .to_string();
    println!("📂  Checking migration status for {}", dropbox_path);
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
                println!("✅ Files the same size on DB & S3");
                db::set_migrated(&sqlite, &dropbox_id);
                1
            }
            false => {
                println!("❌ File exists on S3, but is not the correct size");
                println!("🗳️  DB size: {dropbox_size}");
                println!("🗂️  S3 size: {}", s3_attrs.object_size());
                aws::delete_from_s3(&aws, &bucket, &key).await.unwrap();
                db::set_unmigrated(&sqlite, &dropbox_id);
                0
            }
        },
    }
}
