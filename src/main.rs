use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

use file_indexer::api::{create_router, index::ensure_indexed_async};
use file_indexer::modules::environment::check_vars::check_vars;
use file_indexer::modules::processes;
use file_indexer::states::app_state::{self, AppState};

/// Legacy root-level HTML paths → files under `static/pages/`.
/// Keeps bookmarks like `/search.html` working after the assets reorg.
fn static_page_aliases() -> Router {
    const PAGES: &[(&str, &str)] = &[
        ("/search.html", "static/pages/search.html"),
        ("/explorer.html", "static/pages/explorer.html"),
        ("/duplicates.html", "static/pages/duplicates.html"),
        ("/duplicate-folders.html", "static/pages/duplicate-folders.html"),
        ("/skipped.html", "static/pages/skipped.html"),
        ("/ignored.html", "static/pages/ignored.html"),
        ("/status.html", "static/pages/status.html"),
        ("/processes.html", "static/pages/processes.html"),
        ("/logs.html", "static/pages/logs.html"),
        ("/settings.html", "static/pages/settings.html"),
    ];

    let mut router = Router::new();
    for (route, path) in PAGES {
        router = router.route_service(route, ServeFile::new(path));
    }
    router
}

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
        pause_indexer: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };

    // Bind and start the HTTP server first so the UI is always reachable.
    // Heavy work (indexing, dashboard recompute, materialization) runs after.
    tracing_subscriber::fmt::init();

    let api_router = create_router(state.clone());

    // static/
    //   index.html
    //   pages/*.html
    //   assets/{css,js}/...
    // ServeDir covers / , /pages/..., /assets/...
    // Aliases keep old /foo.html URLs working.
    let app = Router::new()
        .merge(static_page_aliases())
        .merge(api_router)
        .fallback_service(ServeDir::new("static"));

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

    // Spawn background jobs only after the listener is bound.
    spawn_background_jobs(state, database_url);

    axum::serve(listener, app).await.unwrap();
}

fn spawn_background_jobs(state: AppState, database_url: String) {
    // Startup indexing (blocking pool — never on async workers)
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

    // Initial dashboard refresh (blocking pool)
    {
        let db = database_url.clone();
        tokio::spawn(async move {
            let process_id =
                processes::register("Initial dashboard refresh", "dashboard", Some("Startup"));
            let result = tokio::task::spawn_blocking(move || {
                let conn = file_indexer::modules::sql::database::get_connection(&db)?;
                file_indexer::modules::sql::database::refresh_dashboard_stats(&conn);
                Ok::<(), rusqlite::Error>(())
            })
            .await;
            match result {
                Ok(Ok(())) => {
                    processes::complete(process_id, Some("Initial dashboard stats refreshed"));
                }
                Ok(Err(e)) => {
                    processes::fail(process_id, &e.to_string());
                }
                Err(e) => {
                    processes::fail(process_id, &e.to_string());
                }
            }
        });
    }

    // Periodic dashboard refresh
    if file_indexer::modules::environment::env_vars::get_enable_dashboard_refresh() {
        let refresh_db = database_url.clone();
        let refresh_process_id =
            processes::register_controllable("Dashboard refresh", "dashboard", Some("Scheduled"));
        tokio::spawn(async move {
            loop {
                if processes::is_stopped(refresh_process_id) {
                    processes::fail(refresh_process_id, "Stopped by user");
                    break;
                }

                let interval_secs = {
                    let db = refresh_db.clone();
                    tokio::task::spawn_blocking(move || {
                        let conn = file_indexer::modules::sql::database::get_connection(&db).ok()?;
                        file_indexer::modules::sql::database::get_setting(
                            &conn,
                            "dashboard_refresh_interval",
                        )
                        .ok()
                        .flatten()
                        .and_then(|v| v.parse::<u64>().ok())
                    })
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or(60)
                    .max(5)
                };

                processes::update(
                    refresh_process_id,
                    None,
                    Some(&format!("Waiting {}s until next refresh", interval_secs)),
                );

                tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;

                if processes::is_stopped(refresh_process_id) {
                    processes::fail(refresh_process_id, "Stopped by user");
                    break;
                }

                processes::update(
                    refresh_process_id,
                    Some(50.0),
                    Some("Refreshing stats..."),
                );

                let db = refresh_db.clone();
                let ok = tokio::task::spawn_blocking(move || {
                    let conn = file_indexer::modules::sql::database::get_connection(&db)?;
                    file_indexer::modules::sql::database::recompute_dashboard_stats(&conn);
                    Ok::<(), rusqlite::Error>(())
                })
                .await;

                match ok {
                    Ok(Ok(())) => {
                        processes::update(
                            refresh_process_id,
                            Some(100.0),
                            Some(&format!(
                                "Dashboard stats refreshed; next in {}s",
                                interval_secs
                            )),
                        );
                    }
                    Ok(Err(e)) => {
                        processes::fail(refresh_process_id, &e.to_string());
                        break;
                    }
                    Err(e) => {
                        processes::fail(refresh_process_id, &e.to_string());
                        break;
                    }
                }
            }
        });
    } else {
        println!("⏸️  Periodic dashboard refresh disabled via ENABLE_DASHBOARD_REFRESH");
    }

    // Duplicate folder groups materialization
    if file_indexer::modules::environment::env_vars::get_enable_duplicate_folder_groups_refresh()
    {
        let dup_db = database_url.clone();
        let dup_process_id = processes::register_controllable(
            "Duplicate folder groups refresh",
            "duplicate-folders",
            Some("Scheduled"),
        );
        tokio::spawn(async move {
            loop {
                if processes::is_stopped(dup_process_id) {
                    processes::fail(dup_process_id, "Stopped by user");
                    break;
                }

                let interval_secs = file_indexer::modules::environment::env_vars::get_duplicate_folder_groups_refresh_interval()
                    .max(30);

                processes::update(
                    dup_process_id,
                    None,
                    Some(&format!("Waiting {}s until next refresh", interval_secs)),
                );

                tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;

                if processes::is_stopped(dup_process_id) {
                    processes::fail(dup_process_id, "Stopped by user");
                    break;
                }

                processes::update(
                    dup_process_id,
                    Some(50.0),
                    Some("Refreshing duplicate folder groups..."),
                );

                let db = dup_db.clone();
                let ok = tokio::task::spawn_blocking(move || {
                    let conn = file_indexer::modules::sql::database::get_connection(&db)?;
                    file_indexer::modules::sql::database::refresh_duplicate_folder_groups(&conn);
                    Ok::<(), rusqlite::Error>(())
                })
                .await;

                match ok {
                    Ok(Ok(())) => {
                        processes::update(
                            dup_process_id,
                            Some(100.0),
                            Some(&format!(
                                "Duplicate folder groups refreshed; next in {}s",
                                interval_secs
                            )),
                        );
                    }
                    Ok(Err(e)) => {
                        processes::fail(dup_process_id, &e.to_string());
                        break;
                    }
                    Err(e) => {
                        processes::fail(dup_process_id, &e.to_string());
                        break;
                    }
                }
            }
        });
    } else {
        println!(
            "⏸️  Duplicate folder groups refresh disabled via ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH"
        );
    }
}
