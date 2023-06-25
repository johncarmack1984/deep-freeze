# Deep Freeze

This is a Dropbox to Amazon S3 migration tool that uses a local SQLite database to track the migration status. It is designed to handle large migrations and recover from failures.

## Prerequisites

- You will need a Dropbox API access token and a configured AWS CLI with S3 access.
- This tool uses the `rusoto_s3` and `rusoto_core` crates for AWS S3 interaction and `reqwest` for HTTP requests to the Dropbox API.
- The `indicatif` crate provides a nice progress bar during file downloads, and `dotenv` is used for loading environment variables.
- SQLite is used to keep track of file paths and migration status.

## How it works

1. `dotenv().ok();`: Loads environment variables from a .env file located in the same directory.
2. `check_account().await;`: Checks the Dropbox account details to verify that the API access token is correct and the account is accessible.
3. `get_paths().await;`: Retrieves a list of file paths that need to be migrated from the SQLite database. If the list is empty, it queries the Dropbox API to fetch the file paths and populates the SQLite database with them.
4. `perform_migration().await?;`: Takes the paths retrieved by `get_paths()` and begins the process of migrating them to the S3 bucket. It downloads the files from Dropbox, checks their integrity, uploads them to the S3 bucket, and updates the SQLite database to reflect the migrated status.
5. `println!("✅✅✅  Migration complete");`: Prints a success message to the console when all files have been successfully migrated.

## Error handling

The `main` function uses the `?` operator, which can return early with an error if the `perform_migration()` function fails. The returned error is of type `Box<dyn std::error::Error>`, a boxed dynamic error trait object, which means that the function can return any type that implements the `Error` trait. This allows the function to return errors of different types in different situations.
