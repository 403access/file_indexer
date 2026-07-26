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
            parent_path TEXT,
            traversed INTEGER DEFAULT 0,

            FOREIGN KEY(file_name_id) REFERENCES file_names(id),
            FOREIGN KEY(parent_path) REFERENCES files(path)
        );",
        [],
    );

    if files_table_result.is_err() {
        logging::error(&format!("Failed to create 'files' table: {:?}", files_table_result));
        return Err(files_table_result.unwrap_err());
    }

    // Migration: add columns to existing databases (errors ignored if already present)
    let _ = tx.execute("ALTER TABLE files ADD COLUMN parent_path TEXT", []);
    let _ = tx.execute("ALTER TABLE files ADD COLUMN traversed INTEGER DEFAULT 0", []);

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

    // Seed default ignore rules if not already set
    let has_ignore = tx.query_row(
        "SELECT COUNT(*) FROM settings WHERE key = 'ignore_folders'",
        [],
        |row| row.get::<_, i32>(0),
    ).unwrap_or(0);
    if has_ignore == 0 {
        let seed_rules = [
            "node_modules:package.json",
            ".next:package.json",
            "dist:package.json",
            "build:package.json",
            "target:Cargo.toml",
            ".gradle:build.gradle",
            "vendor:composer.json",
            "__pycache__:setup.py",
            ".venv:pyproject.toml",
            ".turbo:package.json",
            "bin:*.csproj",
            "obj:*.csproj",
            "__history:*.dpr",
            "__recovery:*.dpr",
        ].join("\n");
        let _ = tx.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('ignore_folders', ?1)",
            [&seed_rules],
        );
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

pub fn get_or_insert_file_name(tx: &Transaction, name: &str) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT OR IGNORE INTO file_names (name) VALUES (:name)",
        named_params! { ":name": name },
    )?;
    tx.query_row(
        "SELECT id FROM file_names WHERE name = :name",
        named_params! { ":name": name },
        |row| row.get(0),
    )
}

/// We loop over the entries and insert them into the database in a transaction.
/// rusqlite does not support bulk inserts directly, so we insert each file individually.
pub fn insert_file(tx: &Transaction, file: &FileEntry, file_name_id: i64, parent_path: Option<&str>) -> rusqlite::Result<i64> {
    let affected = tx.execute(
        "INSERT OR IGNORE INTO files (path, file_name_id, size, modified, hash, is_directory, is_file, is_symlink, parent_path)
         VALUES (:path, :name_id, :size, :modified, :hash, :is_directory, :is_file, :is_symlink, :parent_path)",
        named_params! {
            ":path": &file.path,
            ":name_id": file_name_id,
            ":size": &file.size,
            ":modified": &file.modified,
            ":hash": &file.hash,
            ":is_directory": file.is_directory as i32,
            ":is_file": file.is_file as i32,
            ":is_symlink": file.is_symlink as i32,
            ":parent_path": parent_path,
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

pub struct IgnoreRule {
    pub name: String,
    pub condition: Option<String>,
}

impl IgnoreRule {
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        if let Some((name, condition)) = raw.split_once(':') {
            let name = name.trim().to_string();
            let condition = condition.trim().to_string();
            if name.is_empty() || condition.is_empty() {
                return Some(Self { name, condition: None });
            }
            Some(Self { name, condition: Some(condition) })
        } else {
            Some(Self { name: raw.to_string(), condition: None })
        }
    }

    pub fn should_skip(&self, parent_path: &std::path::Path) -> bool {
        match &self.condition {
            None => true,
            Some(cond) => {
                let sibling = parent_path.join(cond);
                sibling.exists()
            }
        }
    }

    pub fn to_raw(&self) -> String {
        match &self.condition {
            Some(cond) => format!("{}:{}", self.name, cond),
            None => self.name.clone(),
        }
    }
}

pub fn get_ignore_rules(conn: &Connection) -> Vec<IgnoreRule> {
    get_setting(conn, "ignore_folders")
        .ok()
        .flatten()
        .map(|v| {
            v.split('\n')
                .filter_map(|s| IgnoreRule::parse(s))
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_ignore_rules(conn: &Connection, rules: &[IgnoreRule]) -> rusqlite::Result<()> {
    let value: String = rules
        .iter()
        .map(|r| r.to_raw())
        .collect::<Vec<_>>()
        .join("\n");
    set_setting(conn, "ignore_folders", &value)
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

pub fn mark_directory_traversed(tx: &Transaction, path: &str) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE files SET traversed = 1 WHERE path = ?1",
        [path],
    )?;
    Ok(())
}

pub fn is_directory_indexed(conn: &Connection, path: &str) -> bool {
    let trimmed = path.trim_end_matches('/');
    conn.query_row(
        "SELECT traversed FROM files WHERE path = ?1 AND is_directory = 1",
        [trimmed],
        |row| row.get::<_, i32>(0),
    )
    .unwrap_or(0)
        == 1
}

pub fn get_child_directories(conn: &Connection, parent_path: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT path FROM files WHERE parent_path = ?1 AND is_directory = 1",
    )?;
    let rows = stmt.query_map([parent_path], |row| row.get(0))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}
