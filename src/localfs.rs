use sedregex::find_and_replace;
use std::{
    fs::{self, File},
    path::Path,
};

pub fn local_file_exists(local_path: &str) -> bool {
    let path = Path::new(local_path);
    path.exists()
}

pub fn local_path_is_dir(local_path: &str) -> bool {
    std::fs::metadata(local_path).unwrap().is_dir()
}

pub fn local_folder_exists(local_path: &str) -> bool {
    let path = Path::new(local_path);
    path.is_dir()
}

pub fn create_local_file(local_path: &str) -> File {
    File::create(local_path)
        .or(Err(format!("❌  Failed to create file '{local_path}'")))
        .unwrap()
}

pub fn create_download_folder(dropbox_path: &str, local_path: &str) -> () {
    let base_name = Path::new(&dropbox_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let local_dir = find_and_replace(&local_path, &[format!("s/{}//g", base_name)])
        .unwrap()
        .to_string();
    if !local_folder_exists(&local_dir) {
        fs::create_dir_all(&local_dir).unwrap()
    }
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

// pub fn get_local_checksum(local_path: &str) -> String {
//     let path = Path::new(local_path);
//     let file_size = std::fs::metadata(path).expect("it exists I swear").len();
//     file_size.try_into().unwrap()
// }

pub fn delete_local_file(local_path: &str) -> () {
    std::fs::remove_file(&local_path).unwrap();
}

pub fn delete_local_dir(local_path: &str) -> () {
    std::fs::remove_dir(&local_path).unwrap()
}
