use std::sync::Arc;

use crate::domain::{Erro, Folder, FolderId, UserId};
use crate::ports::FolderRepoBd;

pub struct FolderPath<R> {
    pub repo: Arc<R>,
}

impl<R: FolderRepoBd> FolderPath<R> {
    pub async fn execute(&self, owner: UserId, id: FolderId) -> Result<Vec<Folder>, Erro> {
        let mut path = Vec::new();
        let mut current = Some(id);
        let mut guard = 0;

        while let Some(folder_id) = current {
            guard += 1;
            if guard > 32 {
                return Err(Erro::Interno);
            }
            let folder = self.repo.get(folder_id).await?.ok_or(Erro::NaoEncontrado)?;
            if folder.owner_id != owner {
                return Err(Erro::NaoEncontrado);
            }
            current = folder.parent_id;
            path.push(folder);
        }

        path.reverse();
        Ok(path)
    }
}
