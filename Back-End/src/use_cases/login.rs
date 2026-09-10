use std::sync::Arc;

use argon2::{Argon2, PasswordHash, PasswordVerifier};

use crate::domain::users::UserId;
use crate::domain::Erro;
use crate::ports::user_repo_bd::UserRepoBd;

pub struct GetHashVal<R> {
    pub repo: Arc<R>,
}

impl<R: UserRepoBd> GetHashVal<R> {
    pub async fn execute(&self, id: UserId, password: String) -> Result<(), Erro> {
        let secret = self.repo.get_hashs(id).await?;

        let Some(secret) = secret else {
            return Err(Erro::NaoAutorizado);
        };

        let parsed =
            PasswordHash::new(&secret.password_hash).map_err(|_| Erro::Interno)?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| Erro::NaoAutorizado)
    }
}
