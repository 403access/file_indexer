use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};

use crate::api::offload;
use crate::modules::processes::{self, Process};
use crate::modules::sql::database::{
    get_connection, is_process_stopped, refresh_dashboard_stats,
};

use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Serialize)]
pub struct ProcessesResponse {
    pub processes: Vec<Process>,
    /// Scheduled process types currently disabled (persisted "stopped by user").
    pub disabled_types: Vec<DisabledProcessType>,
}

#[derive(Serialize)]
pub struct DisabledProcessType {
    pub key: String,
    pub name: String,
    pub category: String,
    /// Persisted "stopped by user" flag (survives restarts until re-enabled).
    pub stopped: bool,
    /// Whether the corresponding ENABLE_* env var(s) are on.
    pub env_enabled: bool,
}

type EnvFlags = (
    bool, // enable_startup_indexing
    bool, // enable_initial_dashboard_refresh
    bool, // enable_dashboard_refresh
    bool, // enable_duplicate_folder_groups_refresh
);

fn scheduled_type_env_enabled(flags: &EnvFlags, key: &str) -> bool {
    match key {
        "startup_indexing" => flags.0,
        // The shared key covers both the one-time initial refresh and the
        // periodic refresh; disabled only when both env vars are off.
        "dashboard_refresh" => flags.1 || flags.2,
        "duplicate_folder_groups_refresh" => flags.3,
        _ => false,
    }
}

pub async fn processes_handler(State(state): State<AppState>) -> Json<ProcessesResponse> {
    let processes = processes::get_all();

    // Types that are currently running/pending have a live process; don't list
    // them as disabled even if a stale flag or env state says otherwise.
    let active_keys: std::collections::HashSet<&str> = processes
        .iter()
        .filter(|p| {
            matches!(
                p.status,
                crate::modules::processes::ProcessStatus::Running
                    | crate::modules::processes::ProcessStatus::Pending
            )
        })
        .filter_map(|p| processes::stopped_state_key(&p.name))
        .collect();

    let db = state.db.clone();
    let env_flags = (
        state.enable_startup_indexing,
        state.enable_initial_dashboard_refresh,
        state.enable_dashboard_refresh,
        state.enable_duplicate_folder_groups_refresh,
    );
    let disabled_types = offload(move || {
        let conn = get_connection(&db)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let mut types = Vec::new();
        for t in processes::scheduled_process_types() {
            if active_keys.contains(t.key) {
                continue;
            }
            let stopped = is_process_stopped(&conn, t.key);
            let env_enabled = scheduled_type_env_enabled(&env_flags, t.key);
            if stopped || !env_enabled {
                types.push(DisabledProcessType {
                    key: t.key.to_string(),
                    name: t.name.to_string(),
                    category: t.category.to_string(),
                    stopped,
                    env_enabled,
                });
            }
        }
        Ok::<_, (axum::http::StatusCode, String)>(types)
    })
    .await
    .unwrap_or_default();

    Json(ProcessesResponse {
        processes,
        disabled_types,
    })
}

/// Re-enable a previously stopped scheduled process type by clearing its
/// persisted flag. The process auto-starts again on the next boot.
pub async fn enable_process_type_handler(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<ProcessActionResponse>, (axum::http::StatusCode, String)> {
    let known = processes::scheduled_process_types()
        .into_iter()
        .find(|t| t.key == key)
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("Unknown process type: {}", key),
        ))?;

    let db = state.db.clone();
    let name = known.name.to_string();
    let key_for_job = key.clone();
    let res = offload(move || {
        let conn = get_connection(&db)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        crate::modules::sql::database::set_process_stopped(&conn, &key_for_job, false)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        Ok::<(), (axum::http::StatusCode, String)>(())
    })
    .await;
    if let Err(e) = res {
        return Err(e);
    }

    let env_on = scheduled_type_env_enabled(
        &(
            state.enable_startup_indexing,
            state.enable_initial_dashboard_refresh,
            state.enable_dashboard_refresh,
            state.enable_duplicate_folder_groups_refresh,
        ),
        &key,
    );
    let message = if env_on {
        format!("{} enabled; it will auto-start on the next boot", name)
    } else {
        format!(
            "{} stopped flag cleared, but it remains disabled via ENV; set ENABLE_* to true to start it",
            name
        )
    };

    Ok(Json(ProcessActionResponse { ok: true, message }))
}

pub async fn clear_processes_handler() -> Json<serde_json::Value> {
    processes::clear_completed();
    Json(serde_json::json!({ "ok": true }))
}

#[derive(Serialize)]
pub struct ProcessActionResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct ProcessLogsParams {
    pub limit: Option<i64>,
}

pub async fn process_logs_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Query(params): Query<ProcessLogsParams>,
) -> Result<Json<Vec<crate::modules::logging::LogEntry>>, (axum::http::StatusCode, String)> {
    let limit = params.limit.unwrap_or(1000);
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT timestamp, level, message FROM logs WHERE process_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            )
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let logs: Vec<crate::modules::logging::LogEntry> = stmt
            .query_map(rusqlite::params![id, limit], |row| {
                Ok(crate::modules::logging::LogEntry {
                    timestamp: row.get(0)?,
                    level: row.get(1)?,
                    message: row.get(2)?,
                    files: None,
                })
            })
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Json(logs))
    })
    .await
}

pub async fn trigger_process_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<ProcessActionResponse>, (axum::http::StatusCode, String)> {
    let all_processes = processes::get_all();
    let process = all_processes.iter().find(|p| p.id == id).ok_or((
        axum::http::StatusCode::NOT_FOUND,
        format!("Process #{} not found", id),
    ))?;

    let name = process.name.clone();
    let category = process.category.clone();

    match category.as_str() {
        "dashboard" | "duplicate-folders" => {}
        _ => {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                format!("Cannot manually trigger process category: {}", category),
            ));
        }
    }

    // A successful manual run signals the process is wanted again: clear its
    // persisted "stopped by user" flag so it auto-starts on future boots.
    let stopped_key = processes::stopped_state_key(&name).map(str::to_string);
    let db_path = state.db.clone();

    let category_for_job = category.clone();
    let name_for_job = name.clone();
    let db = db_path.clone();

    // Heavy DB work off the async runtime; return immediately after kickoff would
    // be nicer, but keep sync completion semantics for the UI.
    let new_id = offload(move || {
        let process_id = processes::register_controllable(
            &format!("Manual {} ({})", name_for_job, category_for_job),
            &category_for_job,
            Some("Manual trigger"),
        );

        let conn = get_connection(&db).map_err(|e| {
            processes::fail(process_id, &e.to_string());
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

        match category_for_job.as_str() {
            "dashboard" => {
                refresh_dashboard_stats(&conn);
                processes::complete(process_id, Some("Dashboard stats refreshed"));
            }
            "duplicate-folders" => {
                crate::modules::sql::database::refresh_duplicate_folder_groups(&conn);
                processes::complete(process_id, Some("Duplicate folder groups refreshed"));
            }
            _ => unreachable!(),
        }
        Ok(process_id)
    })
    .await?;

    if let Some(key) = stopped_key {
        let db = db_path;
        let res = offload(move || {
            let conn = get_connection(&db)
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            crate::modules::sql::database::set_process_stopped(&conn, &key, false)
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Ok::<(), (axum::http::StatusCode, String)>(())
        })
        .await;
        if let Err(e) = res {
            return Err(e);
        }
    }

    Ok(Json(ProcessActionResponse {
        ok: true,
        message: format!("Triggered process #{} (new process #{} running)", id, new_id),
    }))
}

pub async fn pause_process_handler(
    State(_state): State<AppState>,
    Path(id): Path<u64>,
) -> Json<ProcessActionResponse> {
    processes::set_paused(id, true);
    Json(ProcessActionResponse {
        ok: true,
        message: format!("Process #{} paused", id),
    })
}

pub async fn resume_process_handler(
    State(_state): State<AppState>,
    Path(id): Path<u64>,
) -> Json<ProcessActionResponse> {
    processes::set_paused(id, false);
    Json(ProcessActionResponse {
        ok: true,
        message: format!("Process #{} resumed", id),
    })
}

pub async fn stop_process_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<ProcessActionResponse>, (axum::http::StatusCode, String)> {
    processes::request_stop(id);

    // Persist the "stopped by user" state so a restart respects it (unless the
    // operator explicitly ignores DB state via IGNORE_PROCESS_DATABASE_STATE).
    let process = processes::get_all().into_iter().find(|p| p.id == id);
    if let Some(key) = process
        .as_ref()
        .and_then(|p| processes::stopped_state_key(&p.name))
    {
        let db = state.db.clone();
        let key = key.to_string();
        let res = offload(move || {
            let conn = get_connection(&db)
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            crate::modules::sql::database::set_process_stopped(&conn, &key, true)
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            Ok::<(), (axum::http::StatusCode, String)>(())
        })
        .await;
        if res.is_err() {
            return Err(res.unwrap_err());
        }
    }

    Ok(Json(ProcessActionResponse {
        ok: true,
        message: format!("Stop requested for process #{}", id),
    }))
}
