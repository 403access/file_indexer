use axum::Router;
use tower_http::services::ServeDir;

use file_indexer::api::{create_router, index::ensure_indexed_async};
use file_indexer::modules::environment::check_vars::check_vars;
use file_indexer::states::app_state::{self, AppState};

#[tokio::main]
async fn main() {
    check_vars();
    let path = file_indexer::modules::environment::env_vars::get_cwd();
    let database_url = file_indexer::modules::environment::env_vars::get_database_url();

    app_state::init(path, database_url.clone());
    file_indexer::modules::logging::init(&database_url);

    let state = AppState {
        cwd: app_state::get_cwd(),
        db: database_url.clone(),
        pause_indexer: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    ensure_indexed_async(database_url.clone(), state.cwd.clone(), state.pause_indexer.clone()).await;

    // Initial dashboard refresh
    {
        let conn = file_indexer::modules::sql::database::get_connection(&database_url).unwrap();
        file_indexer::modules::sql::database::refresh_dashboard_stats(&conn);
    }

    // Spawn background task for periodic dashboard refresh
    let refresh_db = database_url.clone();
    tokio::spawn(async move {
        loop {
            // Read refresh interval from settings (default 60s)
            let interval_secs = {
                let conn = file_indexer::modules::sql::database::get_connection(&refresh_db);
                conn.ok()
                    .and_then(|c| {
                        file_indexer::modules::sql::database::get_setting(&c, "dashboard_refresh_interval")
                            .ok()
                            .flatten()
                    })
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60)
            };

            tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;

            if let Ok(conn) = file_indexer::modules::sql::database::get_connection(&refresh_db) {
                file_indexer::modules::sql::database::recompute_dashboard_stats(&conn);
            }
        }
    });

    tracing_subscriber::fmt::init();

    let api_router = create_router(state);

    let app = Router::new()
        .fallback_service(ServeDir::new("static"))
        .merge(api_router);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
