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

#[derive(Debug, Clone, Copy)]
pub enum SkippedMatchField {
    Both,
    Path,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub enum SkippedSortField {
    Path,
    Error,
}

/// Count skipped paths matching optional search / field filter.
pub fn count_skipped_paths_filtered(
    conn: &Connection,
    q: Option<&str>,
    match_field: SkippedMatchField,
) -> rusqlite::Result<u64> {
    let (sql, pattern) = build_where(q, match_field);
    let query = format!("SELECT COUNT(*) FROM skipped_paths {sql}");
    if let Some(pat) = pattern {
        conn.query_row(&query, named_params! { ":q": pat }, |row| row.get(0))
    } else {
        conn.query_row(&query, [], |row| row.get(0))
    }
}

/// Fetch a page of skipped paths with search, field filter, and sort.
pub fn get_skipped_paths_page(
    conn: &Connection,
    limit: u32,
    offset: u32,
) -> rusqlite::Result<Vec<(String, String)>> {
    get_skipped_paths_page_filtered(
        conn,
        limit,
        offset,
        None,
        SkippedMatchField::Both,
        SkippedSortField::Path,
        true,
    )
}

pub fn get_skipped_paths_page_filtered(
    conn: &Connection,
    limit: u32,
    offset: u32,
    q: Option<&str>,
    match_field: SkippedMatchField,
    sort: SkippedSortField,
    asc: bool,
) -> rusqlite::Result<Vec<(String, String)>> {
    let (where_sql, pattern) = build_where(q, match_field);
    let order_col = match sort {
        SkippedSortField::Path => "path",
        SkippedSortField::Error => "error",
    };
    let order_dir = if asc { "ASC" } else { "DESC" };
    let query = format!(
        "SELECT path, error FROM skipped_paths {where_sql} ORDER BY {order_col} {order_dir} LIMIT :limit OFFSET :offset"
    );

    let mut stmt = conn.prepare(&query)?;
    let map_row = |row: &rusqlite::Row<'_>| Ok((row.get(0)?, row.get(1)?));

    let mut result = Vec::new();
    if let Some(pat) = pattern {
        let rows = stmt.query_map(
            named_params! { ":q": pat, ":limit": limit, ":offset": offset },
            map_row,
        )?;
        for row in rows {
            result.push(row?);
        }
    } else {
        let rows = stmt.query_map(
            named_params! { ":limit": limit, ":offset": offset },
            map_row,
        )?;
        for row in rows {
            result.push(row?);
        }
    }
    Ok(result)
}

fn build_where(q: Option<&str>, match_field: SkippedMatchField) -> (String, Option<String>) {
    let q = q.map(str::trim).filter(|s| !s.is_empty());
    match q {
        None => (String::new(), None),
        Some(q) => {
            let pattern = format!("%{q}%");
            let clause = match match_field {
                SkippedMatchField::Both => {
                    "WHERE (path LIKE :q COLLATE NOCASE OR error LIKE :q COLLATE NOCASE)".to_string()
                }
                SkippedMatchField::Path => "WHERE path LIKE :q COLLATE NOCASE".to_string(),
                SkippedMatchField::Error => "WHERE error LIKE :q COLLATE NOCASE".to_string(),
            };
            (clause, Some(pattern))
        }
    }
}
