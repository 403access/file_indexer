use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::sql::database::{get_connection, get_logs};
use crate::states::app_state::AppState;

#[derive(Deserialize)]
pub struct LogsParams {
    pub limit: Option<i64>,
    pub level: Option<String>,
    pub search: Option<String>,
    pub sort: Option<String>,
}

#[derive(Serialize)]
pub struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Vec<FileSummary>>,
}

#[derive(Serialize)]
pub struct FileSummary {
    name: String,
    size: u64,
}

pub async fn logs_handler(
    State(state): State<AppState>,
    Query(params): Query<LogsParams>,
) -> Json<Vec<LogEntry>> {
    let limit = params.limit.unwrap_or(1000);
    let level = params.level.as_deref().filter(|l| *l != "all");
    let search = params.search.as_deref().filter(|s| !s.is_empty());
    let sort_asc = params.sort.as_deref() == Some("asc");

    let conn = get_connection(&state.db).unwrap();
    let logs = get_logs(&conn, limit, level, search, sort_asc).unwrap_or_default();

    let mut entries: Vec<LogEntry> = Vec::with_capacity(logs.len());
    for (timestamp, level, message) in logs {
        let files = if level == "INFO" && message.starts_with("Indexed '") {
            extract_indexed_path(&message)
                .and_then(|path| get_files_for_dir(&conn, &path))
        } else {
            None
        };
        entries.push(LogEntry { timestamp, level, message, files });
    }

    Json(entries)
}

fn extract_indexed_path(message: &str) -> Option<String> {
    let start = message.find("'")? + 1;
    let end = message[start..].find("'")? + start;
    let raw = &message[start..end];
    Some(raw.replace("//", "/"))
}

fn get_files_for_dir(conn: &rusqlite::Connection, dir_path: &str) -> Option<Vec<FileSummary>> {
    let normalized = dir_path.replace("//", "/");
    let prefix = if normalized.ends_with('/') {
        normalized.clone()
    } else {
        format!("{}/", normalized)
    };

    let mut stmt = conn
        .prepare(
            "SELECT fn.name, f.size, f.path FROM files f JOIN file_names fn ON f.file_name_id = fn.id
             WHERE f.is_file = 1 AND f.path LIKE ?1",
        )
        .ok()?;

    let pattern = format!("{}%", prefix);
    let rows: Vec<FileSummary> = stmt
        .query_map(rusqlite::params![pattern], |row| {
            let name: String = row.get(0)?;
            let size: u64 = row.get(1)?;
            let path: String = row.get(2)?;
            Ok((name, size, path))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .filter(|(_, _, path)| {
            // Only direct children: after removing prefix, no remaining slashes
            let rel = &path[prefix.len()..];
            !rel.contains('/')
        })
        .map(|(name, size, _)| FileSummary { name, size })
        .collect();

    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}
