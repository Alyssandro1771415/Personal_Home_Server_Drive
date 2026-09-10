use sqlx::PgPool;
use crate::domain::{Erro, User, UserId};
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
            "SELECT * FROM users ORDER BY id",
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
}
