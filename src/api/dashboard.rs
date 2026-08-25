use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::offload;
use crate::modules::processes;
use crate::modules::sql::database::{get_connection, get_setting, refresh_dashboard_stats};
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
    pub last_refreshed: Option<f64>,
    pub entries_at_refresh: u64,
    pub entries_behind: u64,
    pub next_refresh: Option<f64>,
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
    let interval = params
        .interval
        .clone()
        .unwrap_or_else(|| "month".to_string());

    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Read from materialized stats table
        let get_stat = |key: &str| -> u64 {
            conn.query_row(
                "SELECT value FROM dashboard_stats WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
        };

        let total_files = get_stat("total_files");
        let total_folders = get_stat("total_folders");
        let total_size = get_stat("total_size");
        let skipped_paths = get_stat("skipped_paths");
        let ignore_rules_count = get_stat("ignore_rules_count");
        let duplicate_file_groups = get_stat("duplicate_file_groups");
        let duplicate_files = get_stat("duplicate_files");
        let wasted_file_bytes = get_stat("wasted_file_bytes");
        let duplicate_folder_groups = get_stat("duplicate_folder_groups");
        let duplicate_folders = get_stat("duplicate_folders");

        let last_refreshed: Option<f64> = conn
            .query_row(
                "SELECT value FROM dashboard_stats WHERE key = 'last_refreshed'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok());

        let db_size = std::fs::metadata(&state.db).map(|m| m.len()).unwrap_or(0);

        let mut stmt = conn
            .prepare(
                "SELECT label, files, folders, size
             FROM dashboard_timeline
             WHERE interval_type = ?1
             ORDER BY label ASC",
            )
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let rows = stmt
            .query_map([interval.as_str()], |row| {
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

        let entries_at_refresh = get_stat("entries_at_refresh");
        let last_entry_id: u64 = get_stat("last_entry_id");
        let current_max_id: u64 = conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM files", [], |r| r.get(0))
            .unwrap_or(0);
        let entries_behind = current_max_id.saturating_sub(last_entry_id);

        let next_refresh: Option<f64> = last_refreshed.and_then(|lr| {
            let interval_secs = get_setting(&conn, "dashboard_refresh_interval")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(60.0);
            if interval_secs > 0.0 {
                Some(lr + interval_secs)
            } else {
                None
            }
        });

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
            last_refreshed,
            entries_at_refresh,
            entries_behind,
            next_refresh,
            timeline,
        }))
    })
    .await
}

pub async fn refresh_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let process_id = processes::register("Manual dashboard refresh", "dashboard", None);

    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db).map_err(|e| {
            processes::fail(process_id, &e.to_string());
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

        refresh_dashboard_stats(&conn);

        let last_refreshed: f64 = conn
            .query_row(
                "SELECT value FROM dashboard_stats WHERE key = 'last_refreshed'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);

        processes::complete(process_id, Some("Dashboard stats refreshed"));
        Ok(Json(serde_json::json!({
            "ok": true,
            "last_refreshed": last_refreshed,
        })))
    })
    .await
}
