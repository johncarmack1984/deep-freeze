use atty;
use dotenv::dotenv;
use dropbox_sdk::auth::AuthError;
use dropbox_sdk::default_client::UserAuthDefaultClient;
use dropbox_sdk::files::{self, ListFolderError, ListFolderResult};
use dropbox_sdk::oauth2::{Authorization, AuthorizeUrlBuilder, Oauth2Type};
use dropbox_sdk::{users, Error};
use std::env;
use std::io::{self, Write};

pub fn authorize() -> Authorization {
    dotenv().ok();
    println!("🔒 Authorizing...");
    if let Ok(access_token) = env::var("ACCESS_TOKEN") {
        let auth: Authorization = Authorization::from_access_token(access_token);
        let _client = UserAuthDefaultClient::new(auth.clone());
        match users::get_current_account(&_client) {
            Ok(account) => {
                println!("👤 Logged in as {}", account.unwrap().email);
                return auth;
            }
            Err(e) => {
                // println!("❌ {}", e);
                if let Error::Authentication(auth_error) = e {
                    match auth_error {
                        dropbox_sdk::auth::AuthError::InvalidAccessToken => {
                            println!("❌ Invalid access token")
                        }
                        AuthError::ExpiredAccessToken => {
                            println!("❌ {}", auth_error);
                            println!("🔐 Refreshing access token...");
                            let auth = Authorization::from_refresh_token(
                                env::var("APP_KEY").unwrap(),
                                env::var("REFRESH_TOKEN").unwrap(),
                            );
                            let _client = UserAuthDefaultClient::new(auth.clone());
                            match users::get_current_account(&_client) {
                                Ok(account) => {
                                    println!("👤 Logged in as {}", account.unwrap().email);
                                    return auth;
                                }
                                Err(e) => {
                                    println!("❌ {}", e);
                                }
                            }
                        }
                        _ => println!("❌ {}", auth_error),
                    }
                };
            }
        }
        // return auth;
    }
    if let (Ok(client_id), Ok(saved)) = (env::var("APP_KEY"), env::var("REFRESH_TOKEN"))
    // important! see the above warning about using environment variables for this
    {
        match Authorization::load(client_id, &saved) {
            Some(auth) => {
                eprintln!("🔓 Re-Authorized with DropBox API using OAuth2 and codeflow");
                return auth;
            }
            None => {
                eprintln!("saved authorization in APP_KEY and REFRESH_TOKEN are invalid");
                // and fall back to prompting
            }
        }
    }
    if !atty::is(atty::Stream::Stdin) {
        panic!("APP_KEY and/or REFRESH_TOKEN not set, and stdin not a TTY; cannot authorize");
    }
    fn prompt(msg: &str) -> String {
        eprint!("{}: ", msg);
        io::stderr().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        input.trim().to_owned()
    }
    let client_id: String = env::var("APP_KEY").unwrap();
    let oauth2_flow: Oauth2Type =
        Oauth2Type::AuthorizationCode(env::var("AUTHORIZATION_CODE").unwrap());
    let url = AuthorizeUrlBuilder::new(&client_id, &oauth2_flow).build();
    eprintln!("Open this URL in your browser:");
    eprintln!("{}", url);
    eprintln!();
    let auth_code: String = prompt("Then paste the code here");
    Authorization::from_auth_code(client_id, oauth2_flow, auth_code.trim().to_owned(), None)
}

pub fn get_client(auth: Authorization) -> UserAuthDefaultClient {
    let client: UserAuthDefaultClient = UserAuthDefaultClient::new(auth);
    return client;
}

pub fn get_paths(
    client: UserAuthDefaultClient,
) -> std::result::Result<std::result::Result<ListFolderResult, ListFolderError>, dropbox_sdk::Error>
{
    println!("📂 Fetching paths...");
    let paths: Result<Result<ListFolderResult, ListFolderError>, dropbox_sdk::Error> =
        files::list_folder(
            &client,
            &files::ListFolderArg::new({
                let ref this: String = env::var("BASE_FOLDER").unwrap();
                unsafe { String::from_utf8_unchecked(this.as_bytes().to_owned()) }
            }),
        );
    return paths;
}
