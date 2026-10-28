use std::sync::Arc;

use chrono::{Duration, Utc};

use crate::domain::Erro;
use crate::ports::{FileRepoBd, FileStore, FolderRepoBd};

pub struct PurgeExpired<F, D, S> {
    pub files: Arc<F>,
    pub folders: Arc<D>,
    pub store: Arc<S>,
    pub ttl: Duration,
}

impl<F: FileRepoBd, D: FolderRepoBd, S: FileStore> PurgeExpired<F, D, S> {
    pub async fn execute(&self) -> Result<(), Erro> {
        let cutoff = Utc::now() - self.ttl;

        let folders = self.folders.list_expired(cutoff).await?;
        for folder in folders {
            let ids = self
                .folders
                .descendant_ids(folder.owner_id, folder.id)
                .await?;
            let files = self.files.list_in_folders(&ids).await?;
            for file in files {
                self.store
                    .remove(file.owner_id, &file.storage_key)
                    .await?;
            }
            self.files.delete_in_folders(&ids).await?;
            self.folders.delete_ids(&ids).await?;
        }

        let files = self.files.list_expired(cutoff).await?;
        for file in files {
            self.store.remove(file.owner_id, &file.storage_key).await?;
            self.files.delete(file.id).await?;
        }
        Ok(())
    }
}
