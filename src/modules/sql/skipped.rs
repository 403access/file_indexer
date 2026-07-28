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
