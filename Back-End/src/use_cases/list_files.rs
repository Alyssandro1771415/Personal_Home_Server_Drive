use std::sync::Arc;

use crate::domain::{Erro, FolderId, StoredFile, UserId};
use crate::ports::{FileRepoBd, FolderRepoBd};

pub struct ListFiles<F, D> {
    pub files: Arc<F>,
    pub folders: Arc<D>,
}

impl<F: FileRepoBd, D: FolderRepoBd> ListFiles<F, D> {
    pub async fn execute(
        &self,
        owner: UserId,
        folder_id: Option<FolderId>,
    ) -> Result<Vec<StoredFile>, Erro> {
        if let Some(id) = folder_id {
            let folder = self.folders.get(id).await?.ok_or(Erro::NaoEncontrado)?;
            if folder.owner_id != owner || folder.trashed_at.is_some() {
                return Err(Erro::NaoEncontrado);
            }
        }
        self.files.list_in(owner, folder_id.map(|id| id.0)).await
    }
}
