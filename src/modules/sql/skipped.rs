use rusqlite::{named_params, Connection, Transaction};

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

pub fn count_skipped_paths(conn: &Connection) -> rusqlite::Result<u64> {
    conn.query_row("SELECT COUNT(*) FROM skipped_paths", [], |row| row.get(0))
}

/// Fetch all skipped paths (legacy full scan — prefer paginated variant).
pub fn get_skipped_paths(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT path, error FROM skipped_paths ORDER BY path")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// Fetch a page of skipped paths ordered by path.
pub fn get_skipped_paths_page(
    conn: &Connection,
    limit: u32,
    offset: u32,
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT path, error FROM skipped_paths ORDER BY path LIMIT :limit OFFSET :offset",
    )?;
    let rows = stmt.query_map(
        named_params! { ":limit": limit, ":offset": offset },
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}
