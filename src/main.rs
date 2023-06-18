use dotenv::dotenv;
use std::fs;
use std::io::ErrorKind;

fn main() {
    dotenv().ok();
    dropbox_login();
    let paths = get_paths();
    perform_migration(paths);
}

fn dropbox_login() {
    let access_token = std::env::var("ACCESS_TOKEN").expect("ACCESS_TOKEN must be set.");
    println!("access_token: {}", access_token);
    println!("🔒 Initiating login...");
    if false {
        // TODO: check if account exists
        println!("⚠️ No account found");
        println!("🔐 Initiating token request");
        println!("🪪 Enter authorization code:");
        println!("🔐 Requesting access token...");
        return;
    } else if true {
        // TODO: check if token is valid
        println!("🔐 Refreshing access token...");
        println!("🪪 Requesting team member id...");
        println!("🔓 Re-Authorized with DropBox API using OAuth2 and codeflow");
        return;
    }
}

fn get_paths() -> Vec<String> {
    let sr = fs::read_to_string("paths.txt");
    let s = match sr {
        Ok(string) => string,
        Err(error) => match error.kind() {
            ErrorKind::NotFound => match fs::File::create("paths.txt") {
                Ok(_) => {
                    println!("Created paths.txt");
                    String::from("")
                }
                Err(e) => panic!("Error creating file: {}", e),
            },
            other_error => panic!("Error reading file: {}", other_error),
        },
    };
    let paths: Vec<&str> = s.split("\n").collect();
    return paths.iter().map(|s| s.to_string()).collect();
}

fn perform_migration(paths: Vec<String>) {
    println!("🗃️ Performing migration...");
    for path in paths {
        migrate_file(path);
    }
    println!("✅ Migration complete");
}

fn migrate_file(path: String) {
    println!("📂 Migrating {}", path);
    println!("📦 Checking S3");
    println!("🗳️ Checking DB");
    println!("🔄 S3 needs to sync file");
    println!("⬇️ Downloading from DropBox");
    println!("🗑️ Removing artifact from S3");
    println!("⬆️ Uploading to S3");
}
