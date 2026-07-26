use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::modules::sql::database::{get_connection, get_skipped_paths};
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Serialize)]
pub struct SkippedPath {
    path: String,
    error: String,
}

pub async fn skipped_handler(State(state): State<AppState>) -> Json<Vec<SkippedPath>> {
    let _guard = IndexerPauseGuard::new(&state);
    let conn = get_connection(&state.db).unwrap();
    let skipped = get_skipped_paths(&conn).unwrap_or_default();
    Json(
        skipped
            .into_iter()
            .map(|(path, error)| SkippedPath { path, error })
            .collect(),
    )
}
