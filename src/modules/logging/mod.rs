use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::modules::sql::database::{get_connection, insert_log};

static DB_PATH: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

pub fn init(db_path: &str) {
    *DB_PATH.lock().unwrap() = db_path.to_string();
}

pub fn log(level: &str, message: &str) {
    let path = DB_PATH.lock().unwrap();
    if path.is_empty() {
        return;
    }
    if let Ok(conn) = get_connection(&path) {
        let _ = insert_log(&conn, level, message);
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
