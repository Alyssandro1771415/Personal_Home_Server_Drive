use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::use_cases::list_trash::ListTrash;
use crate::use_cases::purge_expired::PurgeExpired;

use super::auth::AuthUser;
use super::json_map::{file_json, folder_json, map_err, FileJson, FolderJson};
use super::state::AppState;

#[derive(Serialize)]
struct TrashJson {
    folders: Vec<FolderJson>,
    files: Vec<FileJson>,
    ttl_days: i64,
}

pub fn trash_routes() -> Router<AppState> {
    Router::new().route("/trash", get(list_trash))
}

async fn list_trash(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<TrashJson>, axum::http::StatusCode> {
    let purger = PurgeExpired {
        files: Arc::new(state.files.clone()),
        folders: Arc::new(state.folders.clone()),
        store: Arc::new(state.store.clone()),
        ttl: state.trash_ttl,
    };
    let _ = purger.execute().await;

    let uc = ListTrash {
        files: Arc::new(state.files.clone()),
        folders: Arc::new(state.folders.clone()),
    };
    let listing = uc.execute(auth.id).await.map_err(map_err)?;
    Ok(Json(TrashJson {
        folders: listing
            .folders
            .into_iter()
            .map(|f| folder_json(f, state.trash_ttl))
            .collect(),
        files: listing
            .files
            .into_iter()
            .map(|f| file_json(f, state.trash_ttl))
            .collect(),
        ttl_days: state.trash_ttl.num_days(),
    }))
}
