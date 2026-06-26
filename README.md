# Deep Freeze

Resumable Rust CLI that bulk-migrates a Dropbox tree into AWS S3 Glacier Deep Archive.

[![Build](https://github.com/johncarmack1984/deep-freeze/actions/workflows/main.yml/badge.svg)](https://github.com/johncarmack1984/deep-freeze/actions/workflows/main.yml) [![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Deep Freeze streams every file in a Dropbox folder, including Dropbox Business team accounts, into S3 with the `DEEP_ARCHIVE` storage class, recording every object in a local SQLite database so the transfer is fully resumable. It was built to retire a Dropbox Business subscription by moving its entire contents into cold storage at a fraction of the price.

## What it has done

Deep Freeze drove a real **15.6 TiB migration** (7,253 files from a Dropbox Business account into S3 Glacier Deep Archive) and confirmed 7,241 of them present in S3 at exact byte size, with the remainder resolved by hand (case-only path renames and a couple of re-uploads). The hard part was the long tail: ten media files between 82 GiB and 1.24 TiB, 5.26 TiB combined. Those were moved by a transient `c6in.xlarge` relay that streamed Dropbox → S3 and self-terminated, so nothing multi-terabyte had to round-trip through a laptop. End state: the source account was fully represented in cold storage and safe to cancel.

## How it works

1. Authenticates to Dropbox over OAuth2 with an offline refresh token; on a Business/Team account it lists members and operates as a selected user via the `Dropbox-API-Select-User` header.
2. Recursively lists the chosen base folder and records every file (id, path, size, and Dropbox `content_hash`) in a local SQLite database.
3. For each unfinished file, streams Dropbox → local temp → S3 with a live progress bar, choosing a single `PutObject` or a multipart upload automatically by size (5 GiB threshold), always with `StorageClass::DeepArchive`.
4. Confirms the uploaded object's size matches Dropbox, marks the row migrated, and deletes the temp copy. The run recurses until no unmigrated files remain, so it is idempotent: kill it and rerun and it resumes exactly where it stopped.

## Integrity

Verification today is **size-based**: a file counts as migrated when its S3 object size equals the size Dropbox reported, and the streamed download asserts it received exactly that many bytes, so truncated transfers fail loudly. Dropbox's `content_hash` is recorded for every file, but end-to-end **hash verification is not yet implemented**: it's a known TODO, and the schema already reserves a column for the S3-side hash. Read the guarantee as "same size, same storage class," not "bit-for-bit checksummed."

## Install

```bash
cargo build --release
# binary at target/release/deep-freeze
```

## Usage

Configuration is environment-driven and self-persisting: any value you pass or enter is written back to the `.env` file, and anything missing is prompted for interactively. Dropbox OAuth opens in your browser, and team member, base folder, and bucket are picked from a list. Copy `.env.example` to `.env` to start.

```bash
# First run, interactive: OAuth, then pick team member / base folder / bucket
./target/release/deep-freeze

# Resume a migration against an explicit DB and bucket
./target/release/deep-freeze --dbfile db.sqlite --s3-bucket my-archive-bucket

# Report progress and exit
./target/release/deep-freeze --status-only

# Re-verify already-migrated files against S3 (size) and exit
./target/release/deep-freeze --check-only

# Refresh the Dropbox token only (for CI)
./target/release/deep-freeze --auth-only
```

Useful flags: `--dbfile` (SQLite path, default `db.sqlite`), `--s3-bucket`, `--aws-region` (default `us-east-1`), `--temp-dir` (default `temp`), `--skip "id1,id2"` (repeatable), `--reset` / `--reset-only` (clear DB + temp files), `--silent`. Run with `--help` for the full list.

## Configuration

Required environment (see `.env.example`): `DROPBOX_REFRESH_TOKEN`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_S3_BUCKET`, `DROPBOX_BASE_FOLDER`. The Dropbox app secret is read from AWS Secrets Manager (`DropboxAppSecret`), not the environment.

## Built with

Async Rust on Tokio: the AWS SDK (`aws-sdk-s3`, `aws-sdk-secretsmanager`) with a custom progress-reporting `ByteStream`, `reqwest` for the Dropbox API, the `sqlite` crate for resumable state, and `clap` / `inquire` / `indicatif` for the CLI.

## License

[MIT](LICENSE).
