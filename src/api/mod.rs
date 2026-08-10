use axum::routing::{get, post};
use axum::Router;

use crate::states::app_state::AppState;

use self::config::config_handler;
use self::dashboard::{dashboard_handler, refresh_handler};
use self::duplicate_folders::{
    available_file_types_handler, check_folders_handler, duplicate_folders_handler,
    folder_files_handler,
};
use self::duplicates::duplicates_handler;
use self::explorer::explorer_handler;
use self::file_content::file_content_handler;
use self::folder::folder_handler;
use self::index::index_handler;
use self::logs::logs_handler;
use self::merge::merge_handler;
use self::processes::{clear_processes_handler, pause_process_handler, process_logs_handler, processes_handler, resume_process_handler, stop_process_handler, trigger_process_handler};
use self::search::search_handler;
use self::settings::{get_settings_handler, update_settings_handler};
use self::skipped::skipped_handler;
use self::status::status_handler;
use self::tree::tree_handler;

pub mod config;
pub mod dashboard;
pub mod duplicate_folders;
pub mod duplicates;
pub mod explorer;
pub mod file_content;
pub mod folder;
pub mod index;
pub mod logs;
pub mod merge;
pub mod processes;
pub mod search;
pub mod settings;
pub mod skipped;
pub mod status;
pub mod tree;

/// Run synchronous SQLite / filesystem work off the async runtime so HTTP
/// stays responsive under indexing and heavy queries.
pub(crate) async fn offload<T, F>(f: F) -> Result<T, (axum::http::StatusCode, String)>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, (axum::http::StatusCode, String)> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/dashboard/refresh", post(refresh_handler))
        .route("/api/dashboard", get(dashboard_handler))
        .route("/api/search", get(search_handler))
        .route("/api/duplicates", get(duplicates_handler))
        .route("/api/duplicate-folders/files", get(folder_files_handler))
        .route("/api/duplicate-folders", get(duplicate_folders_handler))
        .route("/api/duplicate-folders/types", get(available_file_types_handler))
        .route("/api/folders/check", post(check_folders_handler))
        .route("/api/tree", get(tree_handler))
        .route("/api/explorer", get(explorer_handler))
        .route("/api/file", get(file_content_handler))
        .route("/api/folder", get(folder_handler))
        .route("/api/index", post(index_handler))
        .route("/api/config", get(config_handler))
        .route("/api/merge", post(merge_handler))
        .route("/api/skipped", get(skipped_handler))
        .route("/api/logs", get(logs_handler))
        .route("/api/status", get(status_handler))
        .route("/api/processes", get(processes_handler))
        .route("/api/processes/clear", post(clear_processes_handler))
        .route("/api/processes/{id}/pause", post(pause_process_handler))
        .route("/api/processes/{id}/resume", post(resume_process_handler))
        .route("/api/processes/{id}/stop", post(stop_process_handler))
        .route("/api/processes/{id}/trigger", post(trigger_process_handler))
        .route("/api/processes/{id}/logs", get(process_logs_handler))
        .route("/api/settings", get(get_settings_handler).post(update_settings_handler))
        .with_state(state)
}
