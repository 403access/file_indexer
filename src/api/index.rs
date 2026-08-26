use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use crate::modules::commands::command_index_files::index_directory;
use crate::modules::logging;
use crate::modules::processes;
use crate::modules::progress;
use crate::modules::sql::database::get_connection;
use crate::states::app_state::AppState;

#[derive(Serialize)]
pub struct IndexResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct IndexRequest {
    /// Optional list of subfolders of CWD to re-sync instead of the whole
    /// tree. Use this after manually editing/moving files in known places.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Convenience alias for a single folder.
    #[serde(default)]
    pub path: Option<String>,
    /// Old paths to purge from the index WITHOUT walking them on disk
    /// (they typically no longer exist there). Use for moves/renames:
    /// resync the new location and remove the stale old one in one call,
    /// instead of re-scanning the whole parent that contained the move.
    #[serde(default)]
    pub remove: Vec<String>,
}

/// Validate one requested target folder and normalize it.
///
/// Returns the trimmed absolute path, or a message explaining why it was
/// rejected.
fn validate_target(raw: &str, cwd_trimmed: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Empty path".to_string());
    }
    if !trimmed.starts_with(cwd_trimmed) {
        return Err(format!(
            "Path '{trimmed}' is outside the indexed working directory '{cwd_trimmed}'"
        ));
    }
    if !std::path::Path::new(trimmed).is_dir() {
        return Err(format!(
            "Path '{trimmed}' does not exist or is not a directory"
        ));
    }
    Ok(trimmed.to_string())
}

/// Validate a removal path: must be inside CWD but is allowed to no longer
/// exist on disk (that's the whole point — purge stale index rows).
fn validate_removal(raw: &str, cwd_trimmed: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Empty path".to_string());
    }
    if !trimmed.starts_with(cwd_trimmed) {
        return Err(format!(
            "Path '{trimmed}' is outside the indexed working directory '{cwd_trimmed}'"
        ));
    }
    Ok(trimmed.to_string())
}

pub async fn index_handler(
    State(state): State<AppState>,
    body: Option<Json<IndexRequest>>,
) -> Result<Json<IndexResponse>, (axum::http::StatusCode, String)> {
    let cwd_trimmed = state.cwd.trim_end_matches('/').to_string();
    let mut targets: Vec<String> = Vec::new();
    let mut removals: Vec<String> = Vec::new();

    let mut requested: Vec<String> = Vec::new();
    if let Some(Json(req)) = body {
        requested.extend(req.paths);
        if let Some(p) = req.path {
            requested.push(p);
        }
        for raw in &req.remove {
            match validate_removal(raw, &cwd_trimmed) {
                Ok(r) => {
                    if !removals.contains(&r) {
                        removals.push(r);
                    }
                }
                Err(msg) => {
                    return Err((axum::http::StatusCode::BAD_REQUEST, msg));
                }
            }
        }
        // Never re-index a folder that is also being removed (contradiction).
        for r in &removals {
            if requested.iter().any(|p| {
                let t = p.trim().trim_end_matches('/');
                t == r || t.starts_with(&format!("{r}/"))
            }) {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Path '{r}' cannot be both removed and re-synced"),
                ));
            }
        }
    }

    if requested.is_empty() && removals.is_empty() {
        targets.push(cwd_trimmed.clone());
    } else {
        // Validate everything up front; reject the whole request on any bad
        // path so typos don't cause a half-done partial resync.
        for raw in &requested {
            match validate_target(raw, &cwd_trimmed) {
                Ok(t) => {
                    if !targets.contains(&t) {
                        targets.push(t);
                    }
                }
                Err(msg) => {
                    return Err((axum::http::StatusCode::BAD_REQUEST, msg));
                }
            }
        }
    }

    let db = state.db.clone();
    let pause = state.pause_indexer.clone();

    let summary = targets.join(", ");
    let process_id =
        processes::register_controllable("Manual re-index", "indexing", Some(&summary));
    progress::start(0);

    let db = db.clone();
    let removal_count = removals.len();
    let result = tokio::task::spawn_blocking(move || {
        // Purge stale old-path subtrees first (moves/renames), without any
        // disk access — this is what avoids re-scanning a huge parent.
        if removal_count > 0 {
            processes::update(
                process_id,
                None,
                Some(&format!("Removing {removal_count} stale path(s) from index")),
            );
            let mut conn = match get_connection(&db) {
                Ok(c) => c,
                Err(e) => {
                    progress::finish();
                    return Err(e.to_string());
                }
            };
            let tx = match conn.transaction() {
                Ok(t) => t,
                Err(e) => {
                    progress::finish();
                    return Err(e.to_string());
                }
            };
            for r in &removals {
                if processes::is_stopped(process_id) {
                    let _ = tx.rollback();
                    progress::finish();
                    return Err("stopped by user".to_string());
                }
                if let Err(e) = crate::modules::sql::database::delete_directory_tree(&tx, r) {
                    let _ = tx.rollback();
                    progress::finish();
                    return Err(e.to_string());
                }
            }
            if let Err(e) = tx.commit() {
                progress::finish();
                return Err(e.to_string());
            }
        }

        for target in &targets {
            if processes::is_stopped(process_id) {
                progress::finish();
                return Err("stopped by user".to_string());
            }
            processes::update(process_id, None, Some(&format!("Re-syncing {target}")));
            if let Err(e) = index_directory(&db, target, Some(pause.clone()), Some(process_id)) {
                progress::finish();
                return Err(e.to_string());
            }
        }
        progress::finish();
        let count = count_entries(&db);
        Ok((count, summary))
    })
    .await
    .map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;

    match result {
        Ok((count, cwd)) => {
            processes::complete(
                process_id,
                Some(&format!("Indexed {} entries from {}", count, cwd)),
            );
            Ok(Json(IndexResponse {
                success: true,
                message: format!("Indexed {} entries from {}", count, cwd),
            }))
        }
        Err(e) => {
            if e.contains("stopped by user") {
                processes::fail(process_id, "Stopped by user");
            } else {
                processes::fail(process_id, &e);
            }
            Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))
        }
    }
}

pub fn count_entries(db_path: &str) -> usize {
    let conn = match get_connection(db_path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .unwrap_or(0)
}

pub fn ensure_indexed(db_path: &str, cwd: &str) {
    logging::info(&format!("Indexing {}...", cwd));
    match index_directory(db_path, cwd, None, None) {
        Ok(()) => {
            let count = count_entries(db_path);
            logging::info(&format!("Indexed {} entries.", count));
        }
        Err(e) => logging::error(&format!("Auto-index failed: {}", e)),
    }
}

pub async fn ensure_indexed_async(
    db_path: String,
    cwd: String,
    pause_flag: Arc<AtomicUsize>,
) {
    logging::info(&format!("Indexing {} in background...", cwd));
    let process_id =
        processes::register_controllable("Startup indexing", "indexing", Some(&cwd));
    progress::start(0);
    // Fire-and-forget on the blocking pool so the async runtime stays free.
    let _ = tokio::task::spawn_blocking(move || {
        match index_directory(&db_path, &cwd, Some(pause_flag), Some(process_id)) {
            Ok(()) => {
                let count = count_entries(&db_path);
                progress::finish();
                processes::complete(process_id, Some(&format!("Indexed {} entries", count)));
                logging::info(&format!("Indexed {} entries.", count));
            }
            Err(e) => {
                progress::finish();
                let msg = e.to_string();
                if msg.contains("stopped by user") {
                    processes::fail(process_id, "Stopped by user");
                    logging::info("Startup indexing stopped by user");
                } else {
                    processes::fail(process_id, &msg);
                    logging::error(&format!("Auto-index failed: {}", msg));
                }
            }
        }
    });
}
