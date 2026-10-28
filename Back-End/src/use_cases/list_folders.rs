use std::sync::Arc;

use crate::domain::{Erro, Folder, FolderId, UserId};
use crate::ports::FolderRepoBd;

pub struct ListFolders<R> {
    pub repo: Arc<R>,
}

impl<R: FolderRepoBd> ListFolders<R> {
    pub async fn execute(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
    ) -> Result<Vec<Folder>, Erro> {
        if let Some(parent) = parent_id {
            let folder = self.repo.get(parent).await?.ok_or(Erro::NaoEncontrado)?;
            if folder.owner_id != owner {
                return Err(Erro::NaoEncontrado);
            }
        }
        self.repo.list(owner, parent_id).await
    }
}
