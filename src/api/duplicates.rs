use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::modules::file_entry::_types::FileEntry;
use crate::api::offload;
use crate::modules::sql::database::get_connection;
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct DuplicatesParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub min_size: Option<u64>,
    #[serde(default)]
    pub ext: Option<String>,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_order")]
    pub order: String,
}

fn default_page() -> u32 { 1 }
fn default_per_page() -> u32 { 20 }
fn default_sort() -> String { "wasted".to_string() }
fn default_order() -> String { "desc".to_string() }

#[derive(Clone, Serialize)]
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
    offload(move || {
    let _guard = IndexerPauseGuard::new(&state);

    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let q = params.q.as_ref().map(|s| s.to_lowercase());
    let min_size = params.min_size.unwrap_or(0);
    let ext_filter: Vec<String> = params.ext.as_ref()
        .map(|s| s.split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();

    let mut stmt = conn.prepare(
        "SELECT f.path, fn.name, f.size, f.modified, f.hash,
                f.is_directory, f.is_file, f.is_symlink
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE f.hash IN (SELECT hash FROM duplicate_hashes)
           AND f.is_file = 1"
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rows = stmt.query_map([], |row| {
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
    }).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let all_files: Vec<FileEntry> = rows.filter_map(|r| r.ok()).collect();
    drop(stmt);

    let mut groups_map: HashMap<String, Vec<FileEntry>> = HashMap::new();
    for file in all_files {
        groups_map.entry(file.hash.clone().unwrap_or_default()).or_default().push(file);
    }

    let mut filtered_groups: Vec<DuplicateGroup> = groups_map.into_iter().filter_map(|(hash, files)| {
        if let Some(ref q_str) = q {
            let matched = files.iter().any(|f| {
                f.name.to_lowercase().contains(q_str) || f.path.clone().unwrap_or_default().to_lowercase().contains(q_str)
            });
            if !matched { return None; }
        }

        if min_size > 0 {
            let matched = files.iter().any(|f| f.size >= min_size);
            if !matched { return None; }
        }

        if !ext_filter.is_empty() {
            let matched = files.iter().any(|f| {
                f.name.rsplit_once('.').map(|(_, e)| ext_filter.contains(&e.to_lowercase())).unwrap_or(false)
            });
            if !matched { return None; }
        }

        let wasted = if files.len() > 1 {
            (files.len() as u64 - 1) * files[0].size
        } else {
            0
        };

        Some(DuplicateGroup {
            hash,
            files,
            wasted_bytes: wasted,
        })
    }).collect();

    let ascending = params.order == "asc";
    match params.sort.as_str() {
        "size" => {
            filtered_groups.sort_by(|a, b| {
                let ord = a.files[0].size.cmp(&b.files[0].size);
                if ascending { ord } else { ord.reverse() }
            });
        }
        "copies" => {
            filtered_groups.sort_by(|a, b| {
                let ord = a.files.len().cmp(&b.files.len());
                if ascending { ord } else { ord.reverse() }
            });
        }
        "hash" => {
            filtered_groups.sort_by(|a, b| {
                let ord = a.hash.cmp(&b.hash);
                if ascending { ord } else { ord.reverse() }
            });
        }
        _ => {
            filtered_groups.sort_by(|a, b| {
                let ord = a.wasted_bytes.cmp(&b.wasted_bytes);
                if ascending { ord } else { ord.reverse() }
            });
        }
    }

    let total_groups = filtered_groups.len();
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(total_groups);
    let page_groups = if start < total_groups { filtered_groups[start..end].to_vec() } else { vec![] };

    Ok(Json(DuplicatesResponse {
        groups: page_groups,
        total_groups,
        page: params.page,
        per_page: params.per_page,
    }))
    }).await
}
