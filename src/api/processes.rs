use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use crate::modules::processes::{self, Process};
use crate::modules::sql::database::get_connection;
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
