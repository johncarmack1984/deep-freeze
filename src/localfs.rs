use sedregex::find_and_replace;
use std::{
    fs::{self, File},
    path::Path,
};

pub fn local_file_exists(local_path: &str) -> bool {
    let path = Path::new(local_path);
    path.exists()
}

pub fn local_folder_exists(local_path: &str) -> bool {
    let path = Path::new(local_path);
    path.is_dir()
}

pub fn create_local_file(dropbox_path: &str, local_path: &str) -> File {
    create_download_folder(dropbox_path, local_path);
    match File::create(&local_path) {
        Ok(file) => file,
        Err(e) => panic!("❌  Failed to create file '{}': {}", local_path, e),
    }
}

pub fn create_download_folder(dropbox_path: &str, local_path: &str) -> () {
    let local_dir = find_and_replace(&local_path, &[format!("s/{}//g", dropbox_path)])
        .unwrap()
        .to_string();
    println!("local_dir: {}", local_dir);
    if !local_folder_exists(&local_dir) {
        fs::create_dir_all(&local_dir).unwrap()
    }
    // }
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
