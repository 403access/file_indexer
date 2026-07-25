use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::modules::logging;
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

    // Normalize destination
    let dest = if req.destination.starts_with('/') {
        req.destination.clone()
    } else {
        format!("{}/{}", cwd, req.destination)
    };

    // Create destination directory
    tokio::fs::create_dir_all(&dest).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create destination: {}", e)))?;

    let mut copied = 0;
    let mut removed = 0;

    // Copy kept files to destination
    for src_path in &req.keep {
        let normalized = src_path.replace("//", "/");
        let full_path = if normalized.starts_with('/') {
            normalized
        } else {
            format!("{}/{}", cwd, normalized)
        };

        // Validate path is under CWD
        if !full_path.starts_with(cwd) {
            continue;
        }

        let file_name = Path::new(&full_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let dest_file = format!("{}/{}", dest, file_name);

        // Only copy if source and destination are different
        if full_path != dest_file {
            match tokio::fs::copy(&full_path, &dest_file).await {
                Ok(_) => copied += 1,
                Err(e) => logging::error(&format!("Failed to copy {}: {}", full_path, e)),
            }
        } else {
            copied += 1;
        }
    }

    // Remove unselected files
    for rm_path in &req.remove {
        let normalized = rm_path.replace("//", "/");
        let full_path = if normalized.starts_with('/') {
            normalized
        } else {
            format!("{}/{}", cwd, normalized)
        };

        // Validate path is under CWD
        if !full_path.starts_with(cwd) {
            continue;
        }

        // Safety: don't delete the destination directory itself
        if full_path == dest || full_path.starts_with(&format!("{}/", dest)) {
            continue;
        }

        match tokio::fs::remove_file(&full_path).await {
            Ok(_) => removed += 1,
            Err(e) => logging::error(&format!("Failed to remove {}: {}", full_path, e)),
        }
    }

    Ok(Json(MergeResponse { copied, removed }))
}
