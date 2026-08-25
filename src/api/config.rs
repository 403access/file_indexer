use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::states::app_state::AppState;

#[derive(Serialize)]
pub struct ConfigResponse {
    pub cwd: String,
}

pub async fn config_handler(
    State(state): State<AppState>,
) -> Json<ConfigResponse> {
    Json(ConfigResponse { cwd: state.cwd.clone() })
}
