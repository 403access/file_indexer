use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::modules::sql::database::get_connection;
use crate::states::app_state::AppState;

#[derive(Serialize)]
pub struct FolderFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub hash: String,
    pub is_duplicate: bool,
}

#[derive(Serialize)]
pub struct FolderGroup {
    pub shared_count: usize,
    pub folders: Vec<FolderInfo>,
}

#[derive(Serialize)]
pub struct FolderInfo {
    pub path: String,
    pub name: String,
    pub files: Vec<FolderFile>,
}

#[derive(Serialize)]
pub struct DuplicateFoldersResponse {
    pub groups: Vec<FolderGroup>,
}

pub async fn duplicate_folders_handler(
    State(state): State<AppState>,
) -> Result<Json<DuplicateFoldersResponse>, (axum::http::StatusCode, String)> {
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Find all hashes that appear more than once
    let mut stmt = conn.prepare(
        "SELECT hash, COUNT(*) as cnt FROM files WHERE hash IS NOT NULL GROUP BY hash HAVING cnt > 1"
    ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let dup_hashes: Vec<String> = stmt.query_map([], |row| row.get(0))
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    // For each dup hash, find all folder paths containing it
    // Map: folder_path -> set of hashes found there
    let mut folder_hashes: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    // Map: folder_path -> display name
    let mut folder_names: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for hash in &dup_hashes {
        let mut stmt = conn.prepare(
            "SELECT f.path, fn.name FROM files f JOIN file_names fn ON f.file_name_id = fn.id WHERE f.hash = ?1"
        ).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let rows: Vec<(String, String)> = stmt.query_map(rusqlite::params![hash], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
        drop(stmt);

        for (path, _name) in rows {
            let normalized = path.replace("//", "/");
            // Extract folder path (parent of the file)
            if let Some(parent) = normalized.rsplit_once('/') {
                let folder = parent.0.to_string();
                folder_hashes.entry(folder.clone()).or_default().insert(hash.clone());
                // Store the folder's display name (last component)
                if let Some(display) = folder.rsplit_once('/') {
                    folder_names.entry(folder.clone()).or_insert_with(|| display.1.to_string());
                }
            }
        }
    }

    // Build groups: find folders that share the most hashes
    // Sort folders by number of shared hashes (descending)
    let mut folder_list: Vec<(String, std::collections::HashSet<String>)> = folder_hashes.into_iter().collect();
    folder_list.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    // Group folders: two folders are in the same group if they share at least one hash
    // and we want to find the most connected groups
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
            // Check if this folder shares any hash with any folder in the group
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

    // Sort groups by total shared count (sum of intersection sizes), descending
    groups.sort_by(|a, b| {
        let a_count: usize = a.iter().map(|f| f.1.len()).sum();
        let b_count: usize = b.iter().map(|f| f.1.len()).sum();
        b_count.cmp(&a_count)
    });

    // Build response
    let response_groups: Vec<FolderGroup> = groups.into_iter().map(|group| {
        // Compute the intersection of all folder hashes in this group
        let shared_hashes: std::collections::HashSet<String> = if let Some(first) = group.first() {
            group.iter().skip(1).fold(first.1.clone(), |acc, f| {
                acc.intersection(&f.1).cloned().collect()
            })
        } else {
            std::collections::HashSet::new()
        };

        let folders: Vec<FolderInfo> = group.into_iter().map(|(path, hashes)| {
            // Get ALL files for this folder (not just duplicates)
            let mut stmt = conn.prepare(
                "SELECT f.path, fn.name, f.size, f.hash FROM files f JOIN file_names fn ON f.file_name_id = fn.id WHERE f.hash IS NOT NULL"
            ).unwrap();

            let files: Vec<FolderFile> = stmt.query_map([], |row| {
                let p: String = row.get(0)?;
                let n: String = row.get(1)?;
                let s: u64 = row.get(2)?;
                let h: String = row.get(3)?;
                Ok((p, n, s, h))
            }).unwrap()
            .filter_map(|r| r.ok())
            .filter(|(p, _, _, _)| {
                let normalized = p.replace("//", "/");
                if let Some(parent) = normalized.rsplit_once('/') {
                    parent.0 == path
                } else {
                    false
                }
            })
            .map(|(p, n, s, h)| FolderFile {
                is_duplicate: hashes.contains(&h),
                name: n,
                path: p.replace("//", "/"),
                size: s,
                hash: h,
            })
            .collect();

            let display_name = folder_names.get(&path).cloned().unwrap_or_else(|| {
                path.rsplit_once('/').map(|s| s.1.to_string()).unwrap_or_default()
            });

            FolderInfo { path, name: display_name, files }
        }).collect();

        FolderGroup {
            shared_count: shared_hashes.len(),
            folders,
        }
    }).collect();

    Ok(Json(DuplicateFoldersResponse { groups: response_groups }))
}
