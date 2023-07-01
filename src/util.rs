use sedregex::find_and_replace;
use std::io::{Read, Write};
use std::{env, fs::File, path::Path};

pub fn setenv(key: &str, value: String) -> Result<(), Box<dyn std::error::Error>> {
    let envpath = Path::new(".env");
    let mut src = File::open(envpath).unwrap();
    let mut data = String::new();
    src.read_to_string(&mut data).unwrap();
    drop(src);
    let regex = format!("s/{}=.*/{}={}/g", key, key, value);
    let newenv = find_and_replace(&data, &[regex]).unwrap();
    let mut dst = File::create(envpath).unwrap();
    dst.write_all(newenv.as_bytes()).unwrap();
    env::set_var(key, value.clone());
    assert_eq!(env::var(key).unwrap(), value);
    Ok(())
}

// pub fn standardize_path(old_path: &str) -> String {
//     let base_folder = env::var("BASE_FOLDER").unwrap();
//     let mut path = find_and_replace(
//         &old_path.clone().to_owned(),
//         &[format!("s/\\{}\\///g", base_folder)],
//     )
//     .unwrap()
//     .to_string();

//     path = find_and_replace(
//         &path,
//         &["s/channel/Channel/g", "s/_/_/g", "s/\\|/\\|/g", "s/•/\\•/g"],
//     )
//     .unwrap()
//     .to_string();

//     path.to_string()
// }
