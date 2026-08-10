use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::offload;
use crate::modules::sql::database::{
    count_skipped_paths, get_connection, get_skipped_paths_page,
};
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct SkippedParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    50
}

#[derive(Serialize)]
pub struct SkippedPath {
    path: String,
    error: String,
}

#[derive(Serialize)]
pub struct SkippedResponse {
    pub results: Vec<SkippedPath>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

pub async fn skipped_handler(
    State(state): State<AppState>,
    Query(params): Query<SkippedParams>,
) -> Result<Json<SkippedResponse>, (axum::http::StatusCode, String)> {
    let page = params.page.max(1);
    let per_page = params.per_page.clamp(1, 200);
    let offset = (page - 1).saturating_mul(per_page);

    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;

        let total = count_skipped_paths(&conn).unwrap_or(0);
        let skipped = get_skipped_paths_page(&conn, per_page, offset).unwrap_or_default();

        Ok(Json(SkippedResponse {
            results: skipped
                .into_iter()
                .map(|(path, error)| SkippedPath { path, error })
                .collect(),
            total,
            page,
            per_page,
        }))
    })
    .await
}
