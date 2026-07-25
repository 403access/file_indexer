use axum::routing::{get, post};
use axum::Router;

use crate::states::app_state::AppState;

use self::duplicates::duplicates_handler;
use self::index::index_handler;
use self::search::search_handler;
use self::tree::tree_handler;

pub mod duplicates;
pub mod index;
pub mod search;
pub mod tree;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/search", get(search_handler))
        .route("/api/duplicates", get(duplicates_handler))
        .route("/api/tree", get(tree_handler))
        .route("/api/index", post(index_handler))
        .with_state(state)
}
