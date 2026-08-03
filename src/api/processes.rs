use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use crate::modules::processes::{self, Process};
use crate::states::app_state::AppState;

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
