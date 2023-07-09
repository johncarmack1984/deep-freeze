use crate::util::getenv;
use std::path::Path;

use tokio::fs::{self, File, OpenOptions};
use tokio::io::{self, AsyncWriteExt};

pub async fn update_env_file(key: &str, value: String) -> io::Result<()> {
    let env_filename = getenv("ENV_FILE").unwrap();
    let env_temp_filename = format!("{env_filename}.temp", env_filename = &env_filename);
    let mut currentenv = match local_file_exists(&env_filename).await {
        true => fs::read_to_string(&env_filename).await?,
        false => "".to_string(),
    };
    let mut newenv: String;
    match currentenv.contains(key) {
        true => {
            newenv = currentenv
                .lines()
                .map(|line| match line.starts_with(format!("{key}=").as_str()) {
                    true => format!("{}=\"{}\"", key, value),
                    false => line.to_string(),
                })
                .collect::<Vec<String>>()
                .join("\n");
        }
        false => {
            currentenv.push_str(format!("{}=\"{}\"", key, value).as_str());
            newenv = currentenv;
        }
    }
    newenv.push_str("\n");
    let mut dst = File::create(&env_temp_filename).await?;
    dst.write_all(newenv.as_bytes()).await?;

    fs::rename(env_temp_filename, &env_filename).await?;
    dotenv::from_filename(env_filename).ok();
    assert_eq!(getenv(key).unwrap(), value);
    Ok(())
}

pub async fn delete_local_file(local_path: &str) {
    if local_file_exists(local_path).await {
        fs::remove_file(&local_path).await.unwrap();
    }
}

pub async fn local_file_exists(local_path: &str) -> bool {
    fs::try_exists(local_path).await.unwrap()
}

pub async fn reset() {
    let temp_path = getenv("TEMP_DIR").unwrap_or("temp".to_string());
    delete_local_dir(temp_path.as_str()).await;
    let env_path = getenv("ENV_FILE").unwrap_or(".env".to_string());
    delete_local_file(env_path.as_str()).await;
    fs::create_dir_all(temp_path).await.unwrap();
}

async fn delete_local_dir(local_path: &str) {
    fs::remove_dir_all(local_path).await.unwrap()
}

pub async fn get_local_file(local_path: &str) -> File {
    if local_file_exists(local_path).await {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(local_path)
            .await
            .unwrap()
    } else {
        create_download_folder(local_path).await;
        create_local_file(local_path).await
    }
}

pub async fn create_local_file(local_path: &str) -> File {
    match File::create(&local_path).await {
        Ok(file) => file,
        Err(e) => panic!("❌  Failed to create file '{}': {}", local_path, e),
    }
}

pub async fn create_download_folder(local_path: &str) -> () {
    let path = Path::new(&local_path);
    let prefix = path.parent().unwrap();
    fs::create_dir_all(prefix).await.unwrap();
}

pub async fn get_local_size(local_path: &str) -> i64 {
    let file_size = if local_file_exists(&local_path).await {
        fs::metadata(local_path).await.unwrap().len()
    } else {
        0
    };
    file_size.try_into().unwrap()
}

pub async fn _create_test_file(key: &str, bytes: i64) -> File {
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    let mut file = create_local_file(&key).await;
    // let pb = crate::progress::new(bytes);
    // let msg = format!("Creating sample file.");
    // pb.set_message(msg);
    // pb.set_position(0);
    while get_local_size(key).await < bytes {
        let position = get_local_size(key).await;
        let rand_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take((bytes - position - 1) as usize)
            .map(char::from)
            .collect();
        let return_string: String = "\n".to_string();
        file.write_all(rand_string.as_ref())
            .await
            .expect("Error writing to file.");
        file.write_all(return_string.as_ref())
            .await
            .expect("Error writing to file.");
        // let position = file.metadata().unwrap().len();
        // let msg = format!("{} of {} bytes written.", position, bytes);
        // pb.set_message(msg);
        // pb.set_position(position);
    }
    // let msg = format!("{} of {} bytes written.", bytes, bytes);
    // pb.finish_with_message(msg);
    File::open(key).await.unwrap()
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn it_creates_a_local_1_kb_file() {
        let key = "test/1-kb-generated.txt";
        let bytes = 1024;
        let file: tokio::fs::File;
        if crate::localfs::local_file_exists(&key.to_string()).await {
            crate::localfs::delete_local_file(&key).await;
        }
        file = crate::localfs::_create_test_file(&key, bytes).await;
        let file_size: i64 = file.metadata().await.unwrap().len() as i64;
        assert_eq!(file_size, bytes);
    }
    #[tokio::test]
    #[ignore]
    async fn it_creates_a_local_5_mb_file() {
        let key = "test/5-MiB-generated.txt";
        let bytes = 5 * 1024 * 1024;
        let file: tokio::fs::File;
        if crate::localfs::local_file_exists(&key.to_string()).await {
            crate::localfs::delete_local_file(&key).await;
        }
        file = crate::localfs::_create_test_file(&key, bytes).await;
        let file_size = file.metadata().await.unwrap().len() as i64;
        assert_eq!(file_size, bytes);
    }

    #[tokio::test]
    #[ignore]
    async fn it_creates_five_1_mb_test_files() {
        let bytes = 1 * 1024 * 1024;
        for i in 1..6 {
            let file: tokio::fs::File;
            let key = format!("test/1-MiB-generated-{i}.txt");
            if crate::localfs::local_file_exists(&key.to_string()).await {
                crate::localfs::delete_local_file(&key).await;
            }
            file = crate::localfs::_create_test_file(&key, bytes).await;
            let file_size = file.metadata().await.unwrap().len() as i64;
            assert_eq!(file_size, bytes);
        }
    }
}
