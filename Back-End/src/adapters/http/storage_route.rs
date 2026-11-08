use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::ports::FileStore;

use super::auth::AuthUser;
use super::json_map::map_err;
use super::state::AppState;

#[derive(Serialize)]
struct StorageStatsJson {
    used_bytes: u64,
    total_bytes: u64,
    available_bytes: u64,
}

pub fn storage_routes() -> Router<AppState> {
    Router::new().route("/storage", get(get_storage_stats))
}

async fn get_storage_stats(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<StorageStatsJson>, StatusCode> {
    let stats = state.store.disk_usage().await.map_err(map_err)?;
    Ok(Json(StorageStatsJson {
        used_bytes: stats.used_bytes,
        total_bytes: stats.total_bytes,
        available_bytes: stats.available_bytes,
    }))
}
