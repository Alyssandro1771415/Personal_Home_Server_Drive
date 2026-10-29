use std::sync::Arc;

use chrono::Utc;

use crate::domain::{validate_name, Erro, FileId, FolderId, StoredFile, UserId};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::storage_paths::{
    folder_rel_dir, pick_free_name, trash_file_key, user_root,
};

const NOME_MAX: usize = 200;

pub struct MutateFile<U, R, D, S> {
    pub users: Arc<U>,
    pub files: Arc<R>,
    pub folders: Arc<D>,
    pub store: Arc<S>,
}

impl<U: UserRepoBd, R: FileRepoBd, D: FolderRepoBd, S: FileStore> MutateFile<U, R, D, S> {
    async fn owned(&self, owner: UserId, id: FileId) -> Result<StoredFile, Erro> {
        let file = self.files.get(id).await?.ok_or(Erro::NaoEncontrado)?;
        if file.owner_id != owner {
            return Err(Erro::NaoEncontrado);
        }
        Ok(file)
    }

    pub async fn rename(&self, owner: UserId, id: FileId, nome: String) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_some() {
            return Err(Erro::NaoEncontrado);
        }
        let nome = validate_name(&nome, NOME_MAX)?;
        if nome == file.nome {
            return Ok(());
        }

        let root = user_root(&*self.users, owner).await?;
        let parent_dir =
            folder_rel_dir(&*self.folders, owner, &root, file.folder_id.map(FolderId)).await?;
        let new_key = format!("{parent_dir}/{nome}");

        if new_key != file.storage_key && self.store.exists(&new_key).await? {
            return Err(Erro::Conflito);
        }

        self.store.move_file(&file.storage_key, &new_key).await?;
        self.files
            .update_entry(id, file.folder_id, nome, new_key, None)
            .await
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
        let dest_raw = folder_id.map(|f| f.0);
        if dest_raw == file.folder_id {
            return Ok(());
        }

        let root = user_root(&*self.users, owner).await?;
        let dest_dir = folder_rel_dir(&*self.folders, owner, &root, folder_id).await?;
        let new_key = format!("{dest_dir}/{}", file.nome);

        if self.store.exists(&new_key).await? {
            return Err(Erro::Conflito);
        }

        self.store.move_file(&file.storage_key, &new_key).await?;
        self.files
            .update_entry(id, dest_raw, file.nome, new_key, None)
            .await
    }

    pub async fn trash(&self, owner: UserId, id: FileId) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_some() {
            return Ok(());
        }

        let root = user_root(&*self.users, owner).await?;
        let trash_key = trash_file_key(&root, id, &file.nome);
        let _ = self.store.remove_file(&trash_key).await;

        self.store.move_file(&file.storage_key, &trash_key).await?;
        self.files
            .update_entry(id, file.folder_id, file.nome, trash_key, Some(Utc::now()))
            .await
    }

    pub async fn restore(&self, owner: UserId, id: FileId) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_none() {
            return Err(Erro::NaoEncontrado);
        }

        let target_folder = match file.folder_id {
            Some(fid) => match self.folders.get(FolderId(fid)).await? {
                Some(folder) if folder.owner_id == owner && folder.trashed_at.is_none() => {
                    Some(FolderId(fid))
                }
                _ => None,
            },
            None => None,
        };

        let root = user_root(&*self.users, owner).await?;
        let dest_dir = folder_rel_dir(&*self.folders, owner, &root, target_folder).await?;
        let (restored_nome, restored_key) =
            pick_free_name(&*self.store, &dest_dir, &file.nome, NOME_MAX).await?;

        self.store
            .move_file(&file.storage_key, &restored_key)
            .await?;
        self.files
            .update_entry(
                id,
                target_folder.map(|f| f.0),
                restored_nome,
                restored_key,
                None,
            )
            .await
    }

    pub async fn purge(&self, owner: UserId, id: FileId) -> Result<(), Erro> {
        let file = self.owned(owner, id).await?;
        if file.trashed_at.is_none() {
            return Err(Erro::Validacao);
        }
        self.store.remove_file(&file.storage_key).await?;
        self.files.delete(id).await
    }
}
