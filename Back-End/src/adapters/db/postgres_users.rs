use sqlx::PgPool;
use crate::domain::{Erro, User, UserId, UserSecret, WrappedDek};
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

fn map_sqlx(e: sqlx::Error) -> Erro {
    if let sqlx::Error::Database(db) = &e {
        if db.code().as_deref() == Some("23505") {
            return Erro::Conflito;
        }
    }
    eprintln!("sqlx: {e}");
    Erro::Interno
}

impl UserRepoBd for PostgresUsers {
    async fn list(&self) -> Result<Vec<User>, Erro> {
        let rows = sqlx::query_as::<_, (i32, String)>(
            "SELECT id, nome FROM users ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(rows
            .into_iter()
            .map(|(id, name)| User {
                id: UserId(id as i64),
                name,
            })
            .collect())
    }

    async fn get(&self, id: UserId) -> Result<Option<User>, Erro> {
        let row = sqlx::query_as::<_, (i32, String)>(
            "SELECT id, nome FROM users WHERE id = $1",
        )
        .bind(id.0 as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(row.map(|(id, name)| User {
            id: UserId(id as i64),
            name,
        }))
    }

    async fn get_hashs(&self, nome: &str) -> Result<Option<UserSecret>, Erro> {
        let row = sqlx::query_as::<_, (i32, String, Vec<u8>, Vec<u8>)>(
            "SELECT id, password_hash, dek_salt, wrapped_dek FROM users WHERE nome = $1",
        )
        .bind(nome)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(row.map(|(id, hash, dek_salt, wrapped_dek)| UserSecret {
            user_id: UserId(id as i64),
            password_hash: hash,
            dek_salt,
            wrapped_dek,
        }))
    }

    async fn create(
        &self,
        nome: String,
        password_hash: String,
        wrapped: WrappedDek,
    ) -> Result<UserId, Erro> {
        let id = sqlx::query_scalar::<_, i32>(
            "INSERT INTO users (nome, password_hash, dek_salt, wrapped_dek) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(&nome)
        .bind(&password_hash)
        .bind(&wrapped.salt)
        .bind(&wrapped.blob)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(UserId(id as i64))
    }
}
