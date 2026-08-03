use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::sql::database::get_connection;
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct DuplicateFoldersParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub min_shared: Option<u32>,
    #[serde(default)]
    pub min_folders: Option<u32>,
    #[serde(default)]
    pub file_types: Option<String>,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_order")]
    pub order: String,
}

fn default_page() -> u32 { 1 }
fn default_per_page() -> u32 { 20 }
fn default_sort() -> String { "shared".to_string() }
fn default_order() -> String { "desc".to_string() }

#[derive(Serialize)]
pub struct FolderInfo {
    pub path: String,
    pub name: String,
    pub file_count: usize,
}

#[derive(Serialize)]
pub struct FolderGroup {
    pub shared_count: usize,
    pub folders: Vec<FolderInfo>,
}

#[derive(Serialize)]
pub struct DuplicateFoldersResponse {
    pub groups: Vec<FolderGroup>,
    pub total_groups: usize,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Serialize)]
pub struct FolderFilesResponse {
    pub files: Vec<FolderFile>,
}

#[derive(Serialize)]
pub struct FolderFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub hash: String,
    pub is_duplicate: bool,
}

pub async fn duplicate_folders_handler(
    State(state): State<AppState>,
    Query(params): Query<DuplicateFoldersParams>,
) -> Result<Json<DuplicateFoldersResponse>, (axum::http::StatusCode, String)> {
    let _guard = IndexerPauseGuard::new(&state);
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Single query: get all files that have duplicate hashes, with their folder info
    let mut stmt = conn.prepare(
        "SELECT f.path, fn.name, f.hash
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE f.hash IN (SELECT hash FROM duplicate_hashes)
           AND f.hash IS NOT NULL AND f.hash != ''"
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut folder_hashes: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    let mut folder_names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut folder_file_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut hash_extension: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for row in rows {
        let (path, name, hash) = row.map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let normalized = path.replace("//", "/");
        // Remember the extension of this duplicate file (for file-type filtering)
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
        hash_extension.entry(hash.clone()).or_insert(ext);
        if let Some(parent) = normalized.rsplit_once('/') {
            let folder = parent.0.to_string();
            folder_hashes.entry(folder.clone()).or_default().insert(hash.clone());
            if let Some(display) = folder.rsplit_once('/') {
                folder_names.entry(folder.clone()).or_insert_with(|| display.1.to_string());
            }
            *folder_file_counts.entry(folder.clone()).or_insert(0) += 1;
        }
    }
    drop(stmt);

    // Phase 3: Group folders by shared hashes
    let mut folder_list: Vec<(String, std::collections::HashSet<String>)> = folder_hashes.into_iter().collect();
    folder_list.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    let mut groups: Vec<Vec<(String, std::collections::HashSet<String>)>> = Vec::new();
    let mut used: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for i in 0..folder_list.len() {
        if used.contains(&i) {
            continue;
        }
        let mut group = vec![folder_list[i].clone()];
        used.insert(i);

        for j in (i + 1)..folder_list.len() {
            if used.contains(&j) {
                continue;
            }
            let shares = group.iter().any(|g| g.1.intersection(&folder_list[j].1).next().is_some());
            if shares {
                group.push(folder_list[j].clone());
                used.insert(j);
            }
        }

        if group.len() >= 2 {
            groups.push(group);
        }
    }

    // Build metadata for every group so we can filter and sort before paginating
    struct GroupMeta {
        idx: usize,
        shared_count: usize,
        folder_count: usize,
        name: String,
        file_count: usize,
    }

    let q = params.q.as_ref().map(|s| s.to_lowercase());
    let min_shared = params.min_shared.unwrap_or(0) as usize;
    let min_folders = params.min_folders.unwrap_or(0) as usize;

    // Parsed list of requested file extensions for filtering (lowercased)
    let file_types: std::collections::HashSet<String> = params.file_types.as_ref()
        .map(|s| s.split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();

    let mut metas: Vec<GroupMeta> = Vec::new();
    for (gi, group) in groups.iter().enumerate() {
        // Number of hashes shared across every folder in the group
        let shared_count = if let Some(first) = group.first() {
            group.iter().skip(1).fold(first.1.clone(), |acc, f| {
                acc.intersection(&f.1).cloned().collect()
            }).len()
        } else {
            0
        };
        let folder_count = group.len();
        let file_count: usize = group.iter()
            .map(|(path, _)| folder_file_counts.get(path).copied().unwrap_or(0))
            .sum();
        let name = group.get(0)
            .and_then(|(path, _)| folder_names.get(path))
            .cloned()
            .unwrap_or_default();

        // Text filter: any folder name or path must contain the query
        if let Some(q) = &q {
            let matched = group.iter().any(|(path, _)| {
                let nm = folder_names.get(path).map(|s| s.to_lowercase()).unwrap_or_default();
                let p = path.to_lowercase();
                nm.contains(q) || p.contains(q)
            });
            if !matched {
                continue;
            }
        }

        // File-type filter: at least one duplicate file in the group must match
        // one of the requested extensions
        if !file_types.is_empty() {
            let has_match = group.iter().any(|(_, hashes)| hashes.iter().any(|h| {
                hash_extension.get(h).map(|e| file_types.contains(e)).unwrap_or(false)
            }));
            if !has_match {
                continue;
            }
        }

        if shared_count < min_shared || folder_count < min_folders {
            continue;
        }

        metas.push(GroupMeta { idx: gi, shared_count, folder_count, name, file_count });
    }

    // Sort
    let ascending = params.order == "asc";
    metas.sort_by(|a, b| {
        let ord = match params.sort.as_str() {
            "folders" => a.folder_count.cmp(&b.folder_count),
            "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            "files" => a.file_count.cmp(&b.file_count),
            _ => a.shared_count.cmp(&b.shared_count),
        };
        if ascending { ord } else { ord.reverse() }
    });

    let total_groups = metas.len();

    // Phase 4: Paginate over the filtered/sorted groups - only build full details for current page
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(total_groups);
    let page_metas = if start < total_groups { &metas[start..end] } else { &[] };

    let response_groups: Vec<FolderGroup> = page_metas.iter().map(|m| {
        let group = &groups[m.idx];
        let folders: Vec<FolderInfo> = group.iter().map(|(path, _hashes)| {
            let display_name = folder_names.get(path).cloned().unwrap_or_else(|| {
                path.rsplit_once('/').map(|s| s.1.to_string()).unwrap_or_default()
            });
            let file_count = folder_file_counts.get(path).copied().unwrap_or(0);

            FolderInfo { path: path.clone(), name: display_name, file_count }
        }).collect();

        FolderGroup {
            shared_count: m.shared_count,
            folders,
        }
    }).collect();

    Ok(Json(DuplicateFoldersResponse {
        groups: response_groups,
        total_groups,
        page: params.page,
        per_page: params.per_page,
    }))
}

/// Load files for a specific folder in a duplicate group.
pub async fn folder_files_handler(
    State(state): State<AppState>,
    Query(params): Query<FolderFilesParams>,
) -> Result<Json<FolderFilesResponse>, (axum::http::StatusCode, String)> {
    let _guard = IndexerPauseGuard::new(&state);
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get all duplicate hashes for reference
    let dup_hashes: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare("SELECT hash FROM duplicate_hashes")
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let result: Vec<String> = stmt.query_map([], |row| row.get(0))
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        result.into_iter().collect()
    };

    // Get files for this folder
    let files: Vec<FolderFile> = {
        let mut stmt = conn.prepare(
            "SELECT f.path, fn.name, f.size, f.hash
             FROM files f
             JOIN file_names fn ON f.file_name_id = fn.id
             WHERE f.parent_path = ?1 AND f.hash IS NOT NULL"
        ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let result: Vec<FolderFile> = stmt.query_map(rusqlite::params![params.path], |row| {
            Ok(FolderFile {
                path: row.get(0)?,
                name: row.get(1)?,
                size: row.get(2)?,
                hash: row.get(3)?,
                is_duplicate: false,
            })
        })
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
        drop(stmt);

        result.into_iter().map(|mut f| {
            f.is_duplicate = dup_hashes.contains(&f.hash);
            f
        }).collect()
    };

    Ok(Json(FolderFilesResponse { files }))
}

#[derive(Deserialize)]
pub struct FolderFilesParams {
    pub path: String,
}

#[derive(Deserialize)]
pub struct CheckFoldersRequest {
    pub paths: Vec<String>,
}

#[derive(Serialize)]
pub struct FolderCheckResult {
    pub path: String,
    pub resolved: String,
    pub exists: bool,
    pub is_dir: bool,
}

#[derive(Serialize)]
pub struct CheckFoldersResponse {
    pub results: Vec<FolderCheckResult>,
}

/// Check whether the given folder paths exist on disk.
/// Relative paths are resolved against the working directory (matching the
/// behaviour used by the merge handler).
pub async fn check_folders_handler(
    State(state): State<AppState>,
    Json(req): Json<CheckFoldersRequest>,
) -> Result<Json<CheckFoldersResponse>, (axum::http::StatusCode, String)> {
    let cwd = state.cwd.trim_end_matches('/');

    let results: Vec<FolderCheckResult> = req
        .paths
        .iter()
        .map(|raw| {
            let normalized = raw.replace("//", "/");
            let resolved = if normalized.starts_with('/') {
                normalized
            } else {
                format!("{}/{}", cwd, normalized.trim_start_matches('/'))
            };
            let exists = std::path::Path::new(&resolved).exists();
            let is_dir = std::path::Path::new(&resolved).is_dir();
            FolderCheckResult {
                path: raw.clone(),
                resolved,
                exists,
                is_dir,
            }
        })
        .collect();

    Ok(Json(CheckFoldersResponse { results }))
}

#[derive(Serialize)]
pub struct AvailableFileTypesResponse {
    pub types: Vec<String>,
}

/// Return the distinct file extensions present among files that have duplicate hashes.
/// Used to populate the file-type selector on the duplicate folders page.
pub async fn available_file_types_handler(
    State(state): State<AppState>,
) -> Result<Json<AvailableFileTypesResponse>, (axum::http::StatusCode, String)> {
    let _guard = IndexerPauseGuard::new(&state);
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut stmt = conn.prepare(
        "SELECT fn.name
         FROM files f
         JOIN file_names fn ON f.file_name_id = fn.id
         WHERE f.hash IN (SELECT hash FROM duplicate_hashes)
           AND f.hash IS NOT NULL AND f.hash != ''"
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rows = stmt.query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for row in rows {
        if let Ok(name) = row {
            if let Some((_, ext)) = name.rsplit_once('.') {
                let ext = ext.to_lowercase();
                if !ext.is_empty() {
                    set.insert(ext);
                }
            }
        }
    }
    drop(stmt);

    Ok(Json(AvailableFileTypesResponse { types: set.into_iter().collect() }))
}
