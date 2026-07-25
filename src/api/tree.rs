use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::file_entry::_types::FileEntry;
use crate::modules::sql::database::get_connection;
use crate::states::app_state::AppState;

#[derive(Deserialize)]
pub struct TreeParams {
    pub path: Option<String>,
}

#[derive(Serialize)]
pub struct TreeNode {
    pub entry: FileEntry,
    pub children: Vec<TreeNode>,
}

#[derive(Serialize)]
pub struct TreeResponse {
    pub root: Vec<TreeNode>,
}

pub async fn tree_handler(
    State(state): State<AppState>,
    Query(params): Query<TreeParams>,
) -> Result<Json<TreeResponse>, (axum::http::StatusCode, String)> {
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sql = "
        SELECT f.path, fn.name, f.size, f.modified, f.hash,
               f.is_directory, f.is_file, f.is_symlink
        FROM files f
        JOIN file_names fn ON f.file_name_id = fn.id
        ORDER BY f.is_directory DESC, fn.name ASC
    ";

    let mut stmt = conn.prepare(sql)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = stmt.query([])
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut all_entries: Vec<FileEntry> = Vec::new();
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

        all_entries.push(FileEntry {
            path, name, size, modified, hash: hash_val,
            is_directory, is_file, is_symlink,
            created: None, accessed: None,
        });
    }
    drop(rows);
    drop(stmt);

    // Determine the root path
    let root_prefix = if let Some(ref p) = params.path {
        p.trim_end_matches('/').to_string()
    } else {
        state.cwd.trim_end_matches('/').to_string()
    };

    // Find root-level entries: whose parent directory is root_prefix
    let root_entries: Vec<FileEntry> = all_entries.iter().filter(|e| {
        let normalized = e.path.as_deref().unwrap_or("").replace("//", "/");
        if let Some(parent) = normalized.rsplit_once('/') {
            let parent_path = parent.0.to_string();
            parent_path == root_prefix
        } else {
            false
        }
    }).cloned().collect();

    // For each entry, find its direct parent
    let mut children_map: std::collections::HashMap<String, Vec<FileEntry>> = std::collections::HashMap::new();

    for entry in &all_entries {
        let normalized = entry.path.as_deref().unwrap_or("").replace("//", "/");
        if let Some(parent_dir) = normalized.rsplit_once('/') {
            let parent = parent_dir.0.to_string();
            if parent != root_prefix {
                children_map.entry(parent).or_default().push(entry.clone());
            }
        }
    }

    // Build tree recursively
    fn build_node(entry: FileEntry, children_map: &std::collections::HashMap<String, Vec<FileEntry>>) -> TreeNode {
        let entry_path = entry.path.as_deref().unwrap_or("").replace("//", "/");
        let mut children_raw = children_map.get(&entry_path).cloned().unwrap_or_default();

        // Sort children: directories first, then by name
        children_raw.sort_by(|a, b| {
            b.is_directory.cmp(&a.is_directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let mut children: Vec<TreeNode> = Vec::new();
        if entry.is_directory {
            for child in &children_raw {
                children.push(build_node(child.clone(), children_map));
            }
        }

        TreeNode { entry, children }
    }

    let mut root: Vec<TreeNode> = root_entries.into_iter()
        .map(|e| build_node(e, &children_map))
        .collect();

    // Sort: directories first, then by name
    root.sort_by(|a, b| {
        b.entry.is_directory.cmp(&a.entry.is_directory)
            .then_with(|| a.entry.name.to_lowercase().cmp(&b.entry.name.to_lowercase()))
    });

    Ok(Json(TreeResponse { root }))
}
