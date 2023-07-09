pub type JSON = serde_json::Value;

pub fn from_res(res: &String) -> JSON {
    match serde_json::from_str::<JSON>(res) {
        Ok(json) => json,
        Err(e) => {
            dbg!(&e);
            panic!("❌  Error: {e}")
        }
    }
}

pub fn get_entries(json: &JSON) -> &Vec<JSON> {
    json.get("entries").unwrap().as_array().unwrap()
}

pub fn get_has_more(json: &JSON) -> bool {
    json.get("has_more").unwrap().as_bool().unwrap()
}

pub fn get_cursor(json: &JSON) -> String {
    json.get("cursor").unwrap().to_string().to_owned()
}

pub fn get_size(json: &JSON) -> i64 {
    match json.get("size").unwrap().as_i64() {
        Some(size) => size,
        None => {
            println!("Your access token is likely expired. Please run `deepfreeze` again, we'll get this handled automatically in a future release.");
            // TODO call auth::refresh_token(), need way to get http client in this function without drilling it through all the way from main()
            std::process::exit(1)
        }
    }
}

pub fn count_files(json: &JSON) -> usize {
    json.get("entries")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row.get(".tag").unwrap().as_str().unwrap() == "file")
        .count()
}

pub fn _get_id(json: &JSON) -> String {
    json.get("id").unwrap().as_str().unwrap().to_string()
}
