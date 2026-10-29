use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::domain::FolderId;
use crate::use_cases::create_folder::CreateFolder;
use crate::use_cases::folder_path::FolderPath;
use crate::use_cases::list_folders::ListFolders;
use crate::use_cases::mutate_folder::MutateFolder;

use super::auth::AuthUser;
use super::json_map::{folder_json, map_err, FolderJson};
use super::state::AppState;

#[derive(Deserialize)]
struct FolderQuery {
    parent_id: Option<i64>,
}

#[derive(Deserialize)]
struct CreateFolderJson {
    nome: String,
    parent_id: Option<i64>,
}

#[derive(Deserialize)]
struct PatchFolderJson {
    nome: Option<String>,
    #[serde(default, deserialize_with = "super::json_map::present_or_null")]
    parent_id: Option<Option<i64>>,
}

pub fn folders_routes() -> Router<AppState> {
    Router::new()
        .route("/folders", get(list_folders).post(create_folder))
        .route("/folders/{id}/path", get(folder_path))
        .route(
            "/folders/{id}",
            axum::routing::patch(patch_folder).delete(trash_folder),
        )
        .route("/folders/{id}/restore", post(restore_folder))
        .route("/folders/{id}/purge", delete(purge_folder))
}

fn mutate(
    state: &AppState,
) -> MutateFolder<
    crate::adapters::db::postgres_users::PostgresUsers,
    crate::adapters::db::postgres_folders::PostgresFolders,
    crate::adapters::db::postgres_files::PostgresFiles,
    crate::adapters::fs::FsStore,
> {
    MutateFolder {
        users: Arc::new(state.users.clone()),
        folders: Arc::new(state.folders.clone()),
        files: Arc::new(state.files.clone()),
        store: Arc::new(state.store.clone()),
    }
}

async fn list_folders(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<FolderQuery>,
) -> Result<Json<Vec<FolderJson>>, StatusCode> {
    let uc = ListFolders {
        repo: Arc::new(state.folders.clone()),
    };
    let folders = uc
        .execute(auth.id, query.parent_id.map(FolderId))
        .await
        .map_err(map_err)?;
    Ok(Json(
        folders
            .into_iter()
            .map(|f| folder_json(f, state.trash_ttl))
            .collect(),
    ))
}

async fn create_folder(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateFolderJson>,
) -> Result<(StatusCode, Json<FolderJson>), StatusCode> {
    let uc = CreateFolder {
        users: Arc::new(state.users.clone()),
        repo: Arc::new(state.folders.clone()),
        store: Arc::new(state.store.clone()),
    };
    let folder = uc
        .execute(auth.id, payload.parent_id.map(FolderId), payload.nome)
        .await
        .map_err(map_err)?;
    Ok((
        StatusCode::CREATED,
        Json(folder_json(folder, state.trash_ttl)),
    ))
}

async fn folder_path(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Vec<FolderJson>>, StatusCode> {
    let uc = FolderPath {
        repo: Arc::new(state.folders.clone()),
    };
    let path = uc
        .execute(auth.id, FolderId(id))
        .await
        .map_err(map_err)?;
    Ok(Json(
        path.into_iter()
            .map(|f| folder_json(f, state.trash_ttl))
            .collect(),
    ))
}

async fn patch_folder(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(payload): Json<PatchFolderJson>,
) -> Result<StatusCode, StatusCode> {
    let uc = mutate(&state);
    let id = FolderId(id);
    let mut did = false;
    if let Some(nome) = payload.nome {
        uc.rename(auth.id, id, nome).await.map_err(map_err)?;
        did = true;
    }
    if let Some(parent) = payload.parent_id {
        uc.move_to(auth.id, id, parent.map(FolderId))
            .await
            .map_err(map_err)?;
        did = true;
    }
    if !did {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn trash_folder(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    mutate(&state)
        .trash(auth.id, FolderId(id))
        .await
        .map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restore_folder(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    mutate(&state)
        .restore(auth.id, FolderId(id))
        .await
        .map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn purge_folder(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    mutate(&state)
        .purge(auth.id, FolderId(id))
        .await
        .map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}
