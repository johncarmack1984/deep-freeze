use std::{fs::File, path::Path};

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
    let path = std::path::Path::new(&local_path);
    let prefix = path.parent().unwrap();
    std::fs::create_dir_all(prefix).unwrap();
}

pub fn get_local_size(local_path: &str) -> i64 {
    let path = Path::new(local_path);
    let file_size = if local_file_exists(&local_path) {
        std::fs::metadata(path).unwrap().len()
    } else {
        0
    };
    file_size.try_into().unwrap()
}

pub fn delete_local_file(local_path: &str) -> () {
    std::fs::remove_file(&local_path).unwrap();
}
