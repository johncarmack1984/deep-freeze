use sedregex::find_and_replace;
use std::io::{self, Read, Write};
use std::{env, fs, fs::File, path::Path};

use crate::localfs::get_local_file;

pub fn setenv(key: &str, value: String) {
    env::set_var(key, value.clone());
    update_env_file(key, value);
}

pub fn getenv(key: &str) -> String {
    env::var(key).unwrap()
}

pub fn update_env_file(key: &str, value: String) {
    let env_filename = env::var("ENV_FILE").unwrap();
    let env_path = Path::new(&env_filename);
    let env_temp_filename = format!("{env_filename}.temp", env_filename = &env_filename);
    let env_temp_path = Path::new(&env_temp_filename);
    let mut src = get_local_file(env_path.to_str().unwrap());
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
            data.push_str(format!("{}=\"{}\"", key, value).as_str());
            newenv = data;
        }
    }
    newenv.push_str("\n");
    let mut dst = File::create(env_temp_path).unwrap();
    dst.write_all(newenv.as_bytes()).unwrap();
    fs::rename(env_temp_path, env_path).unwrap();
    dotenv::from_filename(env_filename).ok();
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

pub fn coerce_static_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}
