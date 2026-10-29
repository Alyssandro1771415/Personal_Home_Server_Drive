use std::sync::Arc;

use chrono::Utc;

use crate::domain::{validate_name, Erro, Folder, FolderId, UserId};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::storage_paths::{
    folder_rel_dir, pick_free_name, trash_folder_dir, user_root,
};

const NOME_MAX: usize = 80;

pub struct MutateFolder<U, R, F, S> {
    pub users: Arc<U>,
    pub folders: Arc<R>,
    pub files: Arc<F>,
    pub store: Arc<S>,
}

impl<U: UserRepoBd, R: FolderRepoBd, F: FileRepoBd, S: FileStore> MutateFolder<U, R, F, S> {
    async fn owned(&self, owner: UserId, id: FolderId) -> Result<Folder, Erro> {
        let folder = self.folders.get(id).await?.ok_or(Erro::NaoEncontrado)?;
        if folder.owner_id != owner {
            return Err(Erro::NaoEncontrado);
        }
        Ok(folder)
    }

    async fn is_descendant(
        &self,
        owner: UserId,
        id: FolderId,
        maybe_ancestor: FolderId,
    ) -> Result<bool, Erro> {
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
        let nome = validate_name(&nome, NOME_MAX)?;
        if nome == folder.nome {
            return Ok(());
        }

        let root = user_root(&*self.users, owner).await?;
        let parent_dir = folder_rel_dir(&*self.folders, owner, &root, folder.parent_id).await?;
        let old_dir = format!("{parent_dir}/{}", folder.nome);
        let new_dir = format!("{parent_dir}/{nome}");

        if new_dir != old_dir && self.store.exists(&new_dir).await? {
            return Err(Erro::Conflito);
        }

        let ids = self.folders.descendant_ids(owner, id).await?;
        self.store.move_dir(&old_dir, &new_dir).await?;
        self.folders.rename(id, nome).await?;
        self.files
            .replace_key_prefix_in_folders(
                owner,
                &ids,
                &format!("{old_dir}/"),
                &format!("{new_dir}/"),
            )
            .await
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
        if parent_id == folder.parent_id {
            return Ok(());
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

        let root = user_root(&*self.users, owner).await?;
        let old_parent_dir = folder_rel_dir(&*self.folders, owner, &root, folder.parent_id).await?;
        let new_parent_dir = folder_rel_dir(&*self.folders, owner, &root, parent_id).await?;
        let old_dir = format!("{old_parent_dir}/{}", folder.nome);
        let new_dir = format!("{new_parent_dir}/{}", folder.nome);

        if self.store.exists(&new_dir).await? {
            return Err(Erro::Conflito);
        }

        let ids = self.folders.descendant_ids(owner, id).await?;
        self.store.move_dir(&old_dir, &new_dir).await?;
        self.folders.set_parent(id, parent_id).await?;
        self.files
            .replace_key_prefix_in_folders(
                owner,
                &ids,
                &format!("{old_dir}/"),
                &format!("{new_dir}/"),
            )
            .await
    }

    pub async fn trash(&self, owner: UserId, id: FolderId) -> Result<(), Erro> {
        let folder = self.owned(owner, id).await?;
        if folder.trashed_at.is_some() {
            return Ok(());
        }

        let root = user_root(&*self.users, owner).await?;
        let parent_dir = folder_rel_dir(&*self.folders, owner, &root, folder.parent_id).await?;
        let old_dir = format!("{parent_dir}/{}", folder.nome);
        let trash_dir = trash_folder_dir(&root, id, &folder.nome);

        let ids = self.folders.descendant_ids(owner, id).await?;
        let at = Utc::now();

        self.store.remove_dir_all(&trash_dir).await?;
        self.store.move_dir(&old_dir, &trash_dir).await?;

        self.folders.trash_ids(owner, &ids, at).await?;
        self.files.trash_in_folders(owner, &ids, at).await?;
        self.files
            .replace_key_prefix_in_folders(
                owner,
                &ids,
                &format!("{old_dir}/"),
                &format!("{trash_dir}/"),
            )
            .await
    }

    pub async fn restore(&self, owner: UserId, id: FolderId) -> Result<(), Erro> {
        let folder = self.owned(owner, id).await?;
        let Some(folder_trashed_at) = folder.trashed_at else {
            return Err(Erro::NaoEncontrado);
        };

        let target_parent = match folder.parent_id {
            Some(parent) => match self.folders.get(parent).await? {
                Some(p) if p.owner_id == owner && p.trashed_at.is_none() => Some(parent),
                _ => {
                    self.folders.set_parent(id, None).await?;
                    None
                }
            },
            None => None,
        };

        let root = user_root(&*self.users, owner).await?;
        let trash_dir = trash_folder_dir(&root, id, &folder.nome);
        let dest_parent_dir = folder_rel_dir(&*self.folders, owner, &root, target_parent).await?;
        let (restored_nome, restored_dir) =
            pick_free_name(&*self.store, &dest_parent_dir, &folder.nome, NOME_MAX).await?;

        let ids = self.folders.descendant_ids(owner, id).await?;
        self.store.move_dir(&trash_dir, &restored_dir).await?;

        if restored_nome != folder.nome {
            self.folders.rename(id, restored_nome).await?;
        }

        self.folders
            .restore_ids(owner, &ids, folder_trashed_at)
            .await?;
        self.files
            .restore_in_folders(owner, &ids, folder_trashed_at)
            .await?;
        self.files
            .replace_key_prefix_in_folders(
                owner,
                &ids,
                &format!("{trash_dir}/"),
                &format!("{restored_dir}/"),
            )
            .await
    }

    pub async fn purge(&self, owner: UserId, id: FolderId) -> Result<(), Erro> {
        let folder = self.owned(owner, id).await?;
        if folder.trashed_at.is_none() {
            return Err(Erro::Validacao);
        }

        let desc_folders = self.folders.descendant_folders(owner, id).await?;
        let ids: Vec<i32> = desc_folders.iter().map(|f| f.id.0 as i32).collect();
        let files = self.files.list_in_folders(&ids).await?;

        for file in files {
            self.store.remove_file(&file.storage_key).await?;
        }
        if let Ok(root) = user_root(&*self.users, owner).await {
            for df in &desc_folders {
                let td = trash_folder_dir(&root, df.id, &df.nome);
                self.store.remove_dir_all(&td).await?;
            }
        }

        self.files.delete_in_folders(&ids).await?;
        self.folders.delete_ids(&ids).await
    }
}
