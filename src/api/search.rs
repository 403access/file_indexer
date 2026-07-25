use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::commands::command_search_file::{OrderKind, PatternKind, TargetKind};
use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::get_connection;
use crate::modules::sql::search::search_file;
use crate::states::app_state::AppState;

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

fn default_type() -> String { "both".to_string() }
fn default_pattern() -> String { "contains".to_string() }
fn default_sort() -> String { "name".to_string() }
fn default_order() -> String { "asc".to_string() }
fn default_page() -> u32 { 1 }
fn default_per_page() -> u32 { 20 }

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

    let mut conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let tx = conn.transaction()
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut entries = search_file(&tx, &name, target_kind, pattern_kind, order_kind)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tx.commit()
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Sort by the requested column
    match params.sort.as_str() {
        "size" => entries.sort_by(|a, b| a.size.cmp(&b.size)),
        "modified" => entries.sort_by(|a, b| a.modified.cmp(&b.modified)),
        "path" => entries.sort_by(|a, b| a.path.cmp(&b.path)),
        _ => entries.sort_by(|a, b| a.name.cmp(&b.name)),
    }
    if params.order == "desc" {
        entries.reverse();
    }

    let total = entries.len();
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(total);
    let paginated = if start < total { entries[start..end].to_vec() } else { vec![] };

    Ok(Json(SearchResponse {
        results: paginated,
        total,
        page: params.page,
        per_page: params.per_page,
    }))
}
