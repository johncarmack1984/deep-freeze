mod auth;
mod aws;
mod db;
mod deepfreeze;
mod dropbox;
mod json;
mod localfs;
mod util;
use crate::deepfreeze::perform_migration;
use aws_sdk_s3::Client as AWSClient;
use dotenv::dotenv;
use futures::executor::block_on;
use sqlite::ConnectionWithFullMutex as DBConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("");
    dotenv().ok();
    block_on(auth::check_account());
    println!("");
    let db_connection: DBConnection = db::connect();
    db::init(&db_connection);
    block_on(dropbox::get_paths(&db_connection));
    println!("");
    let aws_client: AWSClient = aws::new_client().await;
    perform_migration(&db_connection, &aws_client).await
}
