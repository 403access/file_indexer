use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    pub traversed: bool,
    pub traverse_error: Option<String>,
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
    pub traversed: bool,
    pub traverse_error: Option<String>,
    /// Whether this entry is connected to duplicates: for a folder, its
    /// subtree contains at least one duplicate file; for a file, it is itself
    /// a duplicate (its hash appears in `duplicate_hashes`).
    pub has_duplicates: bool,
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
        "SELECT f.path, fn.name, f.size, f.modified, f.traversed, f.traverse_error
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
                row.get::<_, i32>(4)? != 0,
                row.get(5)?,
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
                let traversed = conn
                    .query_row(
                        "SELECT COUNT(*) FROM files WHERE path = ?1 AND is_directory = 1 AND traversed = 1",
                        [path],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap_or(0)
                    > 0;
                (path.to_string(), name, 0u64, None, traversed, None)
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
        "SELECT f.path, fn.name, f.size, f.modified, f.is_directory, f.is_file, f.hash, f.traversed, f.traverse_error
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE f.parent_path = ?1
         ORDER BY f.is_directory DESC, fn.name ASC"
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut entries: Vec<FolderEntry> = stmt.query_map([path], |row| {
        let modified_f64: Option<f64> = row.get(3)?;
        Ok(FolderEntry {
            path: row.get(0)?,
            name: row.get(1)?,
            size: row.get(2)?,
            modified: modified_f64.map(|v| v as u64),
            is_directory: row.get::<_, i32>(4)? != 0,
            is_file: row.get::<_, i32>(5)? != 0,
            hash: row.get(6)?,
            traversed: row.get::<_, i32>(7)? != 0,
            traverse_error: row.get(8)?,
            has_duplicates: false,
        })
    })
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .filter_map(|r| r.ok())
    .collect();

    // Flag entries connected to duplicates:
    //  - a file is a duplicate if its hash is in duplicate_hashes
    //  - a folder contains duplicates if any file in its subtree is duplicated
    let (dup_files, dup_dirs) = duplicate_flags(&conn, path);
    for entry in &mut entries {
        entry.has_duplicates = if entry.is_directory {
            dup_dirs.contains(&entry.path)
        } else {
            dup_files.contains(&entry.path)
        };
    }

    let file_count = entries.iter().filter(|e| e.is_file).count();
    let folder_count = entries.iter().filter(|e| e.is_directory).count();

    Ok(Json(FolderResponse {
        path: folder.0,
        name: folder.1,
        size: folder.2,
        modified: folder.3,
        traversed: folder.4,
        traverse_error: folder.5,
        file_count,
        folder_count,
        files: entries,
    }))
    }).await
}

/// Returns two path sets for the children of `parent_path`:
/// `(duplicate files, folders that contain duplicates in their subtree)`.
///
/// A file is a duplicate when its hash appears in `duplicate_hashes`. A folder
/// is flagged when any file anywhere below it is a duplicate, so the user can
/// drill into a folder from the folder view and keep finding the duplicated
/// files. Guarded: if the `duplicate_hashes` table doesn't exist yet (fresh
/// DB), both sets come back empty.
fn duplicate_flags(
    conn: &rusqlite::Connection,
    parent_path: &str,
) -> (HashSet<String>, HashSet<String>) {
    let has_table: bool = conn
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master WHERE type='table' AND name='duplicate_hashes'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !has_table {
        return (HashSet::new(), HashSet::new());
    }

    let mut dup_files: HashSet<String> = HashSet::new();
    let dup_file_query = conn.prepare(
        "SELECT f.path
         FROM files f INDEXED BY idx_files_parent_path
         JOIN duplicate_hashes d ON d.hash = f.hash
         WHERE f.parent_path = ?1 AND f.is_file = 1",
    );
    if let Ok(mut stmt) = dup_file_query {
        if let Ok(rows) = stmt.query_map([parent_path], |row| row.get::<_, String>(0)) {
            dup_files = rows.filter_map(|r| r.ok()).collect();
        }
    }

    let mut dup_dirs: HashSet<String> = HashSet::new();
    // Instead of recursing the whole subtree (which is O(subtree size) and
    // planning-fragile in SQLite for deep trees), walk only the duplicate
    // files below `parent_path` and bucket each one under its direct child.
    // Any direct child reached this way contains a duplicate somewhere below it.
    let dup_dir_query = conn.prepare(
        "SELECT DISTINCT ?1 || '/' || substr(f.path, length(?1) + 2,
                              instr(substr(f.path, length(?1) + 2), '/') - 1)
         FROM files f INDEXED BY idx_files_path
         JOIN duplicate_hashes dh ON dh.hash = f.hash AND f.is_file = 1
         WHERE f.path >= ?1 || '/' AND f.path < ?1 || char(127)
           AND instr(substr(f.path, length(?1) + 2), '/') > 0",
    );
    if let Ok(mut stmt) = dup_dir_query {
        if let Ok(rows) = stmt.query_map([parent_path], |row| row.get::<_, String>(0)) {
            dup_dirs = rows.filter_map(|r| r.ok()).collect();
        }
    }

    (dup_files, dup_dirs)
}
