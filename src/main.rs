use dotenv::dotenv;

mod auth;
mod aws;
mod db;
mod deepfreeze;
mod dropbox;
mod http;
mod json;
mod localfs;
mod util;

use aws::AWSClient;
use db::DBConnection;
use http::HTTPClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let http: HTTPClient = http::new_client();
    auth::check_account(&http).await;
    let sqlite: DBConnection = db::connect("db.sqlite");
    dropbox::get_paths(&http, &sqlite).await;
    let aws: AWSClient = aws::new_client().await;
    deepfreeze::perform_migration(http, sqlite, aws).await
}
