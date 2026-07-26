use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::sql::database::{get_connection, get_logs};
use crate::states::app_state::{AppState, IndexerPauseGuard};

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

// Only enrich this many log entries with file lists (N+1 query cost)
const MAX_ENRICHED_ENTRIES: usize = 50;

pub async fn logs_handler(
    State(state): State<AppState>,
    Query(params): Query<LogsParams>,
) -> Json<Vec<LogEntry>> {
    let _guard = IndexerPauseGuard::new(&state);
    let limit = params.limit.unwrap_or(1000);
    let level = params.level.as_deref().filter(|l| *l != "all");
    let search = params.search.as_deref().filter(|s| !s.is_empty());
    let sort_asc = params.sort.as_deref() == Some("asc");

    let conn = get_connection(&state.db).unwrap();
    let logs = get_logs(&conn, limit, level, search, sort_asc).unwrap_or_default();

    let mut entries: Vec<LogEntry> = Vec::with_capacity(logs.len());
    let mut enriched_count = 0usize;

    for (timestamp, level, message) in logs {
        let files = if enriched_count < MAX_ENRICHED_ENTRIES
            && level == "INFO"
            && message.starts_with("Indexed '")
        {
            extract_indexed_path(&message)
                .and_then(|path| get_files_for_dir(&conn, &path))
                .map(|f| {
                    enriched_count += 1;
                    f
                })
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

    // Use parent_path index for direct children lookup
    let parent_path = normalized.trim_end_matches('/');

    let mut stmt = conn
        .prepare(
            "SELECT fn.name, f.size FROM files f
             JOIN file_names fn ON f.file_name_id = fn.id
             WHERE f.is_file = 1 AND f.parent_path = ?1",
        )
        .ok()?;

    let rows: Vec<FileSummary> = stmt
        .query_map([parent_path], |row| {
            Ok(FileSummary {
                name: row.get(0)?,
                size: row.get(1)?,
            })
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}
