use std::sync::Arc;

use argon2::{Argon2, PasswordHash, PasswordVerifier};

use crate::domain::{Dek, Erro, UserId, WrappedDek};
use crate::ports::{Crypto, UserRepoBd};

pub struct GetHashVal<R, C> {
    pub repo: Arc<R>,
    pub crypto: Arc<C>,
}

impl<R: UserRepoBd, C: Crypto> GetHashVal<R, C> {
    pub async fn execute(&self, nome: String, password: String) -> Result<(UserId, Dek), Erro> {
        let secret = self.repo.get_hashs(&nome).await?;

        let Some(secret) = secret else {
            return Err(Erro::NaoAutorizado);
        };

        let parsed =
            PasswordHash::new(&secret.password_hash).map_err(|_| Erro::Interno)?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| Erro::NaoAutorizado)?;

        let wrapped = WrappedDek {
            salt: secret.dek_salt,
            blob: secret.wrapped_dek,
        };
        let dek = self.crypto.unwrap_dek(&password, &wrapped)?;
        Ok((secret.user_id, dek))
    }
}
