use axum::routing::get;
use axum::Router;

use crate::states::app_state::AppState;

use self::search::search_handler;

pub mod search;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/search", get(search_handler))
        .with_state(state)
}
