use crate::util::setenv;
use inquire::Text;

pub async fn _prompt_for_access_token() {
    let access_token = Text::new("Enter your Dropbox access token: ").prompt();
    match access_token {
        Ok(access_token) => {
            setenv("DROPBOX_ACCESS_TOKEN", access_token);
            println!("🔑  Access token set");
        }
        Err(_) => panic!("❌  Access token not set"),
    }
}
