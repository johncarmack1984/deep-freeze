use crate::{db, localfs, progress, util};
use db::DBConnection;
use deep_freeze::{
    TrackableBodyStream, MAX_CHUNKS, MAX_CHUNK_SIZE, MAX_UPLOAD_SIZE, MIN_CHUNK_SIZE,
};
use util::setenv;

use aws_config::{meta::region::RegionProviderChain, SdkConfig};
use aws_sdk_s3::{
    config::Region,
    error::SdkError,
    operation::{
        complete_multipart_upload::{CompleteMultipartUploadError, CompleteMultipartUploadOutput},
        create_multipart_upload::{CreateMultipartUploadError, CreateMultipartUploadOutput},
        delete_object::{DeleteObjectError, DeleteObjectOutput},
        get_object_attributes::GetObjectAttributesOutput,
        list_buckets::{ListBucketsError, ListBucketsOutput},
        put_object::{PutObjectError, PutObjectOutput},
        upload_part::{UploadPartError, UploadPartOutput},
    },
    types::{CompletedMultipartUpload, CompletedPart, ObjectAttributes, StorageClass},
    Client, Error,
};

use aws_sdk_secretsmanager::Client as SecretsClient;
use aws_smithy_http::byte_stream::{ByteStream, Length};
use indicatif::HumanBytes;
use std::path::PathBuf;

pub type AWSClient = Client;

pub async fn new_config() -> SdkConfig {
    let region_provider = RegionProviderChain::first_try(Region::new("us-east-1"))
        .or_default_provider()
        .or_else(Region::new("us-east-1"));
    aws_config::from_env().region(region_provider).load().await
}

pub async fn new_secrets_client() -> SecretsClient {
    let sdk_config: SdkConfig = new_config().await;
    SecretsClient::new(&sdk_config)
}

pub async fn get_app_secret() -> &'static str {
    let secrets = crate::aws::new_secrets_client().await;
    let resp = secrets
        .get_secret_value()
        .secret_id("DropboxAppSecret")
        .send()
        .await
        .unwrap();
    let secretstring = resp.secret_string().unwrap_or("No value!").to_string();
    let secretjson = crate::json::from_res(&secretstring);
    let app_secret = secretjson
        .get("dropbox_app_secret")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    crate::util::coerce_static_str(app_secret)
}

pub async fn new_client() -> Client {
    let sdk_config: SdkConfig = new_config().await;
    Client::new(&sdk_config)
}

pub async fn list_buckets(
    client: &Client,
) -> Result<ListBucketsOutput, SdkError<ListBucketsError>> {
    match client.list_buckets().send().await {
        Ok(res) => Ok(res),
        Err(err) => Err(err),
    }
}

pub async fn choose_bucket(client: &Client, sqlite: &DBConnection) {
    let buckets = list_buckets(&client).await.unwrap();
    let bucket_names = buckets
        .buckets
        .unwrap()
        .iter()
        .map(|b| b.name.as_ref().unwrap().to_string())
        .collect::<Vec<String>>();
    match inquire::Select::new(
        "🗄️  Which S3 Bucket shall we freeze your files in?",
        bucket_names,
    )
    .with_page_size(20)
    .prompt()
    {
        Ok(choice) => {
            println!("🗄️  You chose {choice}");
            setenv("AWS_S3_BUCKET", choice.to_string()).await;
            // let aws_region = get_bucket_region(&client, &choice).await.unwrap();
            // dbg!(&aws_region);
            // let aws_region = get_bucket_region(&aws, "font.vegify.app")
            //     .await
            //     .unwrap();
            // dbg!(&aws_region.location_constraint().unwrap().as_str());
            // dbg!(&aws_region);
            // let config = get_bucket_acceleration_config(client, choice)
            //     .await
            //     .unwrap();
            // if &config.status().unwrap().as_str() == &"Enabled" {
            //     println!("🚀  Acceleration enabled");
            //     setenv("AWS_S3_BUCKET_ACCELERATION", "true".to_string());
            // } else {
            //     setenv("AWS_S3_BUCKET_ACCELERATION", "false".to_string());
            // }
            db::insert_config(&sqlite);
        }
        Err(err) => panic!("❌  Error choosing folder {err}"),
    }
}

// pub async fn _get_bucket_acceleration_config(
//     client: &Client,
//     bucket: String,
// ) -> Result<GetBucketAccelerateConfigurationOutput, SdkError<GetBucketAccelerateConfigurationError>>
// {
//     match client
//         .get_bucket_accelerate_configuration()
//         .bucket(bucket)
//         .send()
//         .await
//     {
//         Ok(res) => Ok(res),
//         Err(err) => Err(err),
//     }
// }

// pub async fn _get_bucket_region(
//     client: &Client,
//     bucket: &str,
// ) -> Result<GetBucketLocationOutput, SdkError<GetBucketLocationError>> {
//     dbg!(&bucket);
//     match client
//         .get_bucket_location()
//         .bucket("vegify-dropbox-archive")
//         .send()
//         .await
//     {
//         Ok(res) => Ok(res),
//         Err(err) => Err(err),
//     }
// }

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

pub async fn create_multipart_upload(
    client: &Client,
    bucket: &str,
    key: &str,
) -> Result<CreateMultipartUploadOutput, SdkError<CreateMultipartUploadError>> {
    match client
        .create_multipart_upload()
        .bucket(bucket)
        .key(key)
        .storage_class(StorageClass::DeepArchive)
        .send()
        .await
    {
        Ok(res) => Ok(res),
        Err(err) => {
            dbg!(&err);
            println!("🚫  {err}");
            Err(err)
        }
    }
}

pub async fn upload_part(
    client: &Client,
    key: &str,
    bucket: &str,
    upload_id: &str,
    stream: ByteStream,
    part_number: i32,
) -> Result<UploadPartOutput, SdkError<UploadPartError>> {
    match client
        .upload_part()
        .key(key)
        .bucket(bucket)
        .upload_id(upload_id)
        .body(stream)
        .part_number(part_number)
        .send()
        .await
    {
        Ok(res) => Ok(res),
        Err(err) => {
            dbg!(&err);
            println!("🚫  {err}");
            Err(err)
        }
    }
}

async fn complete_multipart_upload(
    client: &Client,
    bucket: &str,
    key: &str,
    completed_multipart_upload: CompletedMultipartUpload,
    upload_id: &str,
) -> Result<CompleteMultipartUploadOutput, SdkError<CompleteMultipartUploadError>> {
    match client
        .complete_multipart_upload()
        .bucket(bucket)
        .key(key)
        .multipart_upload(completed_multipart_upload)
        .upload_id(upload_id)
        .send()
        .await
    {
        Ok(res) => Ok(res),
        Err(err) => Err(err),
    }
}

pub async fn multipart_upload(
    client: &Client,
    key: &str,
    local_path: &str,
    bucket: &str,
    m: &crate::progress::MultiProgress,
) -> Result<CompleteMultipartUploadOutput, SdkError<CompleteMultipartUploadError>> {
    let res = create_multipart_upload(client, bucket, key).await.unwrap();
    let upload_id = res.upload_id().unwrap();
    let mut upload_parts: Vec<CompletedPart> = Vec::new();

    let file_size = localfs::get_local_size(&local_path).await as u64;
    let (chunk_size, chunk_count, size_of_last_chunk) = chunk_math(file_size);

    let pb = m.add(progress::new(file_size, "file_transfer"));
    pb.set_prefix("⬆️  Upload  ");
    upload_parts = handle_multipart_chunks_upload(
        (chunk_size, chunk_count, size_of_last_chunk),
        (&key, &local_path, &bucket, &upload_id),
        (&client, &mut upload_parts),
        &pb,
    )
    .await;
    let completed_multipart_upload: CompletedMultipartUpload = CompletedMultipartUpload::builder()
        .set_parts(Some(upload_parts))
        .build();
    pb.set_prefix("⏳  Completing upload. ");
    match complete_multipart_upload(
        &client,
        &bucket,
        &key,
        completed_multipart_upload,
        &upload_id,
    )
    .await
    {
        Ok(res) => {
            pb.set_prefix("✅  Upload   ");
            pb.finish();
            Ok(res)
        }
        Err(err) => {
            dbg!(&err);
            println!("🚫  {err}");
            Err(err)
        }
    }
}

async fn handle_multipart_chunks_upload(
    (chunk_size, chunk_count, size_of_last_chunk): (u64, u64, u64),
    (key, local_path, bucket, upload_id): (&str, &str, &str, &str),
    (client, upload_parts): (&Client, &mut Vec<CompletedPart>),
    pb: &crate::progress::Progress,
) -> Vec<CompletedPart> {
    for chunk_index in 0..chunk_count {
        let this_chunk = if chunk_count - 1 == chunk_index {
            size_of_last_chunk
        } else {
            chunk_size
        };
        let uploaded = chunk_index * chunk_size;
        pb.set_prefix(format!("⬆️   Upload: Chunk {chunk_index}/{chunk_count} | ",));
        let stream = ByteStream::read_from()
            .path(local_path)
            .offset(uploaded)
            .length(Length::Exact(this_chunk))
            .build()
            .await
            .unwrap();
        //Chunk index needs to start at 0, but part numbers start at 1.
        let part_number = (chunk_index as i32) + 1;
        let upload_part_res = upload_part(&client, &key, &bucket, &upload_id, stream, part_number)
            .await
            .unwrap();

        upload_parts.push(
            CompletedPart::builder()
                .e_tag(upload_part_res.e_tag.unwrap_or_default())
                .part_number(part_number)
                .build(),
        );
        pb.set_position(uploaded + this_chunk);
    }
    upload_parts.to_owned()
}

pub async fn singlepart_upload(
    client: &Client,
    key: &str,
    local_path: &str,
    bucket: &str,
    m: &crate::progress::MultiProgress,
) -> Result<PutObjectOutput, SdkError<PutObjectError>> {
    let mut body = TrackableBodyStream::try_from(PathBuf::from(local_path))
        .map_err(|e| {
            panic!("Could not open sample file: {}", e);
        })
        .unwrap();
    let pb = m.add(crate::progress::new(
        body.content_length() as u64,
        "file_transfer",
    ));
    pb.set_prefix("⬆️   Upload   ");
    body.set_callback(move |tot_size: u64, sent: u64, cur_buf: u64| {
        pb.inc(cur_buf as u64);
        if sent == tot_size {
            pb.set_prefix("✅  Upload   ");
            pb.finish();
        }
    });
    // err in prod 7/4/2023 ~6pm
    // thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: DispatchFailure(DispatchFailure { source: ConnectorError { kind: Other(Some(TransientError)), source: hyper::Error(IncompleteMessage), connection: Unknown } })', src/aws.rs:176:10
    // note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
    match client
        .put_object()
        .storage_class(StorageClass::DeepArchive)
        .bucket(bucket)
        .key(key)
        .content_length(body.content_length())
        .body(body.to_s3_stream())
        .send()
        .await
    {
        Ok(res) => Ok(res),
        Err(err) => {
            dbg!(&err);
            println!("🚫  Upload failed");
            Err(err)
        }
    }
}

pub async fn upload_to_s3(
    client: &Client,
    key: &str,
    local_path: &str,
    bucket: &str,
    m: &crate::progress::MultiProgress,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    match localfs::get_local_size(&local_path).await {
        // 0 => panic!("file has no size"),
        size if size >= MAX_UPLOAD_SIZE as i64 => panic!("file is too big"),
        size if size < MAX_CHUNK_SIZE as i64 => {
            match singlepart_upload(&client, &key, &local_path, &bucket, &m).await {
                Ok(_) => Ok(()),
                Err(err) => {
                    println!("🚫  {err}");
                    // Err(SdkError::from(err))
                    // Err(PutObjectError::from(err).into())
                    Err(err.into())
                }
            }
        }
        _ => match multipart_upload(&client, &key, &local_path, &bucket, &m).await {
            Ok(_) => Ok(()),
            Err(err) => {
                println!("🚫  {err}");
                Err(err.into())
            }
        },
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
                HumanBytes(dropbox_size.try_into().unwrap()),
                HumanBytes(s3_size.try_into().unwrap())
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

pub fn calculate_chunk_count(file_size: u64, chunk_size: u64) -> (u64, u64) {
    let mut chunk_count = (file_size / chunk_size) + 1;
    let mut size_of_last_chunk = file_size % chunk_size;
    if size_of_last_chunk == 0 {
        size_of_last_chunk = chunk_size;
        chunk_count -= 1;
    }
    if chunk_count > MAX_CHUNKS {
        panic!("Too many chunks! Try increasing your chunk size.")
    }
    if chunk_count == 0 {
        panic!("No chunks! Try decreasing your chunk size.")
    }
    (chunk_count, size_of_last_chunk)
}

pub fn chunk_math(file_size: u64) -> (u64, u64, u64) {
    if file_size == 0 {
        panic!("Bad file size.");
    }
    let mut chunk_size = MIN_CHUNK_SIZE;
    while file_size / chunk_size > MAX_CHUNKS {
        chunk_size *= 2;
    }
    while chunk_size > MAX_CHUNK_SIZE {
        chunk_size -= 1000;
    }
    let (chunk_count, size_of_last_chunk) = calculate_chunk_count(file_size, chunk_size);
    (chunk_size, chunk_count, size_of_last_chunk)
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
        let local_size = crate::localfs::get_local_size(&local_path).await;
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
