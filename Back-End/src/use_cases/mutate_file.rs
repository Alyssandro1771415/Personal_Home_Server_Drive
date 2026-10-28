use std::sync::Arc;

use chrono::Utc;

use crate::domain::{Erro, FileId, FolderId, StoredFile, UserId};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd};

const NOME_MAX: usize = 200;

pub struct MutateFile<R, D, S> {
    pub files: Arc<R>,
    pub folders: Arc<D>,
    pub store: Arc<S>,
}

impl<R: FileRepoBd, D: FolderRepoBd, S: FileStore> MutateFile<R, D, S> {
    async fn owned(&self, owner: UserId, id: FileId) -> Result<StoredFile, Erro> {
        let file = self.files.get(id).await?.ok_or(Erro::NaoEncontrado)?;
        if file.owner_id != owner {
            return Err(Erro::NaoEncontrado);
        }
        Ok(file)
    }

    async fn live_folder(&self, owner: UserId, folder_id: FolderId) -> Result<(), Erro> {
        let folder = self.folders.get(folder_id).await?.ok_or(Erro::NaoEncontrado)?;
        if folder.owner_id != owner || folder.trashed_at.is_some() {
            return Err(Erro::NaoEncontrado);
        }
        Ok(())
    }

    pub async fn rename(&self, owner: UserId, id: FileId, nome: String) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_some() {
            return Err(Erro::NaoEncontrado);
        }
        let nome = nome.trim().to_string();
        if nome.is_empty() || nome.len() > NOME_MAX || nome.contains('/') {
            return Err(Erro::Validacao);
        }
        self.files.rename(id, nome).await
    }

    pub async fn move_to(
        &self,
        owner: UserId,
        id: FileId,
        folder_id: Option<FolderId>,
    ) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_some() {
            return Err(Erro::NaoEncontrado);
        }
        if let Some(dest) = folder_id {
            self.live_folder(owner, dest).await?;
        }
        self.files.set_folder(id, folder_id.map(|f| f.0)).await
    }

    pub async fn trash(&self, owner: UserId, id: FileId) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_some() {
            return Ok(());
        }
        self.files.set_trashed(id, Some(Utc::now())).await
    }

    pub async fn restore(&self, owner: UserId, id: FileId) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_none() {
            return Err(Erro::NaoEncontrado);
        }
        if let Some(folder_id) = file.folder_id {
            match self.folders.get(FolderId(folder_id)).await? {
                Some(folder)
                    if folder.owner_id == owner && folder.trashed_at.is_none() => {}
                _ => self.files.set_folder(id, None).await?,
            }
        }
        self.files.set_trashed(id, None).await
    }

    pub async fn purge(&self, owner: UserId, id: FileId) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_none() {
            return Err(Erro::Validacao);
        }
        self.store.remove(owner, &file.storage_key).await?;
        self.files.delete(id).await
    }
}
