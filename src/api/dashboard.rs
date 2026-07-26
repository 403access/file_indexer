use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::sql::database::{get_connection, get_ignore_rules};
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct DashboardParams {
    pub interval: Option<String>,
}

#[derive(Serialize)]
pub struct DashboardResponse {
    pub total_files: u64,
    pub total_folders: u64,
    pub total_size: u64,
    pub db_size: u64,
    pub duplicate_file_groups: u64,
    pub duplicate_files: u64,
    pub wasted_file_bytes: u64,
    pub duplicate_folder_groups: u64,
    pub duplicate_folders: u64,
    pub skipped_paths: u64,
    pub ignore_rules_count: u64,
    pub timeline: Vec<TimelineBucket>,
}

#[derive(Serialize)]
pub struct TimelineBucket {
    pub label: String,
    pub files: u64,
    pub folders: u64,
    pub size: u64,
}

pub async fn dashboard_handler(
    State(state): State<AppState>,
    Query(params): Query<DashboardParams>,
) -> Result<Json<DashboardResponse>, (axum::http::StatusCode, String)> {
    let _guard = IndexerPauseGuard::new(&state);
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Single pass for basic counts
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

    let db_size = std::fs::metadata(&state.db).map(|m| m.len()).unwrap_or(0);

    let skipped_paths: u64 = conn
        .query_row("SELECT COUNT(*) FROM skipped_paths", [], |r| r.get(0))
        .unwrap_or(0);

    let ignore_rules_count = get_ignore_rules(&conn).len() as u64;

    // Duplicate file stats from maintained table
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

    // Duplicate folder stats from maintained table
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

    // Timeline
    let interval = params.interval.as_deref().unwrap_or("month");
    let group_sql = match interval {
        "day" => "strftime('%Y-%m-%d', modified, 'unixepoch')",
        "week" => "strftime('%Y-W%W', modified, 'unixepoch')",
        "year" => "strftime('%Y', modified, 'unixepoch')",
        _ => "strftime('%Y-%m', modified, 'unixepoch')",
    };

    let sql = format!(
        "SELECT {group_sql} as bucket,
                SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END) as files,
                SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END) as folders,
                COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0) as size
         FROM files
         WHERE modified IS NOT NULL
         GROUP BY bucket
         ORDER BY bucket ASC"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, u64>(3)?,
            ))
        })
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut timeline = Vec::new();
    for row in rows {
        let (label, files, folders, size) =
            row.map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        timeline.push(TimelineBucket {
            label,
            files,
            folders,
            size,
        });
    }

    Ok(Json(DashboardResponse {
        total_files,
        total_folders,
        total_size,
        db_size,
        duplicate_file_groups,
        duplicate_files,
        wasted_file_bytes,
        duplicate_folder_groups,
        duplicate_folders,
        skipped_paths,
        ignore_rules_count,
        timeline,
    }))
}
