use sqlx::PgPool;
use crate::domain::{Erro, User, UserId, UserSecret};
use crate::ports::UserRepoBd;


#[derive(Clone)]
pub struct PostgresUsers {
    pool: PgPool,
}

impl PostgresUsers {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl UserRepoBd for PostgresUsers {
    async fn list(&self) -> Result<Vec<User>, Erro> {
        let rows = sqlx::query_as::<_, (i32, String)>(
            "SELECT id, nome FROM users ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await.map_err(|e| {
            eprintln!("sqlx: {e}");
            Erro::Interno
        })?;

        Ok(rows
            .into_iter()
            .map(|(id, name)| User {
                id: UserId(id as i64),
                name,
            })
            .collect())
    }

    async fn get_hashs(&self, id: UserId) -> Result<Option<UserSecret>, Erro> {
        let rows = sqlx::query_as::<_, (i32, String)>(
            "SELECT id, password_hash FROM users WHERE id = $1",
        )
        .bind(id.0 as i32)
        .fetch_optional(&self.pool)
        .await.map_err(|e| {
            eprintln!("sqlx: {e}");
            Erro::Interno
        })?;

        Ok(rows.map(|(id, hash)| UserSecret {
            user_id: UserId(id as i64),
            password_hash: hash,
        }))
    }

    async fn create(&self, nome: String, password_hash: String) -> Result<UserId, Erro> {
        let id = sqlx::query_scalar::<_, i32>(
            "INSERT INTO users (nome, password_hash) VALUES ($1, $2) RETURNING id",
        )
        .bind(&nome)
        .bind(&password_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            eprintln!("sqlx: {e}");
            Erro::Interno
        })?;

        Ok(UserId(id as i64))
    }
}
