use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use crate::modules::processes::{self, Process};
use crate::modules::sql::database::{get_connection, refresh_dashboard_stats};

use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Serialize)]
pub struct ProcessesResponse {
    pub processes: Vec<Process>,
}

pub async fn processes_handler() -> Json<ProcessesResponse> {
    let processes = processes::get_all();
    Json(ProcessesResponse { processes })
}

pub async fn clear_processes_handler() -> Json<serde_json::Value> {
    processes::clear_completed();
    Json(serde_json::json!({ "ok": true }))
}

#[derive(Serialize)]
pub struct ProcessActionResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct ProcessLogsParams {
    pub limit: Option<i64>,
}

pub async fn process_logs_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Query(params): Query<ProcessLogsParams>,
) -> Result<Json<Vec<crate::modules::logging::LogEntry>>, (axum::http::StatusCode, String)> {
    let _guard = IndexerPauseGuard::new(&state);
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let limit = params.limit.unwrap_or(1000);
    let mut stmt = conn.prepare(
        "SELECT timestamp, level, message FROM logs WHERE process_id = ?1 ORDER BY timestamp DESC LIMIT ?2"
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let logs: Vec<crate::modules::logging::LogEntry> = stmt
        .query_map(rusqlite::params![id, limit], |row| {
            Ok(crate::modules::logging::LogEntry {
                timestamp: row.get(0)?,
                level: row.get(1)?,
                message: row.get(2)?,
                files: None,
            })
        })
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Json(logs))
}

pub async fn trigger_process_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<ProcessActionResponse>, (axum::http::StatusCode, String)> {
    let all_processes = processes::get_all();
    let process = all_processes.iter().find(|p| p.id == id).ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("Process #{} not found", id),
    ))?;

    let name = process.name.as_str();
    let category = process.category.as_str();

    let new_id = match category {
        "dashboard" => {
            let process_id = processes::register_controllable(
                &format!("Manual {} ({})", name, category),
                category,
                Some("Manual trigger"),
            );

            let conn = get_connection(&state.db)
                .map_err(|e| {
                    processes::fail(process_id, &e.to_string());
                    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                })?;

            refresh_dashboard_stats(&conn);

            let _last_refreshed: f64 = conn
                .query_row(
                    "SELECT value FROM dashboard_stats WHERE key = 'last_refreshed'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0);

            processes::complete(process_id, Some("Dashboard stats refreshed"));
            process_id
        }
        "duplicate-folders" => {
            let process_id = processes::register_controllable(
                &format!("Manual {} ({})", name, category),
                category,
                Some("Manual trigger"),
            );

            let conn = get_connection(&state.db)
                .map_err(|e| {
                    processes::fail(process_id, &e.to_string());
                    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                })?;

            crate::modules::sql::database::refresh_duplicate_folder_groups(&conn);
            processes::complete(process_id, Some("Duplicate folder groups refreshed"));
            process_id
        }
        _ => {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                format!("Cannot manually trigger process category: {}", category),
            ));
        }
    };

    Ok(Json(ProcessActionResponse {
        ok: true,
        message: format!("Triggered process #{} (new process #{} running)", id, new_id),
    }))
}

pub async fn pause_process_handler(
    State(_state): State<AppState>,
    Path(id): Path<u64>,
) -> Json<ProcessActionResponse> {
    processes::set_paused(id, true);
    Json(ProcessActionResponse {
        ok: true,
        message: format!("Process #{} paused", id),
    })
}

pub async fn resume_process_handler(
    State(_state): State<AppState>,
    Path(id): Path<u64>,
) -> Json<ProcessActionResponse> {
    processes::set_paused(id, false);
    Json(ProcessActionResponse {
        ok: true,
        message: format!("Process #{} resumed", id),
    })
}

pub async fn stop_process_handler(
    State(_state): State<AppState>,
    Path(id): Path<u64>,
) -> Json<ProcessActionResponse> {
    processes::request_stop(id);
    Json(ProcessActionResponse {
        ok: true,
        message: format!("Stop requested for process #{}", id),
    })
}
