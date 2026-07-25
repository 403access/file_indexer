use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::{get_connection, reset_duplicates_table};
use crate::states::app_state::AppState;

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
    let mut conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let tx = conn.transaction()
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    reset_duplicates_table(&tx)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get all duplicate hashes
    let mut stmt = tx.prepare(
        "SELECT hash FROM duplicate_hashes"
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let hashes: Vec<String> = stmt.query_map([], |row| row.get(0))
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let total_groups = hashes.len();

    // Paginate hashes
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(total_groups);
    let page_hashes = if start < total_groups { &hashes[start..end] } else { &[] };

    // For each hash, get the files
    let mut groups = Vec::new();
    for hash in page_hashes {
        let mut stmt = tx.prepare(
            "SELECT f.path, fn.name, f.size, f.modified, f.hash,
                    f.is_directory, f.is_file, f.is_symlink
             FROM files f
             JOIN file_names fn ON f.file_name_id = fn.id
             WHERE f.hash = ?"
        ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let h = hash.as_str();
        let mut rows = stmt.query(rusqlite::params![h])
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("query error for hash {}: {}", h, e)))?;

        let mut files = Vec::new();
        while let Some(row) = rows.next().map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))? {
            let path: Option<String> = row.get(0).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("col 0: {}", e)))?;
            let name: String = row.get(1).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("col 1: {}", e)))?;
            let size: u64 = row.get(2).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("col 2: {}", e)))?;
            let modified_f64: Option<f64> = row.get(3).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("col 3: {}", e)))?;
            let modified = modified_f64.map(|v| v as u64);
            let hash_val: Option<String> = row.get(4).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("col 4: {}", e)))?;
            let is_directory: bool = row.get::<_, i32>(5).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("col 5: {}", e)))? != 0;
            let is_file: bool = row.get::<_, i32>(6).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("col 6: {}", e)))? != 0;
            let is_symlink: bool = row.get::<_, i32>(7).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("col 7: {}", e)))? != 0;

            files.push(FileEntry {
                path, name, size, modified, hash: hash_val,
                is_directory, is_file, is_symlink,
                created: None,
                accessed: None,
            });
        }
        drop(rows);
        drop(stmt);

        // wasted = (count - 1) * size of first file
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

    tx.commit()
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(DuplicatesResponse {
        groups,
        total_groups,
        page: params.page,
        per_page: params.per_page,
    }))
}
