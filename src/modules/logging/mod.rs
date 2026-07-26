use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::modules::sql::database::get_connection;

static DB_PATH: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

pub fn init(db_path: &str) {
    *DB_PATH.lock().unwrap() = db_path.to_string();
}

pub fn log(level: &str, message: &str) {
    let path = DB_PATH.lock().unwrap();
    if path.is_empty() {
        eprintln!("[{}] {}", level, message);
        return;
    }
    if let Ok(conn) = get_connection(&path) {
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY,
                timestamp TEXT,
                level TEXT,
                message TEXT
            );",
        );
        let timestamp = chrono::Utc::now().to_rfc3339();
        if let Err(e) = conn.execute(
            "INSERT INTO logs (timestamp, level, message) VALUES (?1, ?2, ?3)",
            rusqlite::params![timestamp, level, message],
        ) {
            eprintln!("[LOG-DB-ERROR] Failed to insert log: {}", e);
        }
    }
    eprintln!("[{}] {}", level, message);
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
