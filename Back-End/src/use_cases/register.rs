use std::path::PathBuf;
use std::sync::Arc;

use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
use argon2::Argon2;

use crate::domain::{validate_name, Erro, User, UserId};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::sync_disk::sync_user_from_disk;

const NOME_MAX: usize = 50;
const PASSWORD_MIN: usize = 8;

pub struct Register<R, D, F, S> {
    pub repo: Arc<R>,
    pub folders: Arc<D>,
    pub files: Arc<F>,
    pub store: Arc<S>,
    pub storage_root: PathBuf,
}

impl<R: UserRepoBd, D: FolderRepoBd, F: FileRepoBd, S: FileStore> Register<R, D, F, S> {
    pub async fn execute(&self, nome: String, password: String) -> Result<UserId, Erro> {
        let nome = validate_name(&nome, NOME_MAX)?;
        if password.len() < PASSWORD_MIN {
            return Err(Erro::Validacao);
        }

        let salt = SaltString::generate(OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| Erro::Interno)?
            .to_string();

        let id = self.repo.create(nome.clone(), password_hash).await?;
        self.store.ensure_dir(&nome).await?;

        // Caso a pasta do usuário no disco já possua arquivos (ex: banco recriado do zero),
        // reimporta automaticamente toda a árvore de pastas e arquivos!
        let user = User { id, name: nome };
        let _ = sync_user_from_disk(&self.storage_root, &user, &*self.folders, &*self.files).await;

        Ok(id)
    }
}
