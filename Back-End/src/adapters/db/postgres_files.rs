use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::domain::{Erro, FileId, StoredFile, UserId};
use crate::ports::FileRepoBd;

#[derive(Clone)]
pub struct PostgresFiles {
    pool: PgPool,
}

impl PostgresFiles {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn map_sqlx(e: sqlx::Error) -> Erro {
    eprintln!("sqlx: {e}");
    Erro::Interno
}

type FileRow = (
    i32,
    i32,
    Option<i32>,
    String,
    Option<String>,
    i64,
    String,
    Option<DateTime<Utc>>,
);

fn from_row(row: FileRow) -> StoredFile {
    StoredFile {
        id: FileId(row.0 as i64),
        owner_id: UserId(row.1 as i64),
        folder_id: row.2.map(|v| v as i64),
        nome: row.3,
        mime: row.4,
        tamanho_bytes: row.5,
        storage_key: row.6,
        trashed_at: row.7,
    }
}

const FILE_COLS: &str =
    "id, owner_id, folder_id, nome, mime, tamanho_bytes, storage_key, trashed_at";

impl FileRepoBd for PostgresFiles {
    async fn create(
        &self,
        owner: UserId,
        folder_id: Option<i64>,
        nome: String,
        mime: Option<String>,
        tamanho_bytes: i64,
        storage_key: String,
    ) -> Result<StoredFile, Erro> {
        let row = sqlx::query_as::<_, FileRow>(&format!(
            "INSERT INTO files (owner_id, folder_id, nome, mime, tamanho_bytes, storage_key)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING {FILE_COLS}"
        ))
        .bind(owner.0 as i32)
        .bind(folder_id.map(|id| id as i32))
        .bind(&nome)
        .bind(&mime)
        .bind(tamanho_bytes)
        .bind(&storage_key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(from_row(row))
    }

    async fn list_in(
        &self,
        owner: UserId,
        folder_id: Option<i64>,
    ) -> Result<Vec<StoredFile>, Erro> {
        let rows = sqlx::query_as::<_, FileRow>(&format!(
            "SELECT {FILE_COLS}
             FROM files
             WHERE owner_id = $1 AND folder_id IS NOT DISTINCT FROM $2 AND trashed_at IS NULL
             ORDER BY lower(nome), id"
        ))
        .bind(owner.0 as i32)
        .bind(folder_id.map(|id| id as i32))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(rows.into_iter().map(from_row).collect())
    }

    async fn get(&self, id: FileId) -> Result<Option<StoredFile>, Erro> {
        let row = sqlx::query_as::<_, FileRow>(&format!(
            "SELECT {FILE_COLS} FROM files WHERE id = $1"
        ))
        .bind(id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(row.map(from_row))
    }

    async fn rename(&self, id: FileId, nome: String) -> Result<(), Erro> {
        sqlx::query("UPDATE files SET nome = $2 WHERE id = $1")
            .bind(id.0 as i32)
            .bind(&nome)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn set_folder(&self, id: FileId, folder_id: Option<i64>) -> Result<(), Erro> {
        sqlx::query("UPDATE files SET folder_id = $2 WHERE id = $1")
            .bind(id.0 as i32)
            .bind(folder_id.map(|v| v as i32))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn set_trashed(&self, id: FileId, at: Option<DateTime<Utc>>) -> Result<(), Erro> {
        sqlx::query("UPDATE files SET trashed_at = $2 WHERE id = $1")
            .bind(id.0 as i32)
            .bind(at)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn trash_in_folders(
        &self,
        owner: UserId,
        folder_ids: &[i32],
        at: DateTime<Utc>,
    ) -> Result<(), Erro> {
        sqlx::query(
            "UPDATE files SET trashed_at = $3
             WHERE owner_id = $1 AND folder_id = ANY($2) AND trashed_at IS NULL",
        )
        .bind(owner.0 as i32)
        .bind(folder_ids)
        .bind(at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn restore_in_folders(&self, owner: UserId, folder_ids: &[i32]) -> Result<(), Erro> {
        sqlx::query(
            "UPDATE files SET trashed_at = NULL
             WHERE owner_id = $1 AND folder_id = ANY($2)",
        )
        .bind(owner.0 as i32)
        .bind(folder_ids)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn list_trash(&self, owner: UserId) -> Result<Vec<StoredFile>, Erro> {
        let rows = sqlx::query_as::<_, FileRow>(&format!(
            "SELECT {FILE_COLS}
             FROM files f
             WHERE f.owner_id = $1
               AND f.trashed_at IS NOT NULL
               AND (
                 f.folder_id IS NULL
                 OR NOT EXISTS (
                   SELECT 1 FROM folders d
                   WHERE d.id = f.folder_id AND d.trashed_at IS NOT NULL
                 )
               )
             ORDER BY f.trashed_at DESC, f.id"
        ))
        .bind(owner.0 as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows.into_iter().map(from_row).collect())
    }

    async fn list_expired(&self, before: DateTime<Utc>) -> Result<Vec<StoredFile>, Erro> {
        let rows = sqlx::query_as::<_, FileRow>(&format!(
            "SELECT {FILE_COLS}
             FROM files
             WHERE trashed_at IS NOT NULL AND trashed_at <= $1"
        ))
        .bind(before)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows.into_iter().map(from_row).collect())
    }

    async fn list_in_folders(&self, folder_ids: &[i32]) -> Result<Vec<StoredFile>, Erro> {
        let rows = sqlx::query_as::<_, FileRow>(&format!(
            "SELECT {FILE_COLS} FROM files WHERE folder_id = ANY($1)"
        ))
        .bind(folder_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows.into_iter().map(from_row).collect())
    }

    async fn delete(&self, id: FileId) -> Result<(), Erro> {
        sqlx::query("DELETE FROM files WHERE id = $1")
            .bind(id.0 as i32)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn delete_in_folders(&self, folder_ids: &[i32]) -> Result<(), Erro> {
        sqlx::query("DELETE FROM files WHERE folder_id = ANY($1)")
            .bind(folder_ids)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
}
