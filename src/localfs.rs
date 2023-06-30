use std::{fs, path::Path};

use sedregex::find_and_replace;

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
