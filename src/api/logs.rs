use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::sql::database::{get_connection, get_logs};
use crate::states::app_state::AppState;

#[derive(Deserialize)]
pub struct LogsParams {
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
}

pub async fn logs_handler(
    State(state): State<AppState>,
    Query(params): Query<LogsParams>,
) -> Json<Vec<LogEntry>> {
    let limit = params.limit.unwrap_or(1000);
    let conn = get_connection(&state.db).unwrap();
    let logs = get_logs(&conn, limit).unwrap_or_default();
    Json(
        logs.into_iter()
            .map(|(timestamp, level, message)| LogEntry { timestamp, level, message })
            .collect(),
    )
}
