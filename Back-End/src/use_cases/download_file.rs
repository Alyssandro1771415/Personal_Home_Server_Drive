use std::sync::Arc;

use crate::domain::{Erro, FileId, StoredFile, UserId};
use crate::ports::{FileRepoBd, FileStore};

pub struct DownloadFile<R, S> {
    pub repo: Arc<R>,
    pub store: Arc<S>,
}

impl<R: FileRepoBd, S: FileStore> DownloadFile<R, S> {
    pub async fn execute(
        &self,
        owner: UserId,
        id: FileId,
    ) -> Result<(StoredFile, Vec<u8>), Erro> {
        let file = self.repo.get(id).await?.ok_or(Erro::NaoEncontrado)?;
        if file.owner_id != owner {
            return Err(Erro::NaoEncontrado);
        }

        let bytes = self.store.get(&file.storage_key).await?;
        Ok((file, bytes))
    }
}
