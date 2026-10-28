use std::sync::Arc;

use crate::domain::{Dek, Erro, StoredFile, UserId};
use crate::ports::{Crypto, FileRepoBd, FileStore};

pub struct UploadFile<R, C, S> {
    pub repo: Arc<R>,
    pub crypto: Arc<C>,
    pub store: Arc<S>,
}

impl<R: FileRepoBd, C: Crypto, S: FileStore> UploadFile<R, C, S> {
    pub async fn execute(
        &self,
        owner: UserId,
        dek: &Dek,
        nome: String,
        mime: Option<String>,
        plaintext: Vec<u8>,
    ) -> Result<StoredFile, Erro> {
        let nome = nome.trim().to_string();
        if nome.is_empty() {
            return Err(Erro::Validacao);
        }

        let ciphertext = self.crypto.encrypt(dek, &plaintext)?;
        let storage_key = uuid::Uuid::new_v4().to_string();
        self.store.ensure_user_dir(owner).await?;
        self.store.put(owner, &storage_key, &ciphertext).await?;

        self.repo
            .create(
                owner,
                nome,
                mime,
                plaintext.len() as i64,
                storage_key,
            )
            .await
    }
}
