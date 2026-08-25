use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::offload;
use crate::modules::sql::database::{compute_pair_delta, get_connection, PairDelta};
use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct NearDupParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
    /// Minimum similarity filter (0.0–1.0), applied on the stored pairs.
    #[serde(default)]
    pub min_similarity: Option<f64>,
    /// Maximum similarity filter (exclusive). Use e.g. 1.0 to hide exact dupes.
    #[serde(default)]
    pub max_similarity: Option<f64>,
    #[serde(default)]
    pub q: Option<String>,
}

fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    20
}

#[derive(Serialize)]
pub struct NearDupPairRow {
    pub folder_a: String,
    pub folder_b: String,
    pub similarity: f64,
    pub shared_files: i64,
    pub union_files: i64,
}

#[derive(Serialize)]
pub struct NearDupResponse {
    pub pairs: Vec<NearDupPairRow>,
    pub total_pairs: usize,
    pub page: u32,
    pub per_page: u32,
    /// True when the pairs table is empty (run the refresh process).
    #[serde(default)]
    pub needs_refresh: bool,
}

fn map_pair_row(row: &rusqlite::Row) -> rusqlite::Result<NearDupPairRow> {
    Ok(NearDupPairRow {
        folder_a: row.get(0)?,
        folder_b: row.get(1)?,
        similarity: row.get(2)?,
        shared_files: row.get(3)?,
        union_files: row.get(4)?,
    })
}

/// List materialized near-duplicate folder pairs.
pub async fn near_duplicate_folders_handler(
    State(state): State<AppState>,
    Query(params): Query<NearDupParams>,
) -> Result<Json<NearDupResponse>, (axum::http::StatusCode, String)> {
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;

        // Table may not exist yet (fresh DB, no refresh ran).
        let table_exists: bool = conn
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_master WHERE type='table'
                     AND name='near_duplicate_folder_pairs'
                 )",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false);

        let page = params.page.max(1);
        let per_page = params.per_page.clamp(1, 100);
        if !table_exists {
            return Ok(Json(NearDupResponse {
                pairs: Vec::new(),
                total_pairs: 0,
                page,
                per_page,
                needs_refresh: true,
            }));
        }

        let offset = (page - 1).saturating_mul(per_page);
        let min_sim = params.min_similarity.unwrap_or(0.0);
        let max_sim = params.max_similarity.unwrap_or(f64::MAX);

        let q = params
            .q
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let q_pat = q.as_ref().map(|s| format!("%{s}%"));

        let pairs: Vec<NearDupPairRow> = if let Some(pat) = &q_pat {
            let mut stmt = conn
                .prepare(
                    "SELECT folder_a, folder_b, similarity, shared_files, union_files
                     FROM near_duplicate_folder_pairs
                     WHERE similarity >= ?1 AND similarity < ?2
                       AND (folder_a LIKE ?3 OR folder_b LIKE ?3)
                     ORDER BY similarity DESC, shared_files DESC
                     LIMIT ?4 OFFSET ?5",
                )
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let mapped = stmt.query_map(
                rusqlite::params![min_sim, max_sim, pat, per_page as i64, offset as i64],
                map_pair_row,
            );
            match mapped {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(e) => {
                    return Err((
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        e.to_string(),
                    ))
                }
            }
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT folder_a, folder_b, similarity, shared_files, union_files
                     FROM near_duplicate_folder_pairs
                     WHERE similarity >= ?1 AND similarity < ?2
                     ORDER BY similarity DESC, shared_files DESC
                     LIMIT ?3 OFFSET ?4",
                )
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let mapped = stmt.query_map(
                rusqlite::params![min_sim, max_sim, per_page as i64, offset as i64],
                map_pair_row,
            );
            match mapped {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(e) => {
                    return Err((
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        e.to_string(),
                    ))
                }
            }
        };

        let count_sql = if q_pat.is_some() {
            "SELECT COUNT(*) FROM near_duplicate_folder_pairs
             WHERE similarity >= ?1 AND similarity < ?2
               AND (folder_a LIKE ?3 OR folder_b LIKE ?3)"
        } else {
            "SELECT COUNT(*) FROM near_duplicate_folder_pairs
             WHERE similarity >= ?1 AND similarity < ?2"
        };
        let total: i64 = if let Some(pat) = &q_pat {
            conn.query_row(count_sql, rusqlite::params![min_sim, max_sim, pat], |r| r.get(0))
        } else {
            conn.query_row(count_sql, rusqlite::params![min_sim, max_sim], |r| r.get(0))
        }
        .unwrap_or(0);

        Ok(Json(NearDupResponse {
            pairs,
            total_pairs: total as usize,
            page,
            per_page,
            needs_refresh: false,
        }))
    })
    .await
}

#[derive(Deserialize)]
pub struct DeltaParams {
    /// Path of the first folder.
    pub a: String,
    /// Path of the second folder.
    pub b: String,
}

/// GET /api/near-duplicate-folders/delta?a=/path/a&b=/path/b
///
/// Reports the concrete file differences for a pair: files only in A, only
/// in B, changed (same name, different hash) and identical count.
pub async fn near_duplicate_delta_handler(
    State(state): State<AppState>,
    Query(params): Query<DeltaParams>,
) -> Result<Json<PairDelta>, (axum::http::StatusCode, String)> {
    let a = params.a.trim_end_matches('/').to_string();
    let b = params.b.trim_end_matches('/').to_string();
    if a.is_empty() || b.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Both 'a' and 'b' query params are required".to_string(),
        ));
    }
    offload(move || {
        let _guard = IndexerPauseGuard::new(&state);
        let conn = get_connection(&state.db).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;
        Ok(Json(compute_pair_delta(&conn, &a, &b)))
    })
    .await
}
