use std::sync::Arc;

use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
use argon2::Argon2;

use crate::domain::{Erro, User};
use crate::ports::UserRepoBd;

const PASSWORD_MIN: usize = 8;

pub struct ResetPassword<R> {
    pub repo: Arc<R>,
}

impl<R: UserRepoBd> ResetPassword<R> {
    pub async fn execute(&self, nome: &str, new_password: &str) -> Result<User, Erro> {
        let nome = nome.trim();
        if nome.is_empty() || new_password.len() < PASSWORD_MIN {
            return Err(Erro::Validacao);
        }

        let user = self
            .repo
            .get_by_name(nome)
            .await?
            .ok_or(Erro::NaoEncontrado)?;

        let salt = SaltString::generate(OsRng);
        let password_hash = Argon2::default()
            .hash_password(new_password.as_bytes(), &salt)
            .map_err(|_| Erro::Interno)?
            .to_string();

        self.repo.update_password(user.id, password_hash).await?;
        Ok(user)
    }
}
