use axum::Router;
use tower_http::services::ServeDir;

use file_indexer::api::{create_router, index::ensure_indexed_async};
use file_indexer::modules::environment::check_vars::check_vars;
use file_indexer::modules::processes;
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

    if file_indexer::modules::environment::env_vars::get_enable_startup_indexing() {
        let db_clone = database_url.clone();
        let cwd_clone = state.cwd.clone();
        let pause_clone = state.pause_indexer.clone();
        tokio::spawn(async move {
            ensure_indexed_async(db_clone, cwd_clone, pause_clone).await;
        });
    } else {
        println!("⏸️  Startup indexing disabled via ENABLE_STARTUP_INDEXING");
    }

    // Initial dashboard refresh
    {
        let process_id = processes::register("Initial dashboard refresh", "dashboard", Some("Startup"));
        let conn = file_indexer::modules::sql::database::get_connection(&database_url).unwrap();
        file_indexer::modules::sql::database::refresh_dashboard_stats(&conn);
        processes::complete(process_id, Some("Initial dashboard stats refreshed"));
    }

    // Spawn background task for periodic dashboard refresh
    if file_indexer::modules::environment::env_vars::get_enable_dashboard_refresh() {
        let refresh_db = database_url.clone();
        let refresh_process_id = processes::register_controllable("Dashboard refresh", "dashboard", Some("Scheduled"));
        tokio::spawn(async move {
            loop {
                if processes::is_stopped(refresh_process_id) {
                    processes::fail(refresh_process_id, "Stopped by user");
                    break;
                }

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

                processes::update(refresh_process_id, None, Some(&format!("Waiting {}s until next refresh", interval_secs)));

                tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;

                if processes::is_stopped(refresh_process_id) {
                    processes::fail(refresh_process_id, "Stopped by user");
                    break;
                }

                processes::update(refresh_process_id, Some(50.0), Some("Refreshing stats..."));

                if let Ok(conn) = file_indexer::modules::sql::database::get_connection(&refresh_db) {
                    file_indexer::modules::sql::database::recompute_dashboard_stats(&conn);
                    processes::update(refresh_process_id, Some(100.0), Some(&format!("Dashboard stats refreshed; next in {}s", interval_secs)));
                } else {
                    processes::fail(refresh_process_id, "Failed to connect to database");
                    break;
                }
            }
        });
    } else {
        println!("⏸️  Periodic dashboard refresh disabled via ENABLE_DASHBOARD_REFRESH");
    }

    tracing_subscriber::fmt::init();

    let api_router = create_router(state);

    let app = Router::new()
        .fallback_service(ServeDir::new("static"))
        .merge(api_router);

    let port = file_indexer::modules::environment::env_vars::get_server_port();
    let addr = format!("0.0.0.0:{}", port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("❌ Failed to bind API server to {}: {}", addr, e);
            eprintln!("   Is another process already using port {}?", port);
            std::process::exit(1);
        }
    };
    println!("Server running on http://localhost:{}", port);
    axum::serve(listener, app).await.unwrap();
}
