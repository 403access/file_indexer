use axum::Router;
use tower_http::services::ServeDir;

use file_indexer::api::{create_router, index::ensure_indexed};
use file_indexer::modules::environment::check_vars::check_vars;
use file_indexer::states::app_state::{self, AppState};

#[tokio::main]
async fn main() {
    check_vars();
    let path = file_indexer::modules::environment::env_vars::get_cwd();
    let database_url = file_indexer::modules::environment::env_vars::get_database_url();

    app_state::init(path, database_url.clone());

    let state = AppState {
        cwd: app_state::get_cwd(),
        db: database_url.clone(),
    };

    ensure_indexed(&database_url, &state.cwd);

    tracing_subscriber::fmt::init();

    let api_router = create_router(state);

    let app = Router::new()
        .fallback_service(ServeDir::new("static"))
        .merge(api_router);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
