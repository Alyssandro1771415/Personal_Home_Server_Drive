use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::domain::{Erro, Folder, FolderId, UserId};
use crate::ports::FolderRepoBd;

#[derive(Clone)]
pub struct PostgresFolders {
    pool: PgPool,
}

impl PostgresFolders {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn map_sqlx(e: sqlx::Error) -> Erro {
    eprintln!("sqlx: {e}");
    Erro::Interno
}

type FolderRow = (i32, i32, Option<i32>, String, Option<DateTime<Utc>>);

fn from_row(row: FolderRow) -> Folder {
    Folder {
        id: FolderId(row.0 as i64),
        owner_id: UserId(row.1 as i64),
        parent_id: row.2.map(|v| FolderId(v as i64)),
        nome: row.3,
        trashed_at: row.4,
    }
}

const FOLDER_COLS: &str = "id, owner_id, parent_id, nome, trashed_at";

impl FolderRepoBd for PostgresFolders {
    async fn create(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
        nome: String,
    ) -> Result<Folder, Erro> {
        let row = sqlx::query_as::<_, FolderRow>(&format!(
            "INSERT INTO folders (owner_id, parent_id, nome)
             VALUES ($1, $2, $3)
             RETURNING {FOLDER_COLS}"
        ))
        .bind(owner.0 as i32)
        .bind(parent_id.map(|p| p.0 as i32))
        .bind(&nome)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(from_row(row))
    }

    async fn list(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
    ) -> Result<Vec<Folder>, Erro> {
        let rows = sqlx::query_as::<_, FolderRow>(&format!(
            "SELECT {FOLDER_COLS}
             FROM folders
             WHERE owner_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND trashed_at IS NULL
             ORDER BY lower(nome), id"
        ))
        .bind(owner.0 as i32)
        .bind(parent_id.map(|p| p.0 as i32))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows.into_iter().map(from_row).collect())
    }

    async fn get(&self, id: FolderId) -> Result<Option<Folder>, Erro> {
        let row = sqlx::query_as::<_, FolderRow>(&format!(
            "SELECT {FOLDER_COLS} FROM folders WHERE id = $1"
        ))
        .bind(id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(from_row))
    }

    async fn rename(&self, id: FolderId, nome: String) -> Result<(), Erro> {
        sqlx::query("UPDATE folders SET nome = $2 WHERE id = $1")
            .bind(id.0 as i32)
            .bind(&nome)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn set_parent(&self, id: FolderId, parent_id: Option<FolderId>) -> Result<(), Erro> {
        sqlx::query("UPDATE folders SET parent_id = $2 WHERE id = $1")
            .bind(id.0 as i32)
            .bind(parent_id.map(|p| p.0 as i32))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }

    async fn descendant_ids(&self, owner: UserId, id: FolderId) -> Result<Vec<i32>, Erro> {
        let rows = sqlx::query_scalar::<_, i32>(
            "WITH RECURSIVE tree AS (
                SELECT id FROM folders WHERE id = $1 AND owner_id = $2
                UNION ALL
                SELECT f.id FROM folders f
                JOIN tree t ON f.parent_id = t.id
                WHERE f.owner_id = $2
             )
             SELECT id FROM tree",
        )
        .bind(id.0 as i32)
        .bind(owner.0 as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows)
    }

    async fn trash_ids(
        &self,
        owner: UserId,
        ids: &[i32],
        at: DateTime<Utc>,
    ) -> Result<(), Erro> {
        sqlx::query(
            "UPDATE folders SET trashed_at = $3
             WHERE owner_id = $1 AND id = ANY($2) AND trashed_at IS NULL",
        )
        .bind(owner.0 as i32)
        .bind(ids)
        .bind(at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn restore_ids(&self, owner: UserId, ids: &[i32]) -> Result<(), Erro> {
        sqlx::query(
            "UPDATE folders SET trashed_at = NULL WHERE owner_id = $1 AND id = ANY($2)",
        )
        .bind(owner.0 as i32)
        .bind(ids)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn list_trash(&self, owner: UserId) -> Result<Vec<Folder>, Erro> {
        let rows = sqlx::query_as::<_, FolderRow>(&format!(
            "SELECT {FOLDER_COLS}
             FROM folders f
             WHERE f.owner_id = $1
               AND f.trashed_at IS NOT NULL
               AND (
                 f.parent_id IS NULL
                 OR NOT EXISTS (
                   SELECT 1 FROM folders p
                   WHERE p.id = f.parent_id AND p.trashed_at IS NOT NULL
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

    async fn list_expired(&self, before: DateTime<Utc>) -> Result<Vec<Folder>, Erro> {
        let rows = sqlx::query_as::<_, FolderRow>(&format!(
            "SELECT {FOLDER_COLS}
             FROM folders
             WHERE trashed_at IS NOT NULL AND trashed_at <= $1"
        ))
        .bind(before)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows.into_iter().map(from_row).collect())
    }

    async fn delete_ids(&self, ids: &[i32]) -> Result<(), Erro> {
        sqlx::query("UPDATE folders SET parent_id = NULL WHERE id = ANY($1)")
            .bind(ids)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        sqlx::query("DELETE FROM folders WHERE id = ANY($1)")
            .bind(ids)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
}
