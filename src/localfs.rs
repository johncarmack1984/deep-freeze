use crate::db;
use sedregex::find_and_replace;
use std::{fs, path::Path};

pub fn create_download_folder(dropbox_path: &str, local_path: &str) -> () {
    let base_name = Path::new(&dropbox_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let local_dir = find_and_replace(&local_path, &[format!("s/{}//g", base_name)])
        .unwrap()
        .to_string();
    if !std::path::Path::new(&local_dir).exists() {
        fs::create_dir_all(&local_dir).unwrap()
    }
}

pub fn get_local_size(local_path: &str) -> i64 {
    let path = Path::new(local_path);
    let file_size = std::fs::metadata(path).expect("it exists I swear").len();
    file_size.try_into().unwrap()
}

pub fn confirm_local_size(
    connection: &sqlite::ConnectionWithFullMutex,
    dropbox_path: &str,
    local_path: &str,
) {
    let dropbox_size = db::get_dropbox_size(&connection, &dropbox_path);
    let local_size = get_local_size(&local_path);
    match dropbox_size == local_size {
        true => (),
        false => panic!("🚫  Local file size {local_size} does not match DropBox {dropbox_size}"),
    }
}
