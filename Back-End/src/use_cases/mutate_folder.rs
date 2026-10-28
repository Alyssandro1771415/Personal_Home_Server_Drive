use std::sync::Arc;

use chrono::Utc;

use crate::domain::{Erro, Folder, FolderId, UserId};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd};

const NOME_MAX: usize = 80;

pub struct MutateFolder<R, F, S> {
    pub folders: Arc<R>,
    pub files: Arc<F>,
    pub store: Arc<S>,
}

impl<R: FolderRepoBd, F: FileRepoBd, S: FileStore> MutateFolder<R, F, S> {
    async fn owned(&self, owner: UserId, id: FolderId) -> Result<Folder, Erro> {
        let folder = self.folders.get(id).await?.ok_or(Erro::NaoEncontrado)?;
        if folder.owner_id != owner {
            return Err(Erro::NaoEncontrado);
        }
        Ok(folder)
    }

    async fn is_descendant(&self, owner: UserId, id: FolderId, maybe_ancestor: FolderId) -> Result<bool, Erro> {
        let mut current = Some(id);
        let mut guard = 0;
        while let Some(folder_id) = current {
            guard += 1;
            if guard > 32 {
                return Err(Erro::Interno);
            }
            if folder_id == maybe_ancestor {
                return Ok(true);
            }
            let folder = self.folders.get(folder_id).await?.ok_or(Erro::NaoEncontrado)?;
            if folder.owner_id != owner {
                return Err(Erro::NaoEncontrado);
            }
            current = folder.parent_id;
        }
        Ok(false)
    }

    pub async fn rename(&self, owner: UserId, id: FolderId, nome: String) -> Result<(), Erro> {
        let folder = self.owned(owner, id).await?;
        if folder.trashed_at.is_some() {
            return Err(Erro::NaoEncontrado);
        }
        let nome = nome.trim().to_string();
        if nome.is_empty() || nome.len() > NOME_MAX || nome.contains('/') {
            return Err(Erro::Validacao);
        }
        self.folders.rename(id, nome).await
    }

    pub async fn move_to(
        &self,
        owner: UserId,
        id: FolderId,
        parent_id: Option<FolderId>,
    ) -> Result<(), Erro> {
        let folder = self.owned(owner, id).await?;
        if folder.trashed_at.is_some() {
            return Err(Erro::NaoEncontrado);
        }
        if let Some(dest) = parent_id {
            if dest == id {
                return Err(Erro::Validacao);
            }
            let dest_folder = self.folders.get(dest).await?.ok_or(Erro::NaoEncontrado)?;
            if dest_folder.owner_id != owner || dest_folder.trashed_at.is_some() {
                return Err(Erro::NaoEncontrado);
            }
            if self.is_descendant(owner, dest, id).await? {
                return Err(Erro::Validacao);
            }
        }
        self.folders.set_parent(id, parent_id).await
    }

    pub async fn trash(&self, owner: UserId, id: FolderId) -> Result<(), Erro> {
        let folder = self.owned(owner, id).await?;
        if folder.trashed_at.is_some() {
            return Ok(());
        }
        let ids = self.folders.descendant_ids(owner, id).await?;
        let at = Utc::now();
        self.folders.trash_ids(owner, &ids, at).await?;
        self.files.trash_in_folders(owner, &ids, at).await
    }

    pub async fn restore(&self, owner: UserId, id: FolderId) -> Result<(), Erro> {
        let folder = self.owned(owner, id).await?;
        if folder.trashed_at.is_none() {
            return Err(Erro::NaoEncontrado);
        }
        let ids = self.folders.descendant_ids(owner, id).await?;
        if let Some(parent) = folder.parent_id {
            match self.folders.get(parent).await? {
                Some(p) if p.owner_id == owner && p.trashed_at.is_none() => {}
                _ => self.folders.set_parent(id, None).await?,
            }
        }
        self.folders.restore_ids(owner, &ids).await?;
        self.files.restore_in_folders(owner, &ids).await
    }

    pub async fn purge(&self, owner: UserId, id: FolderId) -> Result<(), Erro> {
        let folder = self.owned(owner, id).await?;
        if folder.trashed_at.is_none() {
            return Err(Erro::Validacao);
        }
        let ids = self.folders.descendant_ids(owner, id).await?;
        let files = self.files.list_in_folders(&ids).await?;
        for file in files {
            self.store.remove(owner, &file.storage_key).await?;
        }
        self.files.delete_in_folders(&ids).await?;
        self.folders.delete_ids(&ids).await
    }
}
