pub type HTTPClient = reqwest::Client;
pub type HeaderMap = reqwest::header::HeaderMap;

pub fn new_client() -> HTTPClient {
    HTTPClient::builder().build().unwrap()
}

pub fn dropbox_authorization_header(headers: &mut HeaderMap) -> HeaderMap {
    let access_token = ::std::env::var("ACCESS_TOKEN").unwrap();
    headers.insert(
        "Authorization",
        format!("Bearer {}", access_token).parse().unwrap(),
    );
    headers.to_owned()
}

pub fn dropbox_select_admin_header(headers: &mut HeaderMap) -> HeaderMap {
    let team_member_id = ::std::env::var("TEAM_MEMBER_ID").unwrap();
    headers.insert(
        "Dropbox-API-Select-Admin",
        format!("{}", team_member_id).parse().unwrap(),
    );
    headers.to_owned()
}

pub fn dropbox_content_type_json_header(headers: &mut HeaderMap) -> HeaderMap {
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.to_owned()
}

pub fn dropbox_content_type_x_www_form_urlencoded_header(headers: &mut HeaderMap) -> HeaderMap {
    headers.insert(
        "Content-Type",
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    headers.to_owned()
}
