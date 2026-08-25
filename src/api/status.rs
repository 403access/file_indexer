use axum::Json;

use crate::modules::progress::get_progress;

pub async fn status_handler() -> Json<crate::modules::progress::Progress> {
    Json(get_progress())
}
