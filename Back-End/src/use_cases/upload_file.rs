use std::sync::Arc;

use crate::domain::{Dek, Erro, FolderId, StoredFile, UserId};
use crate::ports::{Crypto, FileRepoBd, FileStore, FolderRepoBd};

pub struct UploadFile<R, D, C, S> {
    pub repo: Arc<R>,
    pub folders: Arc<D>,
    pub crypto: Arc<C>,
    pub store: Arc<S>,
}

impl<R: FileRepoBd, D: FolderRepoBd, C: Crypto, S: FileStore> UploadFile<R, D, C, S> {
    pub async fn execute(
        &self,
        owner: UserId,
        dek: &Dek,
        folder_id: Option<FolderId>,
        nome: String,
        mime: Option<String>,
        plaintext: Vec<u8>,
    ) -> Result<StoredFile, Erro> {
        let nome = nome.trim().to_string();
        if nome.is_empty() {
            return Err(Erro::Validacao);
        }
        if let Some(id) = folder_id {
            let folder = self.folders.get(id).await?.ok_or(Erro::NaoEncontrado)?;
            if folder.owner_id != owner {
                return Err(Erro::NaoEncontrado);
            }
        }

        let ciphertext = self.crypto.encrypt(dek, &plaintext)?;
        let storage_key = uuid::Uuid::new_v4().to_string();
        self.store.ensure_user_dir(owner).await?;
        self.store.put(owner, &storage_key, &ciphertext).await?;

        self.repo
            .create(
                owner,
                folder_id.map(|id| id.0),
                nome,
                mime,
                plaintext.len() as i64,
                storage_key,
            )
            .await
    }
}
