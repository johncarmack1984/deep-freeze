use crate::{db, localfs};
use aws_config::meta::region::RegionProviderChain;
use aws_config::SdkConfig;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::operation::delete_object::{DeleteObjectError, DeleteObjectOutput};
use aws_sdk_s3::operation::get_object_attributes::GetObjectAttributesOutput;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, ObjectAttributes, StorageClass};
use aws_sdk_s3::{Client, Error};
use aws_smithy_http::byte_stream::{ByteStream, Length};
use aws_smithy_http::result::SdkError;
use deep_freeze::{
    TrackableBodyStream, MAX_CHUNKS, MAX_CHUNK_SIZE, MAX_UPLOAD_SIZE, MIN_CHUNK_SIZE,
};
use std::path::{Path, PathBuf};
use std::result::Result;

pub type AWSClient = Client;

pub async fn new_client() -> Client {
    let region_provider = RegionProviderChain::first_try(Region::new("us-east-1"))
        .or_default_provider()
        .or_else(Region::new("us-east-1"));
    let sdk_config: SdkConfig = aws_config::from_env().region(region_provider).load().await;
    Client::new(&sdk_config)
}

pub async fn get_s3_attrs(
    client: &Client,
    bucket: &str,
    key: &String,
) -> Result<GetObjectAttributesOutput, Error> {
    let res = client
        .get_object_attributes()
        .bucket(bucket)
        .key(key)
        .object_attributes(ObjectAttributes::ObjectSize)
        .send()
        .await?;
    Ok::<GetObjectAttributesOutput, Error>(res)
}

pub async fn multipart_upload(
    client: &Client,
    key: &str,
    local_path: &str,
    bucket: &str,
    m: &crate::progress::MultiProgress,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let res = client
        .create_multipart_upload()
        .bucket(bucket)
        .key(key)
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
    let pb = m.add(crate::progress::new(file_size, "file_transfer"));
    let msg = format!("⬆️  Uploading {} to {}", key, bucket);
    pb.set_message(msg);
    for chunk_index in 0..chunk_count {
        let this_chunk = if chunk_count - 1 == chunk_index {
            size_of_last_chunk
        } else {
            chunk_size
        };
        let uploaded = chunk_index * chunk_size;
        let percent = (uploaded as f64 / file_size as f64) * 100.0;
        pb.set_message(format!(
            "⬆️  {percent:.1}% uploaded. Chunk {chunk_index} of {chunk_count}",
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
        let upload_part_res = client
            .upload_part()
            .key(key)
            .bucket(bucket)
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
    pb.set_message("⬆️  All chunks uploaded.");
    let completed_multipart_upload: CompletedMultipartUpload = CompletedMultipartUpload::builder()
        .set_parts(Some(upload_parts))
        .build();
    pb.set_message("⏳  Completing upload.");
    let _complete_multipart_upload_res = client
        .complete_multipart_upload()
        .bucket(bucket)
        .key(key)
        .multipart_upload(completed_multipart_upload)
        .upload_id(upload_id)
        .send()
        .await
        .unwrap();
    pb.finish_with_message("✅ Done uploading file.");
    // pb.finish_and_clear();
    // println!("✅ Done uploading file.");
    Ok(())
}

pub async fn singlepart_upload(
    client: &Client,
    key: &str,
    local_path: &str,
    bucket: &str,
    m: &crate::progress::MultiProgress,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let mut body = TrackableBodyStream::try_from(PathBuf::from(local_path))
        .map_err(|e| {
            panic!("Could not open sample file: {}", e);
        })
        .unwrap();
    let pb = crate::progress::new(body.content_length() as u64, "file_transfer");
    pb.set_prefix("⬆️  Upload   ");
    body.set_callback(move |tot_size: u64, sent: u64, cur_buf: u64| {
        let percent = (sent as f64 / tot_size as f64) * 100.0;
        let msg = format!("⬆️  {:.1}% uploaded.", percent);
        // pb.set_message(msg);
        pb.inc(cur_buf as u64);
        if sent == tot_size {
            pb.set_prefix("✅  Upload   ");
            pb.finish();
        }
    });
    client
        .put_object()
        .storage_class(StorageClass::DeepArchive)
        .bucket(bucket)
        .key(key)
        .content_length(body.content_length())
        .body(body.to_s3_stream())
        .send()
        .await
        .unwrap();
    Ok(())
}

pub async fn upload_to_s3(
    client: &Client,
    key: &str,
    local_path: &str,
    bucket: &str,
    m: &crate::progress::MultiProgress,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    match localfs::get_local_size(&local_path) {
        // 0 => panic!("file has no size"),
        size if size >= MAX_UPLOAD_SIZE as i64 => panic!("file is too big"),
        size if size < MAX_CHUNK_SIZE as i64 => {
            singlepart_upload(&client, &key, &local_path, &bucket, &m).await
        }
        _ => multipart_upload(&client, &key, &local_path, &bucket, &m).await,
    }
}

pub async fn confirm_upload_size(
    sqlite: &sqlite::ConnectionWithFullMutex,
    aws: &Client,
    bucket: &str,
    dropbox_id: &str,
    key: &String,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let s3_attrs: GetObjectAttributesOutput = get_s3_attrs(&aws, &bucket, &key).await?;
    let s3_size = s3_attrs.object_size();
    let dropbox_size = db::get_dropbox_size(&sqlite, &dropbox_id);
    match s3_size == dropbox_size {
        true => return Ok(()),
        false => {
            return Err(format!(
                "DropBox file size {} does not match S3 {}",
                pretty_bytes::converter::convert(dropbox_size as f64),
                pretty_bytes::converter::convert(s3_size as f64)
            )
            .into());
        }
    }
}

pub async fn delete_from_s3(
    client: &Client,
    bucket: &str,
    key: &str,
) -> Result<DeleteObjectOutput, SdkError<DeleteObjectError>> {
    match client.delete_object().bucket(bucket).key(key).send().await {
        Ok(res) => {
            println!("🗑️  Deleted s3://{}/{}", bucket, key);
            Ok(res)
        }
        Err(err) => Err(err),
    }
}

pub async fn _empty_test_bucket() {
    println!("🗑️  Emptying test bucket");
    let aws = new_client().await;
    let bucket = "deep-freeze-test".to_string();
    let mut objects = aws
        .list_objects_v2()
        .bucket(&bucket)
        .send()
        .await
        .unwrap()
        .contents
        .unwrap_or(Vec::new());
    if objects.len() == 0 {
        return;
    }
    while objects.len() > 0 {
        for object in objects {
            let key = object.key.unwrap();
            delete_from_s3(&aws, &bucket, &key).await.unwrap();
        }
        objects = aws
            .list_objects_v2()
            .bucket(&bucket)
            .send()
            .await
            .unwrap()
            .contents
            .unwrap_or(Vec::new());
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use crate::progress;
    const BUCKET: &str = "deep-freeze-test";

    #[tokio::test]
    async fn it_uploads_to_s3() {
        dotenv::dotenv().ok();
        env::set_var("SILENT", "true");
        let aws = crate::aws::new_client().await;
        let key = String::from("test-s3-upload.txt");
        let local_path = String::from(format!("./test/{key}"));

        assert_eq!(
            true,
            crate::aws::upload_to_s3(
                &aws,
                &key,
                &local_path,
                &BUCKET,
                &&progress::new_multi_progress(),
            )
            .await
            .is_ok(),
            "🚫  file upload unsuccessful"
        );
        assert!(crate::aws::delete_from_s3(&aws, &BUCKET, &key)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn it_gets_s3_attrs() {
        dotenv::dotenv().ok();
        env::set_var("SILENT", "true");
        let aws = crate::aws::new_client().await;
        let key = String::from("test-s3-get-attrs.txt");
        let local_path = String::from(format!("./test/{key}"));
        let local_size = crate::localfs::get_local_size(&local_path);
        crate::aws::upload_to_s3(
            &aws,
            &key,
            &local_path,
            &BUCKET,
            &&progress::new_multi_progress(),
        )
        .await
        .unwrap();
        let attrs = crate::aws::get_s3_attrs(&aws, &BUCKET, &key).await.unwrap();
        assert_eq!(attrs.object_size(), local_size, "🚫  sizes don't match");
        assert!(
            crate::aws::delete_from_s3(&aws, &BUCKET, &key)
                .await
                .is_ok(),
            "🚫  file deletion unsuccessful"
        );
    }

    #[tokio::test]
    async fn it_deletes_from_s3() {
        dotenv::dotenv().ok();
        env::set_var("SILENT", "true");
        let aws = crate::aws::new_client().await;
        let key = String::from("test-s3-delete.txt");
        let local_path = String::from(format!("./test/{key}"));
        crate::aws::upload_to_s3(
            &aws,
            &key,
            &local_path,
            &BUCKET,
            &&progress::new_multi_progress(),
        )
        .await
        .unwrap();
        assert_eq!(
            true,
            crate::aws::delete_from_s3(&aws, "deep-freeze-test", "test-s3-delete.txt")
                .await
                .is_ok(),
            "🚫  file deletion unsuccessful",
        );
    }
}
