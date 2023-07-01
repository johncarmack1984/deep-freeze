pub fn new_client() -> reqwest::Client {
    reqwest::Client::builder().build().unwrap()
}
