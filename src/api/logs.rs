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
    Json(
        logs.into_iter()
            .map(|(timestamp, level, message)| LogEntry { timestamp, level, message })
            .collect(),
    )
}
