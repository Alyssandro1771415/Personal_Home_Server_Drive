use std::io::SeekFrom;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header::{
    ACCEPT_RANGES, AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE,
    RANGE,
};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::domain::{guess_mime, FileId, FolderId, UploadConflictMode};
use crate::ports::FileRepoBd;
use crate::use_cases::list_files::ListFiles;
use crate::use_cases::mutate_file::MutateFile;
use crate::use_cases::upload_file::{UploadFile, UploadPayload};

use super::auth::{authenticate_token, AuthUser};
use super::json_map::{file_json, map_err, FileJson};
use super::state::AppState;

const STREAM_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
struct FileQuery {
    folder_id: Option<i64>,
}

#[derive(Deserialize)]
struct ContentQuery {
    token: Option<String>,
    inline: Option<String>,
}

#[derive(Deserialize)]
struct PatchFileJson {
    nome: Option<String>,
    #[serde(default, deserialize_with = "super::json_map::present_or_null")]
    folder_id: Option<Option<i64>>,
}

#[derive(Deserialize)]
struct BatchMoveRequestJson {
    file_ids: Vec<i64>,
    folder_id: Option<i64>,
}

#[derive(Serialize)]
struct BatchMoveResponseJson {
    moved_ids: Vec<i64>,
    conflict_ids: Vec<i64>,
    conflict_names: Vec<String>,
}

pub fn files_routes() -> Router<AppState> {
    Router::new()
        .route("/files", post(upload).get(list_files))
        .route("/files/batch-move", post(batch_move_files))
        .route("/files/{id}/content", get(download).head(download))
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
    let mut field_nome: Option<String> = None;
    let mut explicit_nome: Option<String> = None;
    let mut mime: Option<String> = None;
    let mut temp_upload: Option<(std::path::PathBuf, u64)> = None;
    let mut folder_id: Option<i64> = None;
    let mut conflict_raw: Option<String> = None;

    let parse_res: Result<(), StatusCode> = async {
        while let Some(mut field) = multipart
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
                Some("on_conflict") => {
                    let raw = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                    let trimmed = raw.trim();
                    if !trimmed.is_empty() {
                        conflict_raw = Some(trimmed.to_string());
                    }
                }
                Some("file_name") => {
                    let raw = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                    let trimmed = raw.trim();
                    if !trimmed.is_empty() {
                        explicit_nome = Some(trimmed.to_string());
                    }
                }
                Some("file") => {
                    if let Some((old_path, _)) = temp_upload.take() {
                        let _ = tokio::fs::remove_file(old_path).await;
                    }
                    field_nome = field.file_name().map(|s| s.to_string());
                    mime = field.content_type().map(|s| s.to_string());

                    let (tmp_path, mut tmp_file) = state
                        .store
                        .create_temp_file()
                        .await
                        .map_err(map_err)?;

                    let mut written: u64 = 0;
                    while let Some(chunk) = match field.chunk().await {
                        Ok(c) => c,
                        Err(_) => {
                            drop(tmp_file);
                            let _ = tokio::fs::remove_file(&tmp_path).await;
                            return Err(StatusCode::BAD_REQUEST);
                        }
                    } {
                        if tmp_file.write_all(&chunk).await.is_err() {
                            drop(tmp_file);
                            let _ = tokio::fs::remove_file(&tmp_path).await;
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        }
                        written = written.saturating_add(chunk.len() as u64);
                    }
                    if tmp_file.flush().await.is_err() {
                        drop(tmp_file);
                        let _ = tokio::fs::remove_file(&tmp_path).await;
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                    drop(tmp_file);
                    temp_upload = Some((tmp_path, written));
                }
                _ => {}
            }
        }
        Ok(())
    }
    .await;

    if let Err(err_code) = parse_res {
        if let Some((tmp_path, _)) = temp_upload {
            let _ = tokio::fs::remove_file(tmp_path).await;
        }
        return Err(err_code);
    }

    let Some((tmp_path, size)) = temp_upload else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let conflict_mode = match UploadConflictMode::parse(conflict_raw.as_deref()) {
        Ok(mode) => mode,
        Err(err) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(map_err(err));
        }
    };

    let nome = explicit_nome
        .or(field_nome)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "unnamed".to_string());

    let uc = UploadFile {
        users: Arc::new(state.users.clone()),
        repo: Arc::new(state.files.clone()),
        folders: Arc::new(state.folders.clone()),
        store: Arc::new(state.store.clone()),
    };

    let file = uc
        .execute(
            auth.id,
            folder_id.map(FolderId),
            nome,
            mime,
            UploadPayload::TempFile {
                path: tmp_path,
                size,
            },
            conflict_mode,
        )
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

fn content_disposition(nome: &str, inline: bool) -> HeaderValue {
    let kind = if inline { "inline" } else { "attachment" };
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
    HeaderValue::from_str(&format!("{kind}; filename=\"{safe}\""))
        .unwrap_or_else(|_| {
            if inline {
                HeaderValue::from_static("inline")
            } else {
                HeaderValue::from_static("attachment")
            }
        })
}

fn is_truthy_flag(raw: Option<&str>) -> bool {
    matches!(
        raw.map(str::trim),
        Some("1") | Some("true") | Some("TRUE") | Some("yes")
    )
}

#[derive(Debug, PartialEq, Eq)]
struct ByteRange {
    start: u64,
    end: u64,
}

fn parse_byte_range(raw: &str, total_len: u64) -> Result<ByteRange, ()> {
    if total_len == 0 {
        return Err(());
    }
    let spec = raw.trim().strip_prefix("bytes=").ok_or(())?;
    // Em caso de múltiplos intervalos, atende o primeiro intervalo se válido, ou rejeita se malformado
    let first = spec.split(',').next().ok_or(())?.trim();
    let (start_s, end_s) = first.split_once('-').ok_or(())?;
    let start_s = start_s.trim();
    let end_s = end_s.trim();

    if start_s.is_empty() {
        // Formato sufixo: bytes=-N (últimos N bytes)
        let suffix: u64 = end_s.parse().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        let start = total_len.saturating_sub(suffix);
        Ok(ByteRange {
            start,
            end: total_len - 1,
        })
    } else {
        let start: u64 = start_s.parse().map_err(|_| ())?;
        if start >= total_len {
            return Err(());
        }
        let end = if end_s.is_empty() {
            total_len - 1
        } else {
            let parsed_end: u64 = end_s.parse().map_err(|_| ())?;
            if parsed_end < start {
                return Err(());
            }
            parsed_end.min(total_len - 1)
        };
        Ok(ByteRange { start, end })
    }
}

fn reader_to_body<R>(reader: R) -> Body
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let stream = futures_util::stream::unfold(reader, |mut r| async move {
        let mut buf = vec![0u8; STREAM_CHUNK_BYTES];
        match r.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok::<Bytes, std::io::Error>(Bytes::from(buf)), r))
            }
            Err(err) => Some((Err(err), r)),
        }
    });
    Body::from_stream(stream)
}

fn authenticate_media_request(
    state: &AppState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<AuthUser, StatusCode> {
    if let Some(auth_header) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(bearer) = auth_header.strip_prefix("Bearer ").filter(|t| !t.is_empty()) {
            return authenticate_token(state, bearer);
        }
    }
    if let Some(token) = query_token {
        return authenticate_token(state, token);
    }
    Err(StatusCode::UNAUTHORIZED)
}

async fn download(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<ContentQuery>,
) -> Result<Response, StatusCode> {
    let auth = authenticate_media_request(&state, &headers, query.token.as_deref())?;

    let file = state
        .files
        .get(FileId(id))
        .await
        .map_err(map_err)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if file.owner_id != auth.id {
        return Err(StatusCode::NOT_FOUND);
    }

    let (mut disk_file, total_len) = state
        .store
        .open_read(&file.storage_key)
        .await
        .map_err(map_err)?;

    let fallback_mime = guess_mime(&file.nome);
    let mime = file
        .mime
        .as_deref()
        .or(fallback_mime.as_deref())
        .and_then(|m| HeaderValue::from_str(m).ok())
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"));

    let inline = is_truthy_flag(query.inline.as_deref());
    let disposition = content_disposition(&file.nome, inline);

    if let Some(range_val) = headers.get(RANGE).and_then(|v| v.to_str().ok()) {
        let Ok(range) = parse_byte_range(range_val, total_len) else {
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            resp.headers_mut()
                .insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
            if let Ok(cr) = HeaderValue::from_str(&format!("bytes */{total_len}")) {
                resp.headers_mut().insert(CONTENT_RANGE, cr);
            }
            return Ok(resp);
        };

        let chunk_len = range.end - range.start + 1;
        let body = if method == Method::HEAD {
            Body::empty()
        } else {
            disk_file
                .seek(SeekFrom::Start(range.start))
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            reader_to_body(disk_file.take(chunk_len))
        };

        let mut response = Response::new(body);
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
        let h = response.headers_mut();
        h.insert(CONTENT_TYPE, mime);
        h.insert(CONTENT_DISPOSITION, disposition);
        h.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        if let Ok(cl) = HeaderValue::from_str(&chunk_len.to_string()) {
            h.insert(CONTENT_LENGTH, cl);
        }
        if let Ok(cr) =
            HeaderValue::from_str(&format!("bytes {}-{}/{}", range.start, range.end, total_len))
        {
            h.insert(CONTENT_RANGE, cr);
        }
        return Ok(response);
    }

    let body = if method == Method::HEAD || total_len == 0 {
        Body::empty()
    } else {
        reader_to_body(disk_file)
    };

    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::OK;
    let h = response.headers_mut();
    h.insert(CONTENT_TYPE, mime);
    h.insert(CONTENT_DISPOSITION, disposition);
    h.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(cl) = HeaderValue::from_str(&total_len.to_string()) {
        h.insert(CONTENT_LENGTH, cl);
    }
    Ok(response)
}

async fn batch_move_files(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<BatchMoveRequestJson>,
) -> Result<Json<BatchMoveResponseJson>, StatusCode> {
    if payload.file_ids.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ids: Vec<FileId> = payload.file_ids.into_iter().map(FileId).collect();
    let res = mutate(&state)
        .batch_move_to(auth.id, &ids, payload.folder_id.map(FolderId))
        .await
        .map_err(map_err)?;

    Ok(Json(BatchMoveResponseJson {
        moved_ids: res.moved_ids,
        conflict_ids: res.conflict_ids,
        conflict_names: res.conflict_names,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_http_byte_ranges() {
        // Open-ended range: bytes=0-
        assert_eq!(
            parse_byte_range("bytes=0-", 1000).unwrap(),
            ByteRange { start: 0, end: 999 }
        );
        // Safari probe: bytes=0-1
        assert_eq!(
            parse_byte_range("bytes=0-1", 1000).unwrap(),
            ByteRange { start: 0, end: 1 }
        );
        // Mid-stream seek: bytes=500-799
        assert_eq!(
            parse_byte_range("bytes=500-799", 1000).unwrap(),
            ByteRange {
                start: 500,
                end: 799
            }
        );
        // Clamped end beyond file length
        assert_eq!(
            parse_byte_range("bytes=900-5000", 1000).unwrap(),
            ByteRange {
                start: 900,
                end: 999
            }
        );
        // Suffix range (e.g. MP4 moov atom at tail): bytes=-128
        assert_eq!(
            parse_byte_range("bytes=-128", 1000).unwrap(),
            ByteRange {
                start: 872,
                end: 999
            }
        );
        // Suffix larger than file clamps to 0..total-1
        assert_eq!(
            parse_byte_range("bytes=-5000", 1000).unwrap(),
            ByteRange { start: 0, end: 999 }
        );
    }

    #[test]
    fn rejects_invalid_or_unsatisfiable_ranges() {
        assert!(parse_byte_range("bytes=0-", 0).is_err());
        assert!(parse_byte_range("bytes=1000-", 1000).is_err());
        assert!(parse_byte_range("bytes=500-400", 1000).is_err());
        assert!(parse_byte_range("bytes=-0", 1000).is_err());
        assert!(parse_byte_range("items=0-10", 1000).is_err());
    }
}
