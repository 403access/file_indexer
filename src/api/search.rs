use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::offload;
use crate::modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind};
use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::get_connection;
use crate::modules::sql::search::{count_search_file, search_file_page};
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct SearchParams {
    pub name: Option<String>,
    #[serde(default = "default_type")]
    pub r#type: String,
    #[serde(default = "default_pattern")]
    pub pattern: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_order")]
    pub order: String,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_type() -> String {
    "both".to_string()
}
fn default_pattern() -> String {
    "contains".to_string()
}
fn default_sort() -> String {
    "name".to_string()
}
fn default_order() -> String {
    "asc".to_string()
}
fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    20
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<FileEntry>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
}

pub async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);

        let name = params.name.unwrap_or_default();
        let target_kind = match params.r#type.as_str() {
            "files" => TargetKind::Files,
            "folders" => TargetKind::Folders,
            _ => TargetKind::Both,
        };
        let pattern_kind = match params.pattern.as_str() {
            "exact" => PatternKind::Exact,
            "starts_with" => PatternKind::StartsWith,
            "ends_with" => PatternKind::EndsWith,
            _ => PatternKind::Contains,
        };
        let order_kind = match params.order.as_str() {
            "desc" => OrderKind::Desc,
            _ => OrderKind::Asc,
        };

        let page = params.page.max(1);
        let per_page = params.per_page.clamp(1, 200);
        let offset = (page - 1).saturating_mul(per_page);

        let conn = get_connection(&state.db)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Apply PRAGMA query_only for this read path (extra safety)
        let _ = conn.execute_batch("PRAGMA query_only=ON;");

        let total = count_search_file(&conn, &name, target_kind, pattern_kind)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            as usize;

        let entries = search_file_page(
            &conn,
            &name,
            target_kind,
            pattern_kind,
            order_kind,
            &params.sort,
            per_page,
            offset,
        )
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        Ok(Json(SearchResponse {
            results: entries,
            total,
            page,
            per_page,
        }))
    })
    .await
}
