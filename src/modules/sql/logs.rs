use rusqlite::{named_params, Connection};

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

/// Count how many times a specific ignore rule has been applied, based on the
/// marker logs emitted by the indexer: "Ignored folder '<name>' via rule '<raw>'".
/// The rule raw string is escaped so LIKE wildcards (% and _) in a rule name do
/// not cause false matches.
pub fn count_ignore_events(conn: &Connection, rule_raw: &str) -> rusqlite::Result<i64> {
    let escaped = rule_raw
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%via rule '{}'%", escaped);
    conn.query_row(
        "SELECT COUNT(*) FROM logs WHERE message LIKE ?1 ESCAPE '\\'",
        [pattern],
        |row| row.get(0),
    )
}
