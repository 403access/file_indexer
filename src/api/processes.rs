use axum::Json;
use serde::Serialize;

use crate::modules::processes::{self, Process};

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
