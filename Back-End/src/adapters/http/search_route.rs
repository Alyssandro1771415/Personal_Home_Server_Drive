use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::domain::{EntryKindFilter, FolderId};
use crate::use_cases::search_drive::SearchDrive;

use super::auth::AuthUser;
use super::json_map::{located_file_json, located_folder_json, map_err, FileJson, FolderJson};
use super::state::AppState;

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    kind: Option<String>,
    folder_id: Option<i64>,
    recursive: Option<bool>,
}

#[derive(Serialize)]
struct SearchJson {
    folders: Vec<FolderJson>,
    files: Vec<FileJson>,
}

pub fn search_routes() -> Router<AppState> {
    Router::new().route("/search", get(search_drive))
}

async fn search_drive(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchJson>, StatusCode> {
    let kind = EntryKindFilter::parse(query.kind.as_deref()).map_err(map_err)?;
    let uc = SearchDrive {
        files: Arc::new(state.files.clone()),
        folders: Arc::new(state.folders.clone()),
    };

    let listing = uc
        .execute(
            auth.id,
            query.q.as_deref(),
            kind,
            query.folder_id.map(FolderId),
            query.recursive.unwrap_or(true),
        )
        .await
        .map_err(map_err)?;

    Ok(Json(SearchJson {
        folders: listing
            .folders
            .into_iter()
            .map(|item| located_folder_json(item.folder, item.location, state.trash_ttl))
            .collect(),
        files: listing
            .files
            .into_iter()
            .map(|item| located_file_json(item.file, item.location, state.trash_ttl))
            .collect(),
    }))
}
