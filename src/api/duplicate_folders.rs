use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::offload;
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
    pub min_size: Option<u64>,
    /// Reserved: materialization does not store per-hash extensions yet.
    #[serde(default)]
    pub file_types: Option<String>,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_order")]
    pub order: String,
}

fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    20
}
fn default_sort() -> String {
    "shared".to_string()
}
fn default_order() -> String {
    "desc".to_string()
}

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
    /// True when the materialization table is empty (run a refresh process).
    #[serde(default)]
    pub needs_refresh: bool,
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

/// Serve duplicate folder groups from the **materialized** table.
///
/// Previously this recomputed clusters from every duplicate file on each request
/// (O(n²) over folders) which hung the API on large indexes.
pub async fn duplicate_folders_handler(
    State(state): State<AppState>,
    Query(params): Query<DuplicateFoldersParams>,
) -> Result<Json<DuplicateFoldersResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;

        let page = params.page.max(1);
        let per_page = params.per_page.clamp(1, 100);
        let offset = (page - 1).saturating_mul(per_page);
        let min_shared = params.min_shared.unwrap_or(0) as i64;
        let min_folders = params.min_folders.unwrap_or(2).max(2) as i64; // groups need ≥2 folders
        // If client sends min_folders=0 treat as 2
        let min_folders = if params.min_folders.unwrap_or(0) == 0 {
            2
        } else {
            min_folders
        };
        let min_size = params.min_size.unwrap_or(0) as i64;

        let q = params
            .q
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let q_pat = q.as_ref().map(|s| format!("%{s}%"));

        // Quick empty check
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM duplicate_folder_groups",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if row_count == 0 {
            return Ok(Json(DuplicateFoldersResponse {
                groups: vec![],
                total_groups: 0,
                page,
                per_page,
                needs_refresh: true,
            }));
        }

        let order = if params.order == "asc" { "ASC" } else { "DESC" };
        let sort_expr = match params.sort.as_str() {
            "folders" => "folder_count",
            "name" => "name COLLATE NOCASE",
            "files" => "file_count",
            _ => "shared_count",
        };

        // Filter groups that match q on any folder path/name, then aggregate.
        // Using a CTE keeps the plan readable and lets SQLite paginate groups only.
        let (count_sql, list_sql) = if q_pat.is_some() {
            (
                format!(
                    "SELECT COUNT(*) FROM (
                        SELECT g.group_id
                        FROM duplicate_folder_groups g
                        WHERE g.group_id IN (
                            SELECT group_id FROM duplicate_folder_groups
                            WHERE folder_name LIKE ?1 COLLATE NOCASE
                               OR folder_path LIKE ?1 COLLATE NOCASE
                        )
                        GROUP BY g.group_id
                        HAVING MAX(g.shared_count) >= ?2 AND COUNT(*) >= ?3 AND MIN(g.min_size) >= ?4
                    )"
                ),
                format!(
                    "SELECT g.group_id,
                            MAX(g.shared_count) AS shared_count,
                            COUNT(*) AS folder_count,
                            SUM(g.file_count) AS file_count,
                            MIN(g.folder_name) AS name
                     FROM duplicate_folder_groups g
                     WHERE g.group_id IN (
                         SELECT group_id FROM duplicate_folder_groups
                         WHERE folder_name LIKE ?1 COLLATE NOCASE
                            OR folder_path LIKE ?1 COLLATE NOCASE
                     )
                     GROUP BY g.group_id
                     HAVING MAX(g.shared_count) >= ?2 AND COUNT(*) >= ?3 AND MIN(g.min_size) >= ?4
                     ORDER BY {sort_expr} {order}
                     LIMIT ?5 OFFSET ?6"
                ),
            )
        } else {
            (
                "SELECT COUNT(*) FROM (
                    SELECT group_id
                    FROM duplicate_folder_groups
                    GROUP BY group_id
                    HAVING MAX(shared_count) >= ?1 AND COUNT(*) >= ?2 AND MIN(min_size) >= ?3
                )"
                .to_string(),
                format!(
                    "SELECT group_id,
                            MAX(shared_count) AS shared_count,
                            COUNT(*) AS folder_count,
                            SUM(file_count) AS file_count,
                            MIN(folder_name) AS name
                     FROM duplicate_folder_groups
                     GROUP BY group_id
                     HAVING MAX(shared_count) >= ?1 AND COUNT(*) >= ?2 AND MIN(min_size) >= ?3
                     ORDER BY {sort_expr} {order}
                     LIMIT ?4 OFFSET ?5"
                ),
            )
        };

        let total_groups: usize = if let Some(ref pat) = q_pat {
            conn.query_row(
                &count_sql,
                rusqlite::params![pat, min_shared, min_folders, min_size],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0) as usize
        } else {
            conn.query_row(
                &count_sql,
                rusqlite::params![min_shared, min_folders, min_size],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0) as usize
        };

        let mut group_ids: Vec<(String, usize)> = Vec::new();
        {
            let mut stmt = conn.prepare(&list_sql).map_err(|e| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string(),
                )
            })?;

            let map_row = |row: &rusqlite::Row<'_>| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as usize,
                ))
            };

            if let Some(ref pat) = q_pat {
                let rows = stmt
                    .query_map(
                        rusqlite::params![pat, min_shared, min_folders, min_size, per_page, offset],
                        map_row,
                    )
                    .map_err(|e| {
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            e.to_string(),
                        )
                    })?;
                for row in rows {
                    group_ids.push(row.map_err(|e| {
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            e.to_string(),
                        )
                    })?);
                }
            } else {
                let rows = stmt
                    .query_map(
                        rusqlite::params![min_shared, min_folders, min_size, per_page, offset],
                        map_row,
                    )
                    .map_err(|e| {
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            e.to_string(),
                        )
                    })?;
                for row in rows {
                    group_ids.push(row.map_err(|e| {
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            e.to_string(),
                        )
                    })?);
                }
            }
        }

        // Load folder rows for the page's group_ids
        let mut response_groups: Vec<FolderGroup> = Vec::with_capacity(group_ids.len());
        let mut stmt = conn
            .prepare(
                "SELECT folder_path, folder_name, file_count, shared_count
                 FROM duplicate_folder_groups
                 WHERE group_id = ?1
                 ORDER BY folder_path",
            )
            .map_err(|e| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string(),
                )
            })?;

        for (group_id, shared_count) in group_ids {
            let mut folders = Vec::new();
            let rows = stmt
                .query_map(rusqlite::params![group_id], |row| {
                    Ok(FolderInfo {
                        path: row.get(0)?,
                        name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        file_count: row.get::<_, i64>(2)? as usize,
                    })
                })
                .map_err(|e| {
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        e.to_string(),
                    )
                })?;
            for row in rows {
                folders.push(row.map_err(|e| {
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        e.to_string(),
                    )
                })?);
            }
            response_groups.push(FolderGroup {
                shared_count,
                folders,
            });
        }

        // file_types filter is not applied against materialization yet (would require
        // re-scanning hashes). Prefer speed; client can still filter after detail load.
        let _ = params.file_types;

        Ok(Json(DuplicateFoldersResponse {
            groups: response_groups,
            total_groups,
            page,
            per_page,
            needs_refresh: false,
        }))
    })
    .await
}

/// Load files for a specific folder in a duplicate group.
pub async fn folder_files_handler(
    State(state): State<AppState>,
    Query(params): Query<FolderFilesParams>,
) -> Result<Json<FolderFilesResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;

        // Single join — avoid loading every duplicate_hashes row into memory.
        let mut stmt = conn
            .prepare(
                "SELECT f.path, fn.name, f.size, COALESCE(f.hash, ''),
                        CASE WHEN d.hash IS NOT NULL THEN 1 ELSE 0 END
                 FROM files f
                 JOIN file_names fn ON f.file_name_id = fn.id
                 LEFT JOIN duplicate_hashes d ON d.hash = f.hash
                 WHERE f.parent_path = ?1
                   AND f.is_file = 1
                 ORDER BY fn.name
                 LIMIT 5000",
            )
            .map_err(|e| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string(),
                )
            })?;

        let files: Vec<FolderFile> = stmt
            .query_map(rusqlite::params![params.path], |row| {
                Ok(FolderFile {
                    path: row.get(0)?,
                    name: row.get(1)?,
                    size: row.get(2)?,
                    hash: row.get(3)?,
                    is_duplicate: row.get::<_, i64>(4)? != 0,
                })
            })
            .map_err(|e| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string(),
                )
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Json(FolderFilesResponse { files }))
    })
    .await
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
    let cwd = state.cwd.trim_end_matches('/').to_string();

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

/// Distinct file extensions among files that have duplicate hashes.
/// Limited scan with DISTINCT-friendly extraction in Rust after a bounded query.
pub async fn available_file_types_handler(
    State(state): State<AppState>,
) -> Result<Json<AvailableFileTypesResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;

        // Prefer joining to duplicate_hashes (indexed) over IN(subquery) full materialization
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT fn.name
                 FROM files f
                 JOIN file_names fn ON f.file_name_id = fn.id
                 JOIN duplicate_hashes d ON d.hash = f.hash
                 WHERE f.is_file = 1
                 LIMIT 50000",
            )
            .map_err(|e| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string(),
                )
            })?;

        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    e.to_string(),
                )
            })?;

        let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for row in rows {
            if let Ok(name) = row {
                if let Some((_, ext)) = name.rsplit_once('.') {
                    let ext = ext.to_lowercase();
                    if !ext.is_empty() && ext.len() <= 16 {
                        set.insert(ext);
                    }
                }
            }
        }

        Ok(Json(AvailableFileTypesResponse {
            types: set.into_iter().collect(),
        }))
    })
    .await
}
