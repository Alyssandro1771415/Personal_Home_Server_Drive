use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::domain::{FileId, FolderId};
use crate::use_cases::download_file::DownloadFile;
use crate::use_cases::list_files::ListFiles;
use crate::use_cases::mutate_file::MutateFile;
use crate::use_cases::upload_file::UploadFile;

use super::auth::AuthUser;
use super::json_map::{file_json, map_err, FileJson};
use super::state::AppState;

#[derive(Deserialize)]
struct FileQuery {
    folder_id: Option<i64>,
}

#[derive(Deserialize)]
struct PatchFileJson {
    nome: Option<String>,
    #[serde(default, deserialize_with = "super::json_map::present_or_null")]
    folder_id: Option<Option<i64>>,
}

pub fn files_routes() -> Router<AppState> {
    Router::new()
        .route("/files", post(upload).get(list_files))
        .route("/files/{id}/content", get(download))
        .route(
            "/files/{id}",
            axum::routing::patch(patch_file).delete(trash_file),
        )
        .route("/files/{id}/restore", post(restore_file))
        .route("/files/{id}/purge", delete(purge_file))
}

fn mutate(
    state: &AppState,
) -> MutateFile<
    crate::adapters::db::postgres_users::PostgresUsers,
    crate::adapters::db::postgres_files::PostgresFiles,
    crate::adapters::db::postgres_folders::PostgresFolders,
    crate::adapters::fs::FsStore,
> {
    MutateFile {
        users: Arc::new(state.users.clone()),
        files: Arc::new(state.files.clone()),
        folders: Arc::new(state.folders.clone()),
        store: Arc::new(state.store.clone()),
    }
}

async fn upload(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<FileJson>), StatusCode> {
    let mut nome = None;
    let mut mime = None;
    let mut data = None;
    let mut folder_id = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        match field.name() {
            Some("folder_id") => {
                let raw = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    folder_id = Some(
                        trimmed
                            .parse::<i64>()
                            .map_err(|_| StatusCode::BAD_REQUEST)?,
                    );
                }
            }
            Some("file") => {
                nome = field.file_name().map(|s| s.to_string());
                mime = field.content_type().map(|s| s.to_string());
                data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| StatusCode::BAD_REQUEST)?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let data = data.ok_or(StatusCode::BAD_REQUEST)?;
    let nome = nome
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "unnamed".to_string());

    let uc = UploadFile {
        users: Arc::new(state.users.clone()),
        repo: Arc::new(state.files.clone()),
        folders: Arc::new(state.folders.clone()),
        store: Arc::new(state.store.clone()),
    };

    let file = uc
        .execute(auth.id, folder_id.map(FolderId), nome, mime, data)
        .await
        .map_err(map_err)?;

    Ok((
        StatusCode::CREATED,
        Json(file_json(file, state.trash_ttl)),
    ))
}

async fn list_files(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<FileQuery>,
) -> Result<Json<Vec<FileJson>>, StatusCode> {
    let uc = ListFiles {
        files: Arc::new(state.files.clone()),
        folders: Arc::new(state.folders.clone()),
    };

    let files = uc
        .execute(auth.id, query.folder_id.map(FolderId))
        .await
        .map_err(map_err)?;
    Ok(Json(
        files
            .into_iter()
            .map(|f| file_json(f, state.trash_ttl))
            .collect(),
    ))
}

fn content_disposition(nome: &str) -> HeaderValue {
    let safe: String = nome
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    HeaderValue::from_str(&format!("attachment; filename=\"{safe}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

async fn download(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Response, StatusCode> {
    let uc = DownloadFile {
        repo: Arc::new(state.files.clone()),
        store: Arc::new(state.store.clone()),
    };

    let (file, bytes) = uc.execute(auth.id, FileId(id)).await.map_err(map_err)?;

    let mime = file
        .mime
        .as_deref()
        .and_then(|m| HeaderValue::from_str(m).ok())
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"));

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(CONTENT_TYPE, mime);
    response
        .headers_mut()
        .insert(CONTENT_DISPOSITION, content_disposition(&file.nome));
    Ok(response)
}

async fn patch_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(payload): Json<PatchFileJson>,
) -> Result<StatusCode, StatusCode> {
    let uc = mutate(&state);
    let id = FileId(id);
    let mut did = false;
    if let Some(nome) = payload.nome {
        uc.rename(auth.id, id, nome).await.map_err(map_err)?;
        did = true;
    }
    if let Some(folder) = payload.folder_id {
        uc.move_to(auth.id, id, folder.map(FolderId))
            .await
            .map_err(map_err)?;
        did = true;
    }
    if !did {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn trash_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    mutate(&state)
        .trash(auth.id, FileId(id))
        .await
        .map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restore_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    mutate(&state)
        .restore(auth.id, FileId(id))
        .await
        .map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn purge_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    mutate(&state)
        .purge(auth.id, FileId(id))
        .await
        .map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}
