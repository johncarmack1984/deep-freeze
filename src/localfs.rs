use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

pub fn local_file_exists(local_path: &str) -> bool {
    let path = Path::new(local_path);
    path.exists()
}

pub fn create_local_file(local_path: &str) -> File {
    create_download_folder(&local_path);
    match File::create(&local_path) {
        Ok(file) => file,
        Err(e) => panic!("❌  Failed to create file '{}': {}", local_path, e),
    }
}

pub fn create_download_folder(local_path: &str) -> () {
    let path = Path::new(&local_path);
    let prefix = path.parent().unwrap();
    fs::create_dir_all(prefix).unwrap();
}

pub fn get_local_size(local_path: &str) -> i64 {
    let path = Path::new(local_path);
    let file_size = if local_file_exists(&local_path) {
        fs::metadata(path).unwrap().len()
    } else {
        0
    };
    file_size.try_into().unwrap()
}

pub fn delete_local_file(local_path: &str) -> () {
    fs::remove_file(&local_path).unwrap();
}

pub fn reset() {
    let temp = Path::new("temp");
    if temp.exists() {
        fs::remove_dir_all(temp).unwrap();
    }
    fs::create_dir_all(temp).unwrap()
}

pub fn _create_test_file(key: &str, bytes: u64) -> File {
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    let mut file = create_local_file(&key);
    // let pb = crate::progress::new(bytes);
    // let msg = format!("Creating sample file.");
    // pb.set_message(msg);
    // pb.set_position(0);
    while file.metadata().unwrap().len() < bytes {
        let position = file.metadata().unwrap().len();
        let rand_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take((bytes - position - 1) as usize)
            .map(char::from)
            .collect();
        let return_string: String = "\n".to_string();
        file.write_all(rand_string.as_ref())
            .expect("Error writing to file.");
        file.write_all(return_string.as_ref())
            .expect("Error writing to file.");
        // let position = file.metadata().unwrap().len();
        // let msg = format!("{} of {} bytes written.", position, bytes);
        // pb.set_message(msg);
        // pb.set_position(position);
    }
    // let msg = format!("{} of {} bytes written.", bytes, bytes);
    // pb.finish_with_message(msg);
    File::open(key).unwrap()
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn it_creates_a_local_1_kb_file() {
        let key = "test/1-kb-generated.txt";
        let bytes = 1024;
        let file: std::fs::File;
        if crate::localfs::local_file_exists(&key.to_string()) {
            crate::localfs::delete_local_file(&key);
        }
        file = crate::localfs::_create_test_file(&key, bytes);
        let file_size = file.metadata().unwrap().len();
        assert_eq!(file_size, bytes);
    }
    #[test]
    #[ignore]
    fn it_creates_a_local_5_mb_file() {
        let key = "test/5-MiB-generated.txt";
        let bytes = 5 * 1024 * 1024;
        let file: std::fs::File;
        if crate::localfs::local_file_exists(&key.to_string()) {
            crate::localfs::delete_local_file(&key);
        }
        file = crate::localfs::_create_test_file(&key, bytes);
        let file_size = file.metadata().unwrap().len();
        assert_eq!(file_size, bytes);
    }

    #[test]
    #[ignore]
    fn it_creates_five_1_mb_test_files() {
        let bytes = 1 * 1024 * 1024;
        for i in 1..6 {
            let file: std::fs::File;
            let key = format!("test/1-MiB-generated-{i}.txt");
            if crate::localfs::local_file_exists(&key.to_string()) {
                crate::localfs::delete_local_file(&key);
            }
            file = crate::localfs::_create_test_file(&key, bytes);
            let file_size = file.metadata().unwrap().len();
            assert_eq!(file_size, bytes);
        }
    }
}
