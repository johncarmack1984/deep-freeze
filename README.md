# Deep Freeze

Deep Freeze is a command-line tool for migrating files to Amazon S3 Deep Archive. It allows you to easily transfer files from various sources, such as Dropbox, to an S3 Deep Archive storage bucket.

## Prerequisites

Before using Deep Freeze, make sure you have the following prerequisites installed:

- Rust programming language
- Cargo package manager

## Installation

To install Deep Freeze, follow these steps:

1. Clone the repository:

   ```bash
   git clone https://github.com/example/deep-freeze.git
   ```

2. Navigate to the project directory:

   ```bash
   cd deep-freeze
   ```

3. Build the project using Cargo:

   ```bash
   cargo build --release
   ```

4. The executable binary will be generated in the `target/release` directory.

## Usage

To use Deep Freeze, follow the steps below:

1. Create a `.env` file in the project directory. This file should contain the necessary environment variables. Refer to the [Environment Variables](#environment-variables) section for a list of required variables.

2. Run the Deep Freeze command with the desired options. Here's an example:

   ```bash
   ./deep-freeze --access-token <dropbox-access-token> --aws-access-key-id <aws-access-key-id> --aws-secret-access-key <aws-secret-access-key> --aws-region <aws-region> --dbfile <path-to-db-file> --env-file <path-to-env-file> --e2e
   ```

   Replace `<dropbox-access-token>`, `<aws-access-key-id>`, `<aws-secret-access-key>`, `<aws-region>`, `<path-to-db-file>`, and `<path-to-env-file>` with the appropriate values.

3. Deep Freeze will perform the migration process and provide the status of the migration. If the migration is successful, the program will exit with a status code of 0. If an error occurs during migration, the program will exit with a non-zero status code.

## Environment Variables

Deep Freeze uses the following environment variables:

- `DROPBOX_ACCESS_TOKEN`: Dropbox access token.
- `AWS_ACCESS_KEY_ID`: AWS access key ID.
- `AWS_SECRET_ACCESS_KEY`: AWS secret access key.
- `AWS_REGION`: AWS region.
- `DBFILE`: Path to the SQLite database file.
- `ENV_FILE`: Path to the `.env` file.
- `E2E`: Set to "true" to run the program with test values.
- `RESET`: Set to "true" to reset the database and temp files.
- `RESET_ONLY`: Set to "true" to reset the database and temp files, then exit.
- `S3_BUCKET`: The S3 bucket to use.
- `SILENT`: Set to "true" to run the program in silent mode.
- `SKIP`: Comma-separated paths to skip during migration.
- `TEMP_DIR`: Path to the temporary directory.

Note: If an environment variable is not set, Deep Freeze will prompt for the value during runtime.

## Contributing

Contributions to Deep Freeze are welcome! If you find any issues or have suggestions for improvement, please open an issue or submit a pull request on the GitHub repository.

## License

This project is licensed under the [MIT License](LICENSE).

## Acknowledgments

Deep Freeze makes use of the following libraries and dependencies:

- [dotenv](https://crates.io/crates/dotenv) - For loading environment variables from a `.env` file.
- [clap](https://crates.io/crates/clap) - For command-line argument parsing.
- [tokio](https://crates.io/crates/tokio) - Asynchronous runtime for Rust.
