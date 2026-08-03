use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;

use crate::modules::sql::database::get_connection;

static DB_PATH: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));
static LOGGER_CONN: Lazy<Mutex<Option<rusqlite::Connection>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileSummary>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileSummary {
    pub name: String,
    pub size: u64,
}

pub fn init(db_path: &str) {
    *DB_PATH.lock().unwrap() = db_path.to_string();
    if let Ok(conn) = get_connection(db_path) {
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY,
                timestamp TEXT,
                level TEXT,
                message TEXT,
                process_id INTEGER
            );",
        );
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_process_id ON logs(process_id)", []);
        *LOGGER_CONN.lock().unwrap() = Some(conn);
    }
}

pub fn log(level: &str, message: &str) {
    log_with_process(level, message, None);
}

pub fn log_with_process(level: &str, message: &str, process_id: Option<u64>) {
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
            "INSERT INTO logs (timestamp, level, message, process_id) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![timestamp, level, message, process_id],
        );
    }
}

pub fn error(message: &str) {
    log("ERROR", message);
}

pub fn error_with_process(message: &str, process_id: u64) {
    log_with_process("ERROR", message, Some(process_id));
}

pub fn warn(message: &str) {
    log("WARN", message);
}

pub fn warn_with_process(message: &str, process_id: u64) {
    log_with_process("WARN", message, Some(process_id));
}

pub fn info(message: &str) {
    log("INFO", message);
}

pub fn info_with_process(message: &str, process_id: u64) {
    log_with_process("INFO", message, Some(process_id));
}

pub fn debug(message: &str) {
    log("DEBUG", message);
}

pub fn debug_with_process(message: &str, process_id: u64) {
    log_with_process("DEBUG", message, Some(process_id));
}
