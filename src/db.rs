pub fn new_connection() -> sqlite::ConnectionWithFullMutex {
    sqlite::Connection::open_with_full_mutex("db.sqlite").unwrap()
}
