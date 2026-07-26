use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::modules::sql::database::{get_connection, get_ignore_rules, get_setting, set_ignore_rules, set_setting, IgnoreRule};
use crate::states::app_state::AppState;

#[derive(Serialize, Deserialize)]
pub struct IgnoreRuleJson {
    pub name: String,
    pub condition: Option<String>,
}

impl IgnoreRuleJson {
    pub fn from_rule(rule: &IgnoreRule) -> Self {
        Self {
            name: rule.name.clone(),
            condition: rule.condition.clone(),
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
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub ignore_folders: Vec<IgnoreRuleJson>,
    pub dashboard_refresh_interval: Option<u64>,
}

pub async fn get_settings_handler(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse>, (axum::http::StatusCode, String)> {
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rules = get_ignore_rules(&conn);
    let folders: Vec<IgnoreRuleJson> = rules.iter().map(IgnoreRuleJson::from_rule).collect();

    let dashboard_refresh_interval = get_setting(&conn, "dashboard_refresh_interval")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    Ok(Json(SettingsResponse {
        ignore_folders: folders,
        dashboard_refresh_interval,
    }))
}

pub async fn update_settings_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, (axum::http::StatusCode, String)> {
    let conn = get_connection(&state.db)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rules: Vec<IgnoreRule> = payload.ignore_folders.iter().map(|r| r.to_rule()).collect();
    set_ignore_rules(&conn, &rules)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(interval) = payload.dashboard_refresh_interval {
        set_setting(&conn, "dashboard_refresh_interval", &interval.to_string())
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    let dashboard_refresh_interval = get_setting(&conn, "dashboard_refresh_interval")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    Ok(Json(SettingsResponse {
        ignore_folders: payload.ignore_folders,
        dashboard_refresh_interval,
    }))
}
