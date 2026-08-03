use axum::extract::State;
use axum::Json;
use serde::Serialize;

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
    let cwd = &state.cwd;
    let db = &state.db;

    let process_id = processes::register("Manual re-index", "indexing", Some(cwd));
    crate::modules::progress::start(0);
    let result = index_directory(db, cwd, Some(state.pause_indexer.clone()));
    crate::modules::progress::finish();

    match result {
        Ok(()) => {
            let count = count_entries(db);
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
            processes::fail(process_id, &e.to_string());
            Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
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
    match index_directory(db_path, cwd, None) {
        Ok(()) => {
            let count = count_entries(db_path);
            logging::info(&format!("Indexed {} entries.", count));
        }
        Err(e) => logging::error(&format!("Auto-index failed: {}", e)),
    }
}

pub async fn ensure_indexed_async(db_path: String, cwd: String, pause_flag: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    logging::info(&format!("Indexing {} in background...", cwd));
    let process_id = processes::register("Startup indexing", "indexing", Some(&cwd));
    progress::start(0);
    tokio::task::spawn_blocking(move || match index_directory(&db_path, &cwd, Some(pause_flag)) {
        Ok(()) => {
            let count = count_entries(&db_path);
            progress::finish();
            processes::complete(process_id, Some(&format!("Indexed {} entries", count)));
            logging::info(&format!("Indexed {} entries.", count));
        }
        Err(e) => {
            progress::finish();
            processes::fail(process_id, &e.to_string());
            logging::error(&format!("Auto-index failed: {}", e));
        }
    });
}
