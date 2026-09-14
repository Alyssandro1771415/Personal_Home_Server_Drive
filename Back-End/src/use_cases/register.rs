use std::sync::Arc;

use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
use argon2::Argon2;

use crate::domain::{Erro, UserId};
use crate::ports::UserRepoBd;

pub struct Register<R> {
    pub repo: Arc<R>,
}

impl<R: UserRepoBd> Register<R> {
    pub async fn execute(&self, nome: String, password: String) -> Result<UserId, Erro> {
        let salt = SaltString::generate(OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| Erro::Interno)?
            .to_string();

        self.repo.create(nome, password_hash).await
    }
}
