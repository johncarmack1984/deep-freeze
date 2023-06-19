mod dropbox;

fn main() {
    // let client =
    let auth = dropbox::authorize();
    let client = dropbox::get_client(auth);
    let paths = dropbox::get_paths(client);
    println!("{:?}", paths);
    // perform_migration();
}
