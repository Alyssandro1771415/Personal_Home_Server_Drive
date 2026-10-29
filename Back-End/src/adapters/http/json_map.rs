use chrono::{DateTime, Duration, Utc};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};

use crate::domain::{Folder, StoredFile};

/// Distingue campo ausente (`None`) de `null` (`Some(None)` = raiz).
pub fn present_or_null<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Serialize)]
pub struct FileJson {
    pub id: i64,
    pub nome: String,
    pub mime: Option<String>,
    pub tamanho_bytes: i64,
    pub folder_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct FolderJson {
    pub id: i64,
    pub nome: String,
    pub parent_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

pub fn file_json(file: StoredFile, ttl: Duration) -> FileJson {
    FileJson {
        id: file.id.0,
        nome: file.nome,
        mime: file.mime,
        tamanho_bytes: file.tamanho_bytes,
        folder_id: file.folder_id,
        expires_at: file.trashed_at.map(|t| t + ttl),
        trashed_at: file.trashed_at,
    }
}

pub fn folder_json(folder: Folder, ttl: Duration) -> FolderJson {
    FolderJson {
        id: folder.id.0,
        nome: folder.nome,
        parent_id: folder.parent_id.map(|p| p.0),
        expires_at: folder.trashed_at.map(|t| t + ttl),
        trashed_at: folder.trashed_at,
    }
}

pub fn map_err(err: crate::domain::Erro) -> axum::http::StatusCode {
    use crate::domain::Erro;
    use axum::http::StatusCode;
    match err {
        Erro::Validacao => StatusCode::BAD_REQUEST,
        Erro::NaoEncontrado => StatusCode::NOT_FOUND,
        Erro::NaoAutorizado => StatusCode::UNAUTHORIZED,
        Erro::Conflito => StatusCode::CONFLICT,
        Erro::Interno => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
