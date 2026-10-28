use std::sync::Arc;

use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
use argon2::Argon2;

use crate::domain::{Erro, UserId};
use crate::ports::{Crypto, FileStore, UserRepoBd};

const NOME_MAX: usize = 50;
const PASSWORD_MIN: usize = 8;

pub struct Register<R, C, S> {
    pub repo: Arc<R>,
    pub crypto: Arc<C>,
    pub store: Arc<S>,
}

impl<R: UserRepoBd, C: Crypto, S: FileStore> Register<R, C, S> {
    pub async fn execute(&self, nome: String, password: String) -> Result<UserId, Erro> {
        let nome = nome.trim().to_string();
        if nome.is_empty() || nome.len() > NOME_MAX || password.len() < PASSWORD_MIN {
            return Err(Erro::Validacao);
        }

        let salt = SaltString::generate(OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| Erro::Interno)?
            .to_string();

        let dek = self.crypto.generate_dek();
        let wrapped = self.crypto.wrap_dek(&password, &dek)?;

        let id = self.repo.create(nome, password_hash, wrapped).await?;
        self.store.ensure_user_dir(id).await?;
        Ok(id)
    }
}
