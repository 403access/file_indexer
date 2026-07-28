use rusqlite::Transaction;

use crate::modules::logging;

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

    Ok(())
}
