use std::sync::Arc;

use crate::domain::{Erro, Folder, StoredFile, UserId};
use crate::ports::{FileRepoBd, FolderRepoBd};

pub struct ListTrash<F, D> {
    pub files: Arc<F>,
    pub folders: Arc<D>,
}

pub struct TrashListing {
    pub folders: Vec<Folder>,
    pub files: Vec<StoredFile>,
}

impl<F: FileRepoBd, D: FolderRepoBd> ListTrash<F, D> {
    pub async fn execute(&self, owner: UserId) -> Result<TrashListing, Erro> {
        Ok(TrashListing {
            folders: self.folders.list_trash(owner).await?,
            files: self.files.list_trash(owner).await?,
        })
    }
}
