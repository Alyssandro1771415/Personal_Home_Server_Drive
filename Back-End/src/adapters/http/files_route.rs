use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::domain::{Erro, FileId, FolderId};
use crate::use_cases::download_file::DownloadFile;
use crate::use_cases::list_files::ListFiles;
use crate::use_cases::upload_file::UploadFile;

use super::auth::AuthUser;
use super::state::AppState;

#[derive(Deserialize)]
struct FileQuery {
    folder_id: Option<i64>,
}

#[derive(Serialize)]
struct FileJson {
    id: i64,
    nome: String,
    mime: Option<String>,
    tamanho_bytes: i64,
    folder_id: Option<i64>,
}

pub fn files_routes() -> Router<AppState> {
    Router::new()
        .route("/files", post(upload).get(list_files))
        .route("/files/{id}/content", get(download))
}

fn map_err(err: Erro) -> StatusCode {
    match err {
        Erro::Validacao => StatusCode::BAD_REQUEST,
        Erro::NaoEncontrado => StatusCode::NOT_FOUND,
        Erro::NaoAutorizado => StatusCode::UNAUTHORIZED,
        Erro::Conflito | Erro::Interno => StatusCode::INTERNAL_SERVER_ERROR,
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
        repo: Arc::new(state.files.clone()),
        folders: Arc::new(state.folders.clone()),
        crypto: Arc::new(state.crypto),
        store: Arc::new(state.store.clone()),
    };

    let file = uc
        .execute(
            auth.id,
            &auth.dek,
            folder_id.map(FolderId),
            nome,
            mime,
            data,
        )
        .await
        .map_err(map_err)?;

    Ok((
        StatusCode::CREATED,
        Json(FileJson {
            id: file.id.0,
            nome: file.nome,
            mime: file.mime,
            tamanho_bytes: file.tamanho_bytes,
            folder_id: file.folder_id,
        }),
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
            .map(|f| FileJson {
                id: f.id.0,
                nome: f.nome,
                mime: f.mime,
                tamanho_bytes: f.tamanho_bytes,
                folder_id: f.folder_id,
            })
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
        crypto: Arc::new(state.crypto),
        store: Arc::new(state.store.clone()),
    };

    let (file, bytes) = uc
        .execute(auth.id, &auth.dek, FileId(id))
        .await
        .map_err(map_err)?;

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
