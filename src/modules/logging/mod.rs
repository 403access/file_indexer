use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::modules::sql::database::get_connection;

static DB_PATH: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));
static LOGGER_CONN: Lazy<Mutex<Option<rusqlite::Connection>>> = Lazy::new(|| Mutex::new(None));

pub fn init(db_path: &str) {
    *DB_PATH.lock().unwrap() = db_path.to_string();
    if let Ok(conn) = get_connection(db_path) {
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY,
                timestamp TEXT,
                level TEXT,
                message TEXT
            );",
        );
        *LOGGER_CONN.lock().unwrap() = Some(conn);
    }
}

pub fn log(level: &str, message: &str) {
    eprintln!("[{}] {}", level, message);

    let path = DB_PATH.lock().unwrap();
    if path.is_empty() {
        return;
    }
    drop(path);

    let guard = LOGGER_CONN.lock().unwrap();
    if let Some(ref conn) = *guard {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT INTO logs (timestamp, level, message) VALUES (?1, ?2, ?3)",
            rusqlite::params![timestamp, level, message],
        );
    }
}

pub fn error(message: &str) {
    log("ERROR", message);
}

pub fn warn(message: &str) {
    log("WARN", message);
}

pub fn info(message: &str) {
    log("INFO", message);
}

pub fn debug(message: &str) {
    log("DEBUG", message);
}
