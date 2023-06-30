use crate::{db, localfs};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::operation::get_object_attributes::GetObjectAttributesOutput;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, ObjectAttributes, StorageClass};
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
use aws_smithy_http::byte_stream::{ByteStream, Length};
use deep_freeze::TrackableBodyStream;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

const MIN_CHUNK_SIZE: u64 = 5242880; // 5 MiB in bytes
const MAX_CHUNK_SIZE: u64 = 5368709120; // 5 GiB in bytes
const MAX_UPLOAD_SIZE: u64 = 5497558138880; // 5 TiB in bytes
const MAX_CHUNKS: u64 = 10000;

pub async fn new_client() -> AWSClient {
    let region_provider = RegionProviderChain::first_try(Region::new("us-east-1"))
        .or_default_provider()
        .or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    AWSClient::new(&config)
}

pub async fn get_s3_attrs(
    base_path: &String,
    client: &AWSClient,
    bucket: &str,
) -> Result<GetObjectAttributesOutput, AWSError> {
    let res = client
        .get_object_attributes()
        .bucket(bucket)
        .key(base_path)
        .object_attributes(ObjectAttributes::ObjectSize)
        .send()
        .await?;

    Ok::<GetObjectAttributesOutput, AWSError>(res)
}

pub async fn multipart_upload(
    aws_client: &AWSClient,
    s3_path: &str,
    local_path: &str,
    s3_bucket: &str,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let res = aws_client
        .create_multipart_upload()
        .bucket(s3_bucket)
        .key(s3_path)
        .storage_class(StorageClass::DeepArchive)
        .send()
        .await
        .unwrap();
    let upload_id = res.upload_id().unwrap();
    let path = Path::new(local_path);
    let file_size = tokio::fs::metadata(path)
        .await
        .expect("it exists I swear")
        .len();
    let mut chunk_size = MIN_CHUNK_SIZE;
    while file_size / chunk_size > MAX_CHUNKS {
        chunk_size *= 2;
    }
    while chunk_size > MAX_CHUNK_SIZE {
        chunk_size -= 1000;
    }
    let mut chunk_count = (file_size / chunk_size) + 1;
    let mut size_of_last_chunk = file_size % chunk_size;
    if size_of_last_chunk == 0 {
        size_of_last_chunk = chunk_size;
        chunk_count -= 1;
    }
    if file_size == 0 {
        panic!("Bad file size.");
    }
    if chunk_count > MAX_CHUNKS {
        panic!("Too many chunks! Try increasing your chunk size.")
    }
    let mut upload_parts: Vec<CompletedPart> = Vec::new();
    let pb = ProgressBar::new(file_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
    let msg = format!("⬆️  Uploading {} to {}", s3_path, s3_bucket);
    pb.set_message(msg);
    for chunk_index in 0..chunk_count {
        let this_chunk = if chunk_count - 1 == chunk_index {
            size_of_last_chunk
        } else {
            chunk_size
        };
        let uploaded = chunk_index * chunk_size;
        pb.set_message(format!(
            "⬆️  Uploading chunk {} of {}.",
            chunk_index + 1,
            chunk_count
        ));
        let stream = ByteStream::read_from()
            .path(Path::new(local_path))
            .offset(uploaded)
            .length(Length::Exact(this_chunk))
            .build()
            .await
            .unwrap();
        //Chunk index needs to start at 0, but part numbers start at 1.
        let part_number = (chunk_index as i32) + 1;
        let upload_part_res = aws_client
            .upload_part()
            .key(s3_path)
            .bucket(s3_bucket)
            .upload_id(upload_id)
            .body(stream)
            .part_number(part_number)
            .send()
            .await?;
        upload_parts.push(
            CompletedPart::builder()
                .e_tag(upload_part_res.e_tag.unwrap_or_default())
                .part_number(part_number)
                .build(),
        );
        pb.set_position(uploaded + this_chunk);
    }
    pb.finish_with_message("⬆️  All chunks uploaded.");
    let completed_multipart_upload: CompletedMultipartUpload = CompletedMultipartUpload::builder()
        .set_parts(Some(upload_parts))
        .build();
    println!("⏳  Completing upload.");
    let _complete_multipart_upload_res = aws_client
        .complete_multipart_upload()
        .bucket(s3_bucket)
        .key(s3_path)
        .multipart_upload(completed_multipart_upload)
        .upload_id(upload_id)
        .send()
        .await
        .unwrap();
    println!("✅ Done uploading file.");
    Ok(())
}

pub async fn singlepart_upload(
    aws_client: &AWSClient,
    s3_path: &str,
    local_path: &str,
    s3_bucket: &str,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let mut body = TrackableBodyStream::try_from(PathBuf::from(local_path))
        .map_err(|e| {
            panic!("Could not open sample file: {}", e);
        })
        .unwrap();
    let pb = ProgressBar::new(body.content_length() as u64);
    body.set_callback(move |tot_size: u64, sent: u64, cur_buf: u64| {
        pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("█  "));
        let percent = (sent as f64 / tot_size as f64) * 100.0;
        let msg = format!("⬆️  {:.1}% uploaded.", percent);
        pb.set_message(msg);
        pb.inc(cur_buf as u64);
        if sent == tot_size {
                pb.set_message(format!("⬆️  Finished uploading"));
                pb.finish();
            }
    });

    let _upload_res = aws_client
        .put_object()
        .storage_class(StorageClass::DeepArchive)
        .bucket(s3_bucket)
        .key(s3_path)
        .content_length(body.content_length())
        .body(body.to_s3_stream())
        .send()
        .await?;
    Ok(())
}

pub async fn upload_to_s3(
    aws_client: &AWSClient,
    s3_path: &str,
    local_path: &str,
    s3_bucket: &str,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    match localfs::get_local_size(&local_path) {
        0 => panic!("file has no size"),
        size if size >= MAX_UPLOAD_SIZE as i64 => panic!("file is too big"),
        size if size < MAX_CHUNK_SIZE as i64 => {
            singlepart_upload(&aws_client, &s3_path, &local_path, &s3_bucket).await
        }
        _ => multipart_upload(&aws_client, &s3_path, &local_path, &s3_bucket).await,
    }
}

pub async fn confirm_upload_size(
    connection: &sqlite::ConnectionWithFullMutex,
    aws_client: &AWSClient,
    s3_bucket: &str,
    dropbox_path: &str,
    base_path: &String,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let s3_attrs: Result<GetObjectAttributesOutput, AWSError> =
        get_s3_attrs(&base_path, &aws_client, &s3_bucket).await;
    let s3_size = s3_attrs.unwrap().object_size();
    let dropbox_size = db::get_dropbox_size(&connection, &dropbox_path);
    match s3_size == dropbox_size {
        true => (),
        false => panic!("🚫  DropBox file size {dropbox_size} does not match S3 {s3_size}"),
    }
    Ok(())
}
