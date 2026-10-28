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

fn from_row(
    id: i32,
    owner_id: i32,
    folder_id: Option<i32>,
    nome: String,
    mime: Option<String>,
    tamanho_bytes: i64,
    storage_key: String,
) -> StoredFile {
    StoredFile {
        id: FileId(id as i64),
        owner_id: UserId(owner_id as i64),
        folder_id: folder_id.map(|v| v as i64),
        nome,
        mime,
        tamanho_bytes,
        storage_key,
    }
}

impl FileRepoBd for PostgresFiles {
    async fn create(
        &self,
        owner: UserId,
        nome: String,
        mime: Option<String>,
        tamanho_bytes: i64,
        storage_key: String,
    ) -> Result<StoredFile, Erro> {
        let row = sqlx::query_as::<_, (i32, i32, Option<i32>, String, Option<String>, i64, String)>(
            "INSERT INTO files (owner_id, folder_id, nome, mime, tamanho_bytes, storage_key)
             VALUES ($1, NULL, $2, $3, $4, $5)
             RETURNING id, owner_id, folder_id, nome, mime, tamanho_bytes, storage_key",
        )
        .bind(owner.0 as i32)
        .bind(&nome)
        .bind(&mime)
        .bind(tamanho_bytes)
        .bind(&storage_key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(from_row(row.0, row.1, row.2, row.3, row.4, row.5, row.6))
    }

    async fn list_root(&self, owner: UserId) -> Result<Vec<StoredFile>, Erro> {
        let rows = sqlx::query_as::<_, (i32, i32, Option<i32>, String, Option<String>, i64, String)>(
            "SELECT id, owner_id, folder_id, nome, mime, tamanho_bytes, storage_key
             FROM files
             WHERE owner_id = $1 AND folder_id IS NULL
             ORDER BY id",
        )
        .bind(owner.0 as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(rows
            .into_iter()
            .map(|r| from_row(r.0, r.1, r.2, r.3, r.4, r.5, r.6))
            .collect())
    }

    async fn get(&self, id: FileId) -> Result<Option<StoredFile>, Erro> {
        let row = sqlx::query_as::<_, (i32, i32, Option<i32>, String, Option<String>, i64, String)>(
            "SELECT id, owner_id, folder_id, nome, mime, tamanho_bytes, storage_key
             FROM files
             WHERE id = $1",
        )
        .bind(id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(row.map(|r| from_row(r.0, r.1, r.2, r.3, r.4, r.5, r.6)))
    }
}
