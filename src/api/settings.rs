use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::offload;
use crate::modules::sql::database::{
    count_ignore_events, get_connection, get_ignore_rules, get_setting, set_ignore_rules,
    set_setting, IgnoreRule,
};
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Serialize, Deserialize)]
pub struct IgnoreRuleJson {
    pub name: String,
    pub condition: Option<String>,
    #[serde(default)]
    pub ignore_count: u64,
}

impl IgnoreRuleJson {
    pub fn from_rule(rule: &IgnoreRule, ignore_count: u64) -> Self {
        Self {
            name: rule.name.clone(),
            condition: rule.condition.clone(),
            ignore_count,
        }
    }

    pub fn to_rule(&self) -> IgnoreRule {
        IgnoreRule {
            name: self.name.clone(),
            condition: self.condition.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct SettingsResponse {
    pub ignore_folders: Vec<IgnoreRuleJson>,
    pub dashboard_refresh_interval: u64,
    pub enable_startup_indexing: bool,
    pub enable_dashboard_refresh: bool,
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub ignore_folders: Vec<IgnoreRuleJson>,
    pub dashboard_refresh_interval: Option<u64>,
    pub enable_startup_indexing: Option<bool>,
    pub enable_dashboard_refresh: Option<bool>,
}

fn parse_bool_setting(conn: &rusqlite::Connection, key: &str, default: bool) -> bool {
    get_setting(conn, key)
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(default)
}

fn set_bool_setting(conn: &rusqlite::Connection, key: &str, value: bool) -> rusqlite::Result<()> {
    set_setting(conn, key, &value.to_string())
}

pub async fn get_settings_handler(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let rules = get_ignore_rules(&conn);
        let folders: Vec<IgnoreRuleJson> = rules
            .iter()
            .map(|rule| {
                let count = count_ignore_events(&conn, &rule.to_raw())
                    .unwrap_or(0)
                    .max(0) as u64;
                IgnoreRuleJson::from_rule(rule, count)
            })
            .collect();

        let dashboard_refresh_interval = get_setting(&conn, "dashboard_refresh_interval")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let enable_startup_indexing = parse_bool_setting(&conn, "enable_startup_indexing", true);
        let enable_dashboard_refresh = parse_bool_setting(&conn, "enable_dashboard_refresh", true);

        Ok(Json(SettingsResponse {
            ignore_folders: folders,
            dashboard_refresh_interval,
            enable_startup_indexing,
            enable_dashboard_refresh,
        }))
    })
    .await
}

pub async fn update_settings_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let rules: Vec<IgnoreRule> = payload.ignore_folders.iter().map(|r| r.to_rule()).collect();
        set_ignore_rules(&conn, &rules)
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if let Some(interval) = payload.dashboard_refresh_interval {
            set_setting(&conn, "dashboard_refresh_interval", &interval.to_string())
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }

        if let Some(enabled) = payload.enable_startup_indexing {
            set_bool_setting(&conn, "enable_startup_indexing", enabled)
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }

        if let Some(enabled) = payload.enable_dashboard_refresh {
            set_bool_setting(&conn, "enable_dashboard_refresh", enabled)
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }

        let dashboard_refresh_interval = get_setting(&conn, "dashboard_refresh_interval")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let enable_startup_indexing = parse_bool_setting(&conn, "enable_startup_indexing", true);
        let enable_dashboard_refresh = parse_bool_setting(&conn, "enable_dashboard_refresh", true);

        Ok(Json(SettingsResponse {
            ignore_folders: payload.ignore_folders,
            dashboard_refresh_interval,
            enable_startup_indexing,
            enable_dashboard_refresh,
        }))
    })
    .await
}
