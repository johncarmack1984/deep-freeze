use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::operation::get_object_attributes::GetObjectAttributesOutput;
use aws_sdk_s3::types::{ObjectAttributes, StorageClass};
use aws_sdk_s3::{Client as AWSClient, Error as AWSError};
// use aws_sdk_s3::primitives::ByteStream;
// use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, ObjectAttributes, StorageClass};
// use aws_smithy_http::byte_stream::Length;
// use indicatif::{ProgressBar, ProgressStyle};

// const MIN_CHUNK_SIZE: u64 = 5000000;
// const MAX_CHUNK_SIZE: u64 = 5000000000;
// const MAX_CHUNKS: u64 = 10000;

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

pub async fn upload_to_s3(
    aws_client: &AWSClient,
    s3_path: &str,
    local_path: &str,
    s3_bucket: &str,
) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    println!("📂  Uploading to s3://{}/{}", s3_bucket, s3_path);
    println!("📂  Uploading from {}", local_path);
    // let res = aws_client
    aws_client
        .create_multipart_upload()
        .bucket(s3_bucket)
        .key(s3_path)
        .storage_class(StorageClass::DeepArchive)
        .send()
        .await
        .unwrap();
    // let upload_id = res.upload_id().unwrap();

    // let path = Path::new(local_path);
    // let file_size = tokio::fs::metadata(path)
    //     .await
    //     .expect("it exists I swear")
    //     .len();

    // let mut chunk_size = MIN_CHUNK_SIZE;

    // while file_size / chunk_size > MAX_CHUNKS {
    //     chunk_size *= 2;
    // }

    // while chunk_size > MAX_CHUNK_SIZE {
    //     chunk_size -= 1000;
    // }

    // let mut chunk_count = (file_size / chunk_size) + 1;
    // let mut size_of_last_chunk = file_size % chunk_size;
    // if size_of_last_chunk == 0 {
    //     size_of_last_chunk = chunk_size;
    //     chunk_count -= 1;
    // }

    // if file_size == 0 {
    //     panic!("Bad file size.");
    // }
    // if chunk_count > MAX_CHUNKS {
    //     panic!("Too many chunks! Try increasing your chunk size.")
    // }

    // let mut upload_parts: Vec<CompletedPart> = Vec::new();

    // println!("⬆️  Uploading {} chunks.", chunk_count);

    // let pb = ProgressBar::new(file_size);
    // pb.set_style(ProgressStyle::default_bar()
    //     .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
    //     .unwrap()
    //     .progress_chars("█  "));
    // let msg = format!("⬆️  Uploading {} to {}", s3_path, s3_bucket);
    // pb.set_message(msg);

    // for chunk_index in 0..chunk_count {
    //     let this_chunk = if chunk_count - 1 == chunk_index {
    //         size_of_last_chunk
    //     } else {
    //         chunk_size
    //     };
    //     let uploaded = chunk_index * chunk_size;
    //     pb.set_message(format!(
    //         "⬆️  Uploading chunk {} of {}.",
    //         chunk_index + 1,
    //         chunk_count
    //     ));
    //     let stream = ByteStream::read_from()
    //         .path(Path::new(local_path))
    //         .offset(uploaded)
    //         .length(Length::Exact(this_chunk))
    //         .build()
    //         .await
    //         .unwrap();
    //     //Chunk index needs to start at 0, but part numbers start at 1.
    //     let part_number = (chunk_index as i32) + 1;
    //     let upload_part_res = aws_client
    //         .upload_part()
    //         .key(s3_path)
    //         .bucket(s3_bucket)
    //         .upload_id(upload_id)
    //         .body(stream)
    //         // .body(stream.to_multipart_s3_stream())
    //         .part_number(part_number)
    //         .send()
    //         .await?;
    //     upload_parts.push(
    //         CompletedPart::builder()
    //             .e_tag(upload_part_res.e_tag.unwrap_or_default())
    //             .part_number(part_number)
    //             .build(),
    //     );
    //     pb.set_position(uploaded + this_chunk);
    // }
    // pb.finish_with_message("🎉  All chunks uploaded.");
    // let completed_multipart_upload: CompletedMultipartUpload = CompletedMultipartUpload::builder()
    //     .set_parts(Some(upload_parts))
    //     .build();
    // println!("⏳  Completing upload.");
    // let _complete_multipart_upload_res = aws_client
    //     .complete_multipart_upload()
    //     .bucket(s3_bucket)
    //     .key(s3_path)
    //     .multipart_upload(completed_multipart_upload)
    //     .upload_id(upload_id)
    //     .send()
    //     .await
    //     .unwrap();
    // println!("✅ Done uploading file.");

    Ok(())
}
