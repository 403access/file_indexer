use axum::extract::State;
use axum::Json;
use serde::Serialize;
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

pub async fn index_handler(
    State(state): State<AppState>,
) -> Result<Json<IndexResponse>, (axum::http::StatusCode, String)> {
    let cwd = state.cwd.clone();
    let db = state.db.clone();
    let pause = state.pause_indexer.clone();

    let process_id =
        processes::register_controllable("Manual re-index", "indexing", Some(&cwd));
    progress::start(0);

    let result = tokio::task::spawn_blocking(move || {
        let result = index_directory(&db, &cwd, Some(pause), Some(process_id));
        progress::finish();
        match result {
            Ok(()) => {
                let count = count_entries(&db);
                Ok((count, cwd))
            }
            Err(e) => Err(e.to_string()),
        }
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
