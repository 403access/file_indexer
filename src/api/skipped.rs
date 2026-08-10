use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::offload;
use crate::modules::sql::database::{
    count_skipped_paths_filtered, get_connection, get_skipped_paths_page_filtered,
};
use crate::modules::sql::skipped::{SkippedMatchField, SkippedSortField};
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct SkippedParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    /// Free-text search (path and/or error, depending on `match_field`)
    pub q: Option<String>,
    /// Which fields to search: both | path | error
    #[serde(default = "default_match")]
    pub r#match: String,
    /// Sort column: path | error
    #[serde(default = "default_sort")]
    pub sort: String,
    /// asc | desc
    #[serde(default = "default_order")]
    pub order: String,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    50
}

fn default_match() -> String {
    "both".to_string()
}

fn default_sort() -> String {
    "path".to_string()
}

fn default_order() -> String {
    "asc".to_string()
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
    let q = params.q.clone();
    let match_field = match params.r#match.as_str() {
        "path" => SkippedMatchField::Path,
        "error" => SkippedMatchField::Error,
        _ => SkippedMatchField::Both,
    };
    let sort = match params.sort.as_str() {
        "error" => SkippedSortField::Error,
        _ => SkippedSortField::Path,
    };
    let asc = params.order != "desc";

    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;

        let q_ref = q.as_deref().filter(|s| !s.trim().is_empty());
        let total = count_skipped_paths_filtered(&conn, q_ref, match_field).unwrap_or(0);
        let skipped = get_skipped_paths_page_filtered(
            &conn,
            per_page,
            offset,
            q_ref,
            match_field,
            sort,
            asc,
        )
        .unwrap_or_default();

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
