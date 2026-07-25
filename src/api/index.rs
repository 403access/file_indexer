use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::modules::commands::command_index_files::index_directory;
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

    match index_directory(db, cwd) {
        Ok(()) => {
            let count = count_entries(db);
            Ok(Json(IndexResponse {
                success: true,
                message: format!("Indexed {} entries from {}", count, cwd),
            }))
        }
        Err(e) => Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
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
    if count_entries(db_path) == 0 {
        println!("Database empty, indexing {}...", cwd);
        match index_directory(db_path, cwd) {
            Ok(()) => {
                let count = count_entries(db_path);
                println!("Indexed {} entries.", count);
            }
            Err(e) => eprintln!("Auto-index failed: {}", e),
        }
    }
}

pub async fn ensure_indexed_async(db_path: String, cwd: String) {
    if count_entries(&db_path) == 0 {
        println!("Database empty, indexing {} in background...", cwd);
        progress::start(0);
        tokio::task::spawn_blocking(move || match index_directory(&db_path, &cwd) {
            Ok(()) => {
                let count = count_entries(&db_path);
                progress::finish();
                println!("Indexed {} entries.", count);
            }
            Err(e) => {
                progress::finish();
                eprintln!("Auto-index failed: {}", e);
            }
        });
    }
}
