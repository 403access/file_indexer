use rusqlite::Connection;

pub fn get_connection(db_path: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    // WAL allows readers during writers; busy_timeout avoids instant SQLITE_BUSY.
    // synchronous=NORMAL is safe with WAL and much faster than FULL.
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=30000;
         PRAGMA temp_store=MEMORY;
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(conn)
}

/// Run blocking work (typically SQLite) on Tokio's blocking pool so the
/// async runtime stays free to accept connections and serve static files.
pub async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("background task failed: {e}"))
}
