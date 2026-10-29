use std::sync::Arc;

use chrono::{Duration, Utc};

use crate::domain::Erro;
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd, UserRepoBd};
use crate::use_cases::storage_paths::{trash_folder_dir, user_root};

pub struct PurgeExpired<U, F, D, S> {
    pub users: Arc<U>,
    pub files: Arc<F>,
    pub folders: Arc<D>,
    pub store: Arc<S>,
    pub ttl: Duration,
}

impl<U: UserRepoBd, F: FileRepoBd, D: FolderRepoBd, S: FileStore> PurgeExpired<U, F, D, S> {
    pub async fn execute(&self) -> Result<(), Erro> {
        let cutoff = Utc::now() - self.ttl;

        let folders = self.folders.list_expired(cutoff).await?;
        for folder in folders {
            let desc_folders = self
                .folders
                .descendant_folders(folder.owner_id, folder.id)
                .await?;
            if desc_folders.is_empty() {
                continue;
            }
            let ids: Vec<i32> = desc_folders.iter().map(|f| f.id.0 as i32).collect();
            let files = self.files.list_in_folders(&ids).await?;
            for file in files {
                self.store.remove_file(&file.storage_key).await?;
            }
            if let Ok(root) = user_root(&*self.users, folder.owner_id).await {
                for df in &desc_folders {
                    let td = trash_folder_dir(&root, df.id, &df.nome);
                    self.store.remove_dir_all(&td).await?;
                }
            }
            self.files.delete_in_folders(&ids).await?;
            self.folders.delete_ids(&ids).await?;
        }

        let files = self.files.list_expired(cutoff).await?;
        for file in files {
            self.store.remove_file(&file.storage_key).await?;
            self.files.delete(file.id).await?;
        }
        Ok(())
    }
}
