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
pub struct ExplorerParams {
    #[serde(default = "default_type")]
    pub r#type: String,
    pub q: Option<String>,
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
    50
}

#[derive(Serialize)]
pub struct ExplorerResponse {
    pub results: Vec<FileEntry>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
}

pub async fn explorer_handler(
    State(state): State<AppState>,
    Query(params): Query<ExplorerParams>,
) -> Result<Json<ExplorerResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);

        let name = params.q.unwrap_or_default().trim().to_string();
        let target_kind = match params.r#type.as_str() {
            "files" => TargetKind::Files,
            "folders" => TargetKind::Folders,
            _ => TargetKind::Both,
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

        let total = count_search_file(&conn, &name, target_kind, PatternKind::Contains)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            as usize;

        let results = search_file_page(
            &conn,
            &name,
            target_kind,
            PatternKind::Contains,
            order_kind,
            &params.sort,
            per_page,
            offset,
        )
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        Ok(Json(ExplorerResponse {
            results,
            total,
            page,
            per_page,
        }))
    })
    .await
}