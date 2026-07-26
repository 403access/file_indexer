use axum::extract::{State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::sql::database::{get_connection, get_ignore_list, set_ignore_list};
use crate::states::app_state::AppState;

#[derive(Serialize)]
pub struct SettingsResponse {
    pub ignore_folders: Vec<String>,
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub ignore_folders: Vec<String>,
}

pub async fn get_settings_handler(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse>, (axum::http::StatusCode, String)> {
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let ignore_folders = get_ignore_list(&conn);

    Ok(Json(SettingsResponse { ignore_folders }))
}

pub async fn update_settings_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, (axum::http::StatusCode, String)> {
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    set_ignore_list(&conn, &payload.ignore_folders)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(SettingsResponse {
        ignore_folders: payload.ignore_folders,
    }))
}
