use serde_json::Value;

pub type JSON = serde_json::Value;

pub fn from_res(res: &String) -> Value {
    match serde_json::from_str::<Value>(res) {
        Ok(json) => json,
        Err(e) => panic!("❌  Error: {e}"),
    }
}

pub fn get_entries(json: &Value) -> &Vec<serde_json::Value> {
    json.get("entries").unwrap().as_array().unwrap()
}

pub fn get_has_more(json: &Value) -> bool {
    json.get("has_more").unwrap().as_bool().unwrap()
}

pub fn get_cursor(json: &Value) -> String {
    json.get("cursor").unwrap().to_string().to_owned()
}

pub fn get_size(json: &Value) -> i64 {
    match json.get("size").unwrap().as_i64() {
        Some(size) => size,
        None => {
            println!("Your access token is likely expired. Please run `deepfreeze` again, we'll get this handled automatically in a future release.");
            // TODO call auth::refresh_token(), need way to get http client in this function without drilling it through all the way from main()
            std::process::exit(1)
        }
    }
}

pub fn _get_id(json: &Value) -> String {
    json.get("id").unwrap().as_str().unwrap().to_string()
}

pub fn count_files(json: &Value) -> usize {
    json.get("entries")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row.get(".tag").unwrap().as_str().unwrap() == "file")
        .count()
}
