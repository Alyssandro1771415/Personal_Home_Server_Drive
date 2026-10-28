use std::sync::Arc;

use crate::domain::{Dek, Erro, FileId, StoredFile, UserId};
use crate::ports::{Crypto, FileRepoBd, FileStore};

pub struct DownloadFile<R, C, S> {
    pub repo: Arc<R>,
    pub crypto: Arc<C>,
    pub store: Arc<S>,
}

impl<R: FileRepoBd, C: Crypto, S: FileStore> DownloadFile<R, C, S> {
    pub async fn execute(
        &self,
        owner: UserId,
        dek: &Dek,
        id: FileId,
    ) -> Result<(StoredFile, Vec<u8>), Erro> {
        let file = self.repo.get(id).await?.ok_or(Erro::NaoEncontrado)?;
        if file.owner_id != owner {
            return Err(Erro::NaoEncontrado);
        }

        let ciphertext = self.store.get(owner, &file.storage_key).await?;
        let plaintext = self.crypto.decrypt(dek, &ciphertext)?;
        Ok((file, plaintext))
    }
}
