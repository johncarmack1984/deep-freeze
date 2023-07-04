use sedregex::find_and_replace;
use std::io::{self, Read, Write};
use std::{env, fs, fs::File, path::Path};

use crate::localfs::get_local_file;

pub fn setenv(key: &str, value: String) {
    let envpath = Path::new(".env");
    let envtemp = Path::new(".env.temp");
    let mut src = get_local_file(envpath.to_str().unwrap());
    let mut data = String::new();
    src.read_to_string(&mut data).unwrap();
    let mut newenv: String;
    match data.contains(key) {
        true => {
            newenv = data
                .lines()
                .map(|line| match line.starts_with(format!("{key}=").as_str()) {
                    true => format!("{}=\"{}\"", key, value),
                    false => line.to_string(),
                })
                .collect::<Vec<String>>()
                .join("\n");
        }
        false => {
            data.push_str(format!("\n{}=\"{}\"", key, value).as_str());
            newenv = data;
        }
    }
    newenv.push_str("\n");
    let mut dst = File::create(envtemp).unwrap();
    dst.write_all(newenv.as_bytes()).unwrap();
    fs::rename(envtemp, envpath).unwrap();
    dotenv::dotenv().ok();
    assert_eq!(env::var(key).unwrap(), value);
}

pub fn standardize_path(old_path: &str) -> String {
    let base_folder = env::var("BASE_FOLDER").unwrap();
    let mut path = find_and_replace(
        &old_path.clone().to_owned(),
        &[format!("s/\\{}\\///g", base_folder)],
    )
    .unwrap()
    .to_string();

    path = find_and_replace(
        &path,
        &["s/channel/Channel/g", "s/_/_/g", "s/\\|/\\|/g", "s/•/\\•/g"],
    )
    .unwrap()
    .to_string();

    path.to_string()
}

pub fn prompt(msg: &str) -> String {
    eprint!("{}: ", msg);
    io::stderr().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_owned()
}
