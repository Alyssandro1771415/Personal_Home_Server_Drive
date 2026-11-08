use std::sync::Arc;

use crate::domain::{Erro, UserId, TRASH_DIR_NAME};
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::mutate_file::MutateFile;
use crate::use_cases::mutate_folder::MutateFolder;
use crate::use_cases::storage_paths::user_root;

pub struct EmptyTrash<U, F, D, S> {
    pub users: Arc<U>,
    pub files: Arc<F>,
    pub folders: Arc<D>,
    pub store: Arc<S>,
}

impl<U: UserRepoBd, F: FileRepoBd, D: FolderRepoBd, S: FileStore> EmptyTrash<U, F, D, S> {
    pub async fn execute(&self, owner: UserId) -> Result<(), Erro> {
        let mf_folder = MutateFolder {
            users: Arc::clone(&self.users),
            folders: Arc::clone(&self.folders),
            files: Arc::clone(&self.files),
            store: Arc::clone(&self.store),
        };
        let mf_file = MutateFile {
            users: Arc::clone(&self.users),
            files: Arc::clone(&self.files),
            folders: Arc::clone(&self.folders),
            store: Arc::clone(&self.store),
        };

        let trashed_folders = self.folders.list_trash(owner).await?;
        for folder in trashed_folders {
            mf_folder.purge(owner, folder.id).await?;
        }

        let trashed_files = self.files.list_trash(owner).await?;
        for file in trashed_files {
            mf_file.purge(owner, file.id).await?;
        }

        if let Ok(root) = user_root(&*self.users, owner).await {
            let _ = self
                .store
                .remove_dir_all(&format!("{root}/{TRASH_DIR_NAME}"))
                .await;
        }

        Ok(())
    }
}
