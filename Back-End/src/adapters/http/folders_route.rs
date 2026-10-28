use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::domain::{Erro, FolderId};
use crate::use_cases::create_folder::CreateFolder;
use crate::use_cases::folder_path::FolderPath;
use crate::use_cases::list_folders::ListFolders;

use super::auth::AuthUser;
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

#[derive(Serialize)]
struct FolderJson {
    id: i64,
    nome: String,
    parent_id: Option<i64>,
}

pub fn folders_routes() -> Router<AppState> {
    Router::new()
        .route("/folders", get(list_folders).post(create_folder))
        .route("/folders/{id}/path", get(folder_path))
}

fn map_err(err: Erro) -> StatusCode {
    match err {
        Erro::Validacao => StatusCode::BAD_REQUEST,
        Erro::NaoEncontrado => StatusCode::NOT_FOUND,
        Erro::NaoAutorizado => StatusCode::UNAUTHORIZED,
        Erro::Conflito | Erro::Interno => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn to_json(folder: crate::domain::Folder) -> FolderJson {
    FolderJson {
        id: folder.id.0,
        nome: folder.nome,
        parent_id: folder.parent_id.map(|p| p.0),
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
    Ok(Json(folders.into_iter().map(to_json).collect()))
}

async fn create_folder(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateFolderJson>,
) -> Result<(StatusCode, Json<FolderJson>), StatusCode> {
    let uc = CreateFolder {
        repo: Arc::new(state.folders.clone()),
    };
    let folder = uc
        .execute(auth.id, payload.parent_id.map(FolderId), payload.nome)
        .await
        .map_err(map_err)?;
    Ok((StatusCode::CREATED, Json(to_json(folder))))
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
    Ok(Json(path.into_iter().map(to_json).collect()))
}
