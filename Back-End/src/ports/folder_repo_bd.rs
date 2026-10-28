use std::future::Future;

use crate::domain::{Erro, Folder, FolderId, UserId};

pub trait FolderRepoBd: Send + Sync {
    fn create(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
        nome: String,
    ) -> impl Future<Output = Result<Folder, Erro>> + Send;

    fn list(
        &self,
        owner: UserId,
        parent_id: Option<FolderId>,
    ) -> impl Future<Output = Result<Vec<Folder>, Erro>> + Send;

    fn get(&self, id: FolderId) -> impl Future<Output = Result<Option<Folder>, Erro>> + Send;
}
