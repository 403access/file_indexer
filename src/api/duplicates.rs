use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::get_connection;
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct DuplicatesParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 { 1 }
fn default_per_page() -> u32 { 20 }

#[derive(Serialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub files: Vec<FileEntry>,
    pub wasted_bytes: u64,
}

#[derive(Serialize)]
pub struct DuplicatesResponse {
    pub groups: Vec<DuplicateGroup>,
    pub total_groups: usize,
    pub page: u32,
    pub per_page: u32,
}

pub async fn duplicates_handler(
    State(state): State<AppState>,
    Query(params): Query<DuplicatesParams>,
) -> Result<Json<DuplicatesResponse>, (axum::http::StatusCode, String)> {
    let _guard = IndexerPauseGuard::new(&state);

    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Count total duplicate groups from maintained table
    let total_groups: usize = conn
        .query_row("SELECT COUNT(*) FROM duplicate_hashes", [], |r| r.get(0))
        .unwrap_or(0);

    // Get hashes for this page
    let offset = ((params.page - 1) * params.per_page) as i64;
    let limit = params.per_page as i64;

    let mut stmt = conn
        .prepare("SELECT hash FROM duplicate_hashes LIMIT ?1 OFFSET ?2")
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let page_hashes: Vec<String> = stmt
        .query_map(rusqlite::params![limit, offset], |row| row.get(0))
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut groups = Vec::with_capacity(page_hashes.len());
    for hash in &page_hashes {
        let mut stmt = conn
            .prepare(
                "SELECT f.path, fn.name, f.size, f.modified, f.hash,
                        f.is_directory, f.is_file, f.is_symlink
                 FROM files f
                 JOIN file_names fn ON f.file_name_id = fn.id
                 WHERE f.hash = ?1 AND f.is_file = 1",
            )
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let files: Vec<FileEntry> = stmt
            .query_map(rusqlite::params![hash], |row| {
                let modified_f64: Option<f64> = row.get(3)?;
                Ok(FileEntry {
                    path: row.get(0)?,
                    name: row.get(1)?,
                    size: row.get(2)?,
                    modified: modified_f64.map(|v| v as u64),
                    hash: row.get(4)?,
                    is_directory: row.get::<_, i32>(5)? != 0,
                    is_file: row.get::<_, i32>(6)? != 0,
                    is_symlink: row.get::<_, i32>(7)? != 0,
                    created: None,
                    accessed: None,
                    parent_path: None,
                })
            })
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        let wasted = if files.len() > 1 {
            (files.len() as u64 - 1) * files[0].size
        } else {
            0
        };

        groups.push(DuplicateGroup {
            hash: hash.clone(),
            files,
            wasted_bytes: wasted,
        });
    }

    Ok(Json(DuplicatesResponse {
        groups,
        total_groups,
        page: params.page,
        per_page: params.per_page,
    }))
}
