use rusqlite::{named_params, Connection, Transaction, OptionalExtension};

use crate::modules::file_entry::_types::FileEntry;
use crate::modules::logging;

pub fn get_connection(db_path: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
    Ok(conn)
}

pub fn init_db(tx: &Transaction) -> rusqlite::Result<()> {
    let file_names_table_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS file_names (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE
        );",
        [],
    );

    if file_names_table_result.is_err() {
        logging::error(&format!(
            "Failed to create 'file_names' table: {:?}",
            file_names_table_result
        ));
        return Err(file_names_table_result.unwrap_err());
    }

    let files_table_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE,
            file_name_id INTEGER,
            size INTEGER,
            modified REAL,
            hash TEXT,
            is_directory INTEGER,
            is_file INTEGER,
            is_symlink INTEGER,

            FOREIGN KEY(file_name_id) REFERENCES file_names(id)
        );",
        [],
    );

    if files_table_result.is_err() {
        logging::error(&format!("Failed to create 'files' table: {:?}", files_table_result));
        return Err(files_table_result.unwrap_err());
    }

    let hash_index_result = tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_hash ON files(hash);",
        [],
    );
    if hash_index_result.is_err() {
        logging::error(&format!("Failed to create hash index: {:?}", hash_index_result));
        return Err(hash_index_result.unwrap_err());
    }

    let path_index_result = tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);",
        [],
    );
    if path_index_result.is_err() {
        logging::error(&format!("Failed to create path index: {:?}", path_index_result));
        return Err(path_index_result.unwrap_err());
    }

    let skipped_table_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS skipped_paths (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE,
            error TEXT
        );",
        [],
    );
    if skipped_table_result.is_err() {
        logging::error(&format!("Failed to create 'skipped_paths' table: {:?}", skipped_table_result));
        return Err(skipped_table_result.unwrap_err());
    }

    let logs_table_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY,
            timestamp TEXT,
            level TEXT,
            message TEXT
        );",
        [],
    );
    if logs_table_result.is_err() {
        logging::error(&format!("Failed to create 'logs' table: {:?}", logs_table_result));
        return Err(logs_table_result.unwrap_err());
    }

    let settings_table_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
        [],
    );
    if settings_table_result.is_err() {
        logging::error(&format!("Failed to create 'settings' table: {:?}", settings_table_result));
        return Err(settings_table_result.unwrap_err());
    }

    return Ok(());
}

pub fn insert_file_name(tx: &Transaction, name: &str) -> rusqlite::Result<i64> {
    let affected = tx.execute(
        "INSERT OR IGNORE INTO file_names (name) VALUES (:name)",
        named_params! { ":name": name },
    )?;
    if affected == 1 {
        // Row was inserted, return the new id
        Ok(tx.last_insert_rowid())
    } else if affected == 0 {
        // Row was ignored (already exists)
        Err(rusqlite::Error::ExecuteReturnedResults)
    } else {
        Err(rusqlite::Error::ExecuteReturnedResults)
    }
}

/// We loop over the entries and insert them into the database in a transaction.
/// rusqlite does not support bulk inserts directly, so we insert each file individually.
pub fn insert_file(tx: &Transaction, file: &FileEntry, file_name_id: i64) -> rusqlite::Result<i64> {
    let affected = tx.execute(
        "INSERT OR IGNORE INTO files (path, file_name_id, size, modified, hash, is_directory, is_file, is_symlink)
         VALUES (:path, :name_id, :size, :modified, :hash, :is_directory, :is_file, :is_symlink)",
        named_params! {
            ":path": &file.path,
            ":name_id": file_name_id,
            ":size": &file.size,
            ":modified": &file.modified,
            ":hash": &file.hash,
            ":is_directory": file.is_directory as i32,
            ":is_file": file.is_file as i32,
            ":is_symlink": file.is_symlink as i32,
        },
    )?;
    if affected == 1 {
        // Row was inserted, return the new id
        Ok(tx.last_insert_rowid())
    } else {
        // Row was ignored (already exists) — not an error
        Ok(0)
    }
}

pub fn create_duplicates_table(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute(
        "
        CREATE TABLE IF NOT EXISTS duplicate_hashes AS
        SELECT hash
        FROM files
        GROUP BY hash
        HAVING COUNT(*) > 1;
        ",
        [],
    )?;
    Ok(())
}

pub fn remove_duplicates_table(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute("DROP TABLE IF EXISTS duplicate_hashes", [])?;
    Ok(())
}

pub fn reset_duplicates_table(tx: &Transaction) -> rusqlite::Result<()> {
    remove_duplicates_table(tx)?;
    create_duplicates_table(tx)?;
    Ok(())
}

pub fn insert_skipped_path(tx: &Transaction, path: &str, error: &str) -> rusqlite::Result<i64> {
    let affected = tx.execute(
        "INSERT OR IGNORE INTO skipped_paths (path, error) VALUES (:path, :error)",
        named_params! { ":path": path, ":error": error },
    )?;
    if affected == 1 {
        Ok(tx.last_insert_rowid())
    } else {
        Err(rusqlite::Error::ExecuteReturnedResults)
    }
}

pub fn get_skipped_paths(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT path, error FROM skipped_paths")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn insert_log(conn: &Connection, level: &str, message: &str) -> rusqlite::Result<()> {
    let timestamp = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO logs (timestamp, level, message) VALUES (:timestamp, :level, :message)",
        named_params! { ":timestamp": timestamp, ":level": level, ":message": message },
    )?;
    Ok(())
}

pub fn get_logs(
    conn: &Connection,
    limit: i64,
    level: Option<&str>,
    search: Option<&str>,
    sort_asc: bool,
) -> rusqlite::Result<Vec<(String, String, String)>> {
    let order = if sort_asc { "ASC" } else { "DESC" };

    let mut sql = format!(
        "SELECT timestamp, level, message FROM logs WHERE 1=1"
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(l) = level {
        sql.push_str(" AND level = ?1");
        params.push(Box::new(l.to_string()));
    }

    if let Some(s) = search {
        if s.ends_with('*') {
            let prefix = s.trim_end_matches('*');
            let idx = params.len() + 1;
            sql.push_str(&format!(" AND message LIKE ?{}", idx));
            params.push(Box::new(format!("{}%", prefix)));
        } else {
            let idx = params.len() + 1;
            sql.push_str(&format!(" AND message = ?{}", idx));
            params.push(Box::new(s.to_string()));
        }
    }

    sql.push_str(&format!(" ORDER BY id {}", order));

    let idx = params.len() + 1;
    sql.push_str(&format!(" LIMIT ?{}", idx));
    params.push(Box::new(limit));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (:key, :value)",
        named_params! { ":key": key, ":value": value },
    )?;
    Ok(())
}

pub fn get_ignore_list(conn: &Connection) -> Vec<String> {
    get_setting(conn, "ignore_folders")
        .ok()
        .flatten()
        .map(|v| {
            v.split('\n')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_ignore_list(conn: &Connection, folders: &[String]) -> rusqlite::Result<()> {
    let value = folders.join("\n");
    set_setting(conn, "ignore_folders", &value)
}
