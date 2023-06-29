mod auth;
mod aws;
mod db;
mod deepfreeze;
mod dropbox;
mod util;
use aws_sdk_s3::Client as AWSClient;
use dotenv::dotenv;
use futures::executor::block_on;
use inquire::Confirm;
use sqlite::ConnectionWithFullMutex as DBConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    match Confirm::new("Are you sure you want to run the migration?")
        .with_default(true)
        .prompt()
    {
        Ok(true) => println!("🚀  Starting migration"),
        Ok(false) => println!("🚫  Migration cancelled"),
        Err(err) => println!("🚫  {err}"),
    }
    block_on(auth::check_account());

    let db_connection: DBConnection = db::new_connection();
    block_on(dropbox::get_paths(&db_connection));

    let aws_client: AWSClient = aws::new_client().await;
    deepfreeze::perform_migration(&db_connection, &aws_client).await
}
