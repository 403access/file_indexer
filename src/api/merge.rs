use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::modules::logging;
use crate::modules::processes;
use crate::states::app_state::AppState;

#[derive(Deserialize)]
pub struct MergeRequest {
    pub keep: Vec<String>,
    pub remove: Vec<String>,
    pub destination: String,
}

#[derive(Serialize)]
pub struct MergeResponse {
    pub copied: usize,
    pub removed: usize,
}

pub async fn merge_handler(
    State(state): State<AppState>,
    Json(req): Json<MergeRequest>,
) -> Result<Json<MergeResponse>, (axum::http::StatusCode, String)> {
    let cwd = state.cwd.trim_end_matches('/');
    let dest = if req.destination.starts_with('/') {
        req.destination.clone()
    } else {
        format!("{}/{}", cwd, req.destination)
    };

    let process_id = processes::register("Merge duplicate folders", "merge", Some(&dest));
    let total_ops = req.keep.len() + req.remove.len();

    tokio::fs::create_dir_all(&dest).await
        .map_err(|e| {
            processes::fail(process_id, &format!("Failed to create destination: {}", e));
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create destination: {}", e))
        })?;

    let mut copied = 0;
    let mut removed = 0;
    let mut ops_done = 0;

    for src_path in &req.keep {
        let normalized = src_path.replace("//", "/");
        let full_path = if normalized.starts_with('/') {
            normalized
        } else {
            format!("{}/{}", cwd, normalized)
        };

        if !full_path.starts_with(cwd) {
            ops_done += 1;
            continue;
        }

        let file_name = Path::new(&full_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let dest_file = format!("{}/{}", dest, file_name);

        if full_path != dest_file {
            match tokio::fs::copy(&full_path, &dest_file).await {
                Ok(_) => copied += 1,
                Err(e) => logging::error(&format!("Failed to copy {}: {}", full_path, e)),
            }
        } else {
            copied += 1;
        }

        ops_done += 1;
        if total_ops > 0 {
            processes::update(process_id, Some((ops_done as f64 / total_ops as f64) * 100.0), Some(&format!("Copying {}...", file_name)));
        }
    }

    for rm_path in &req.remove {
        let normalized = rm_path.replace("//", "/");
        let full_path = if normalized.starts_with('/') {
            normalized
        } else {
            format!("{}/{}", cwd, normalized)
        };

        if !full_path.starts_with(cwd) {
            ops_done += 1;
            continue;
        }

        if full_path == dest || full_path.starts_with(&format!("{}/", dest)) {
            ops_done += 1;
            continue;
        }

        match tokio::fs::remove_file(&full_path).await {
            Ok(_) => removed += 1,
            Err(e) => logging::error(&format!("Failed to remove {}: {}", full_path, e)),
        }

        ops_done += 1;
        if total_ops > 0 {
            processes::update(process_id, Some((ops_done as f64 / total_ops as f64) * 100.0), Some(&format!("Removing {}...", Path::new(&full_path).file_name().and_then(|n| n.to_str()).unwrap_or("file"))));
        }
    }

    processes::complete(process_id, Some(&format!("Copied {}, removed {}", copied, removed)));
    Ok(Json(MergeResponse { copied, removed }))
}
