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

fn from_row(id: i32, owner_id: i32, parent_id: Option<i32>, nome: String) -> Folder {
    Folder {
        id: FolderId(id as i64),
        owner_id: UserId(owner_id as i64),
        parent_id: parent_id.map(|v| FolderId(v as i64)),
        nome,
    }
}

impl FolderRepoBd for PostgresFolders {
    async fn create(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
        nome: String,
    ) -> Result<Folder, Erro> {
        let row = sqlx::query_as::<_, (i32, i32, Option<i32>, String)>(
            "INSERT INTO folders (owner_id, parent_id, nome)
             VALUES ($1, $2, $3)
             RETURNING id, owner_id, parent_id, nome",
        )
        .bind(owner.0 as i32)
        .bind(parent_id.map(|p| p.0 as i32))
        .bind(&nome)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(from_row(row.0, row.1, row.2, row.3))
    }

    async fn list(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
    ) -> Result<Vec<Folder>, Erro> {
        let rows = sqlx::query_as::<_, (i32, i32, Option<i32>, String)>(
            "SELECT id, owner_id, parent_id, nome
             FROM folders
             WHERE owner_id = $1 AND parent_id IS NOT DISTINCT FROM $2
             ORDER BY lower(nome), id",
        )
        .bind(owner.0 as i32)
        .bind(parent_id.map(|p| p.0 as i32))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(rows
            .into_iter()
            .map(|r| from_row(r.0, r.1, r.2, r.3))
            .collect())
    }

    async fn get(&self, id: FolderId) -> Result<Option<Folder>, Erro> {
        let row = sqlx::query_as::<_, (i32, i32, Option<i32>, String)>(
            "SELECT id, owner_id, parent_id, nome
             FROM folders
             WHERE id = $1",
        )
        .bind(id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(row.map(|r| from_row(r.0, r.1, r.2, r.3)))
    }
}
