use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::offload;
use crate::modules::sql::database::get_connection;
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct FolderParams {
    pub path: String,
}

#[derive(Serialize)]
pub struct FolderResponse {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: Option<u64>,
    pub file_count: usize,
    pub folder_count: usize,
    pub files: Vec<FolderEntry>,
}

#[derive(Serialize)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: Option<u64>,
    pub is_directory: bool,
    pub is_file: bool,
    pub hash: Option<String>,
}

pub async fn folder_handler(
    State(state): State<AppState>,
    Query(params): Query<FolderParams>,
) -> Result<Json<FolderResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
    let _guard = IndexerPauseGuard::new(&state);
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let path = params.path.replace("//", "/");
    let path = path.trim_end_matches('/');

    // Get folder metadata. The configured root may not be stored as a folder
    // row (only its contents are), so synthesize metadata for it.
    let folder = match conn.query_row(
        "SELECT f.path, fn.name, f.size, f.modified
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE f.path = ?1 AND f.is_directory = 1",
        [path],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, Option<f64>>(3)?.map(|v| v as u64),
            ))
        },
    ) {
        Ok(f) => f,
        Err(_) => {
            if path == state.cwd.trim_end_matches('/') {
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string());
                (path.to_string(), name, 0u64, None)
            } else {
                return Err((
                    axum::http::StatusCode::NOT_FOUND,
                    format!("Folder not found"),
                ));
            }
        }
    };

    // Get children
    let mut stmt = conn.prepare(
        "SELECT f.path, fn.name, f.size, f.modified, f.is_directory, f.is_file, f.hash
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE f.parent_path = ?1
         ORDER BY f.is_directory DESC, fn.name ASC"
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let entries: Vec<FolderEntry> = stmt.query_map([path], |row| {
        let modified_f64: Option<f64> = row.get(3)?;
        Ok(FolderEntry {
            path: row.get(0)?,
            name: row.get(1)?,
            size: row.get(2)?,
            modified: modified_f64.map(|v| v as u64),
            is_directory: row.get::<_, i32>(4)? != 0,
            is_file: row.get::<_, i32>(5)? != 0,
            hash: row.get(6)?,
        })
    })
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .filter_map(|r| r.ok())
    .collect();

    let file_count = entries.iter().filter(|e| e.is_file).count();
    let folder_count = entries.iter().filter(|e| e.is_directory).count();

    Ok(Json(FolderResponse {
        path: folder.0,
        name: folder.1,
        size: folder.2,
        modified: folder.3,
        file_count,
        folder_count,
        files: entries,
    }))
    }).await
}
