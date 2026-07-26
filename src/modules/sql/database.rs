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

    // Composite indexes for hot queries
    let _ = tx.execute("CREATE INDEX IF NOT EXISTS idx_files_hash_is_file ON files(hash, is_file);", []);
    let _ = tx.execute("CREATE INDEX IF NOT EXISTS idx_files_hash_is_dir ON files(hash, is_directory);", []);
    let _ = tx.execute("CREATE INDEX IF NOT EXISTS idx_files_is_file ON files(is_file);", []);
    let _ = tx.execute("CREATE INDEX IF NOT EXISTS idx_files_is_dir ON files(is_directory);", []);
    let _ = tx.execute("CREATE INDEX IF NOT EXISTS idx_files_modified ON files(modified);", []);
    let _ = tx.execute("CREATE INDEX IF NOT EXISTS idx_logs_level ON logs(level);", []);

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

    // Materialized dashboard stats
    let dashboard_stats_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS dashboard_stats (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at REAL NOT NULL
        );",
        [],
    );
    if dashboard_stats_result.is_err() {
        logging::error(&format!("Failed to create 'dashboard_stats' table: {:?}", dashboard_stats_result));
        return Err(dashboard_stats_result.unwrap_err());
    }

    // Materialized dashboard timeline buckets
    let dashboard_timeline_result = tx.execute(
        "CREATE TABLE IF NOT EXISTS dashboard_timeline (
            id INTEGER PRIMARY KEY,
            interval_type TEXT NOT NULL,
            label TEXT NOT NULL,
            files INTEGER NOT NULL DEFAULT 0,
            folders INTEGER NOT NULL DEFAULT 0,
            size INTEGER NOT NULL DEFAULT 0,
            UNIQUE(interval_type, label)
        );",
        [],
    );
    if dashboard_timeline_result.is_err() {
        logging::error(&format!("Failed to create 'dashboard_timeline' table: {:?}", dashboard_timeline_result));
        return Err(dashboard_timeline_result.unwrap_err());
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

/// Refresh the duplicate_hashes table (drop + recreate). Call after folder commits.
pub fn refresh_duplicate_hashes(conn: &Connection) {
    let _ = conn.execute("DROP TABLE IF EXISTS duplicate_hashes", []);
    let _ = conn.execute(
        "CREATE TABLE duplicate_hashes AS
         SELECT hash FROM files
         WHERE hash IS NOT NULL AND hash != ''
         GROUP BY hash HAVING COUNT(*) > 1",
        [],
    );
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

/// Refresh the materialized dashboard stats and timeline tables.
pub fn refresh_dashboard_stats(conn: &Connection) {
    let now = chrono::Utc::now().timestamp() as f64;

    // Scalar stats
    let (total_files, total_folders, total_size): (u64, u64, u64) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0)
             FROM files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    let skipped_paths: u64 = conn
        .query_row("SELECT COUNT(*) FROM skipped_paths", [], |r| r.get(0))
        .unwrap_or(0);

    let ignore_rules_count = get_ignore_rules(conn).len() as u64;

    // Duplicate stats
    let duplicate_file_groups: u64 = conn
        .query_row("SELECT COUNT(*) FROM duplicate_hashes", [], |r| r.get(0))
        .unwrap_or(0);

    let (duplicate_files, wasted_file_bytes): (u64, u64) = if duplicate_file_groups > 0 {
        conn.query_row(
            "SELECT COALESCE(SUM(cnt), 0), COALESCE(SUM((cnt - 1) * size), 0)
             FROM (
                SELECT COUNT(*) as cnt, MIN(f.size) as size
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_file = 1
                GROUP BY f.hash
             )",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    let duplicate_folders: u64 = if duplicate_file_groups > 0 {
        conn.query_row(
            "SELECT COALESCE(SUM(cnt), 0)
             FROM (
                SELECT COUNT(*) as cnt
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_directory = 1
                GROUP BY f.hash
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    let duplicate_folder_groups: u64 = if duplicate_folders > 0 {
        conn.query_row(
            "SELECT COUNT(*)
             FROM (
                SELECT f.hash
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_directory = 1
                GROUP BY f.hash
                HAVING COUNT(*) > 1
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    // Store scalar stats
    let total_entries = total_files + total_folders;
    let last_entry_id: i64 = conn
        .query_row("SELECT COALESCE(MAX(id), 0) FROM files", [], |r| r.get(0))
        .unwrap_or(0);
    let stats = [
        ("total_files", total_files.to_string()),
        ("total_folders", total_folders.to_string()),
        ("total_size", total_size.to_string()),
        ("skipped_paths", skipped_paths.to_string()),
        ("ignore_rules_count", ignore_rules_count.to_string()),
        ("duplicate_file_groups", duplicate_file_groups.to_string()),
        ("duplicate_files", duplicate_files.to_string()),
        ("wasted_file_bytes", wasted_file_bytes.to_string()),
        ("duplicate_folder_groups", duplicate_folder_groups.to_string()),
        ("duplicate_folders", duplicate_folders.to_string()),
        ("entries_at_refresh", total_entries.to_string()),
        ("last_entry_id", last_entry_id.to_string()),
    ];

    for (key, value) in &stats {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO dashboard_stats (key, value, updated_at) VALUES (:key, :value, :updated_at)",
            named_params! { ":key": key, ":value": value, ":updated_at": now },
        );
    }

    // Refresh timeline for all intervals
    let _ = conn.execute("DELETE FROM dashboard_timeline", []);

    let intervals = ["day", "week", "month", "year"];
    for interval in &intervals {
        let group_sql = match *interval {
            "day" => "strftime('%Y-%m-%d', modified, 'unixepoch')",
            "week" => "strftime('%Y-W%W', modified, 'unixepoch')",
            "year" => "strftime('%Y', modified, 'unixepoch')",
            _ => "strftime('%Y-%m', modified, 'unixepoch')",
        };

        let sql = format!(
            "INSERT INTO dashboard_timeline (interval_type, label, files, folders, size)
             SELECT ?1 as interval_type,
                    {group_sql} as label,
                    SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END) as files,
                    SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END) as folders,
                    COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0) as size
             FROM files
             WHERE modified IS NOT NULL
             GROUP BY label
             ORDER BY label ASC"
        );

        let _ = conn.execute(&sql, [interval]);
    }

    // Store refresh timestamp
    let _ = conn.execute(
        "INSERT OR REPLACE INTO dashboard_stats (key, value, updated_at) VALUES ('last_refreshed', :value, :updated_at)",
        named_params! { ":value": now.to_string(), ":updated_at": now },
    );
}

/// Recompute dashboard stats WITHOUT updating the snapshot timestamp or entries_at_refresh.
/// Used by the periodic timer so the "behind" count keeps growing between manual refreshes.
pub fn recompute_dashboard_stats(conn: &Connection) {
    let now = chrono::Utc::now().timestamp() as f64;

    let (total_files, total_folders, total_size): (u64, u64, u64) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0)
             FROM files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    let skipped_paths: u64 = conn
        .query_row("SELECT COUNT(*) FROM skipped_paths", [], |r| r.get(0))
        .unwrap_or(0);

    let ignore_rules_count = get_ignore_rules(conn).len() as u64;

    let duplicate_file_groups: u64 = conn
        .query_row("SELECT COUNT(*) FROM duplicate_hashes", [], |r| r.get(0))
        .unwrap_or(0);

    let (duplicate_files, wasted_file_bytes): (u64, u64) = if duplicate_file_groups > 0 {
        conn.query_row(
            "SELECT COALESCE(SUM(cnt), 0), COALESCE(SUM((cnt - 1) * size), 0)
             FROM (
                SELECT COUNT(*) as cnt, MIN(f.size) as size
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_file = 1
                GROUP BY f.hash
             )",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    let duplicate_folders: u64 = if duplicate_file_groups > 0 {
        conn.query_row(
            "SELECT COALESCE(SUM(cnt), 0)
             FROM (
                SELECT COUNT(*) as cnt
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_directory = 1
                GROUP BY f.hash
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    let duplicate_folder_groups: u64 = if duplicate_folders > 0 {
        conn.query_row(
            "SELECT COUNT(*)
             FROM (
                SELECT f.hash
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_directory = 1
                GROUP BY f.hash
                HAVING COUNT(*) > 1
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    // Update stats but NOT entries_at_refresh or last_refreshed
    let stats = [
        ("total_files", total_files.to_string()),
        ("total_folders", total_folders.to_string()),
        ("total_size", total_size.to_string()),
        ("skipped_paths", skipped_paths.to_string()),
        ("ignore_rules_count", ignore_rules_count.to_string()),
        ("duplicate_file_groups", duplicate_file_groups.to_string()),
        ("duplicate_files", duplicate_files.to_string()),
        ("wasted_file_bytes", wasted_file_bytes.to_string()),
        ("duplicate_folder_groups", duplicate_folder_groups.to_string()),
        ("duplicate_folders", duplicate_folders.to_string()),
    ];

    for (key, value) in &stats {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO dashboard_stats (key, value, updated_at) VALUES (:key, :value, :updated_at)",
            named_params! { ":key": key, ":value": value, ":updated_at": now },
        );
    }

    // Refresh timeline
    let _ = conn.execute("DELETE FROM dashboard_timeline", []);
    let intervals = ["day", "week", "month", "year"];
    for interval in &intervals {
        let group_sql = match *interval {
            "day" => "strftime('%Y-%m-%d', modified, 'unixepoch')",
            "week" => "strftime('%Y-W%W', modified, 'unixepoch')",
            "year" => "strftime('%Y', modified, 'unixepoch')",
            _ => "strftime('%Y-%m', modified, 'unixepoch')",
        };
        let sql = format!(
            "INSERT INTO dashboard_timeline (interval_type, label, files, folders, size)
             SELECT ?1 as interval_type,
                    {group_sql} as label,
                    SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END) as files,
                    SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END) as folders,
                    COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0) as size
             FROM files
             WHERE modified IS NOT NULL
             GROUP BY label
             ORDER BY label ASC"
        );
        let _ = conn.execute(&sql, [interval]);
    }
}
